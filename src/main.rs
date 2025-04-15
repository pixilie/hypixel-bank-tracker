#![allow(dead_code)]

use dotenvy::{self, var};
use helpers::{get_max_balance, handle_connection};
use models::{Banking, Config, Profile, ProfileResponse, Transaction, Username};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{
	collections::HashMap,
	fmt::Display,
	fs,
	net::TcpListener,
	time::{SystemTime, UNIX_EPOCH},
};
use thread::ThreadPool;
use url::Url;

mod helpers;
mod models;
mod thread;

const DB_FILE: &str = "data.json";
const DB_VERSION: u64 = 3;

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct DataFile {
	version: u64,
	last_transaction_timestamp: u128,
	balance: f64,
	drift: f64,
	max_balance: Option<u64>,
	bank_interests: f64,
	users: HashMap<Username, f64>,
	operations: Vec<(u128, Operation)>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type")]
pub(crate) enum Operation {
	PlayerPurse {
		amount: f64,
		username: Username,
		repeat_count: u64,
	},
	PlayerTransfer {
		amount: f64,
		receiver: Username,
		sender: Username,
		repeat_count: u64,
	},
	WeirdWaypoint,
	BankInterests {
		amount: f64,
	},
}

impl Display for Operation {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			Self::PlayerPurse {
				amount, username, ..
			} => write!(
				f,
				"TSC NEW: {} has {} {} ¤",
				username,
				if *amount > 0.0 { "deposit" } else { "withdraw" },
				amount
			),
			Self::PlayerTransfer {
				amount,
				receiver,
				sender,
				..
			} => write!(
				f,
				"TSF NEW: {sender} has transfered {amount} coins to {receiver}"
			),
			Self::WeirdWaypoint => write!(f, "Weirdwaypoint"),
			Self::BankInterests { amount } => write!(f, "BANK INTEREST: {amount} ¤"),
		}
	}
}

fn load_database(file_path: &str) -> DataFile {
	let content = fs::read_to_string(file_path).expect("Should have been able to read the file");
	let database = serde_json::from_str::<DataFile>(content.as_str());

	database.unwrap()
}

fn write_database(database: &DataFile) {
	let database_json = serde_json::to_string(&database)
		.expect("An error occured while parsing Datafile into json");
	fs::write(DB_FILE, database_json).expect("An error occured while writing into the json file");
}

fn load_config_from_env() -> Config {
	Config {
		hypixel_api_key: var("HYPIXEL_API_KEY").unwrap(),
		profile_uuid: var("PROFILE_UUID").unwrap(),
	}
}

fn fetch_api(config: &Config, client: &Client) -> Profile {
	println!(
		"Fetching fresh information for profile {0}",
		config.profile_uuid
	);

	let mut url = Url::parse("https://api.hypixel.net/v2/skyblock/profile").unwrap();
	url.query_pairs_mut()
		.append_pair("profile", &config.profile_uuid)
		.append_pair("key", &config.hypixel_api_key);

	let response = match client.get(url).send() {
		Ok(response) => response.json::<ProfileResponse>().unwrap(),
		Err(err) => panic!("There was an error while fetching Hypixel's API: {err}"),
	};

	println!(
		"Got fresh information for profile {:?}, reloadind clients",
		config.profile_uuid
	);

	response.profile
}

fn update_transaction(profile: &Profile, database: &mut DataFile) {
	println!("TSC: Updating");

	let banking: Banking = profile.clone().banking;

	let mut new_transactions = banking
		.transactions
		.into_iter()
		.take_while(|transaction| transaction.timestamp > database.last_transaction_timestamp)
		.map(
			|Transaction {
			     amount,
			     initiator_name,
			     timestamp,
			     action,
			 }| {
				let operation = match action {
					models::TransactionAction::Deposit => {
						if let "Bank Interest" | "Bank Interest (x2)" = initiator_name.as_str() {
							Operation::BankInterests { amount }
						} else {
							Operation::PlayerPurse {
								amount,
								username: Username::new(initiator_name),
								repeat_count: 1,
							}
						}
					}
					models::TransactionAction::Withdraw => Operation::PlayerPurse {
						amount: -amount,
						username: Username::new(initiator_name),
						repeat_count: 1,
					},
				};

				(timestamp, operation)
			},
		)
		.collect::<Vec<_>>();

	if new_transactions.len() > 50 {
		println!(
			"TSC WARN: there are 50 new transactions, maybe some were not correctly registered"
		);
		new_transactions.push((
			SystemTime::now()
				.duration_since(UNIX_EPOCH)
				.unwrap()
				.as_millis(),
			Operation::WeirdWaypoint,
		));
	}

	for (timestamp, operation) in new_transactions {
		println!("{operation}");
		match &operation {
			Operation::PlayerPurse {
				amount, username, ..
			} => {
				let user_balance = database.users.entry(username.clone()).or_insert(0.0);
				*user_balance += amount;
			}

			Operation::BankInterests { amount } => {
				database.bank_interests += amount;
			}

			Operation::PlayerTransfer {
				amount,
				receiver,
				sender,
				..
			} => {
				//TODO: Transfer with bank interests
				if (sender == receiver)
					| !database.users.contains_key(sender)
					| !database.users.contains_key(receiver)
				{
					panic!("An error occured with the users concerned by the transfer");
				}

				let [Some(sender_balance), Some(receiver_balance)] =
					database.users.get_disjoint_mut([sender, receiver])
				else {
					panic!("An error occured while writing the transfer in the database");
				};

				*sender_balance -= amount;
				*receiver_balance += amount;
			}

			Operation::WeirdWaypoint => {}
		}

		database.operations.push((timestamp, operation));
	}

	let sum: f64 = database.users.clone().into_values().sum();
	let drift = (banking.balance - sum).abs();

	if drift > 1.0 {
		println!(
			"TSC DRIFT: found {drift} between balance: {0} and sum: {sum}",
			banking.balance
		);
	}

	database.last_transaction_timestamp = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.unwrap()
		.as_millis();

	database.drift = drift;
	database.max_balance = get_max_balance(profile);
	database.balance = banking.balance;
}

fn main() {
	let pool = ThreadPool::new(4);
	let listener = TcpListener::bind("127.0.0.1:7878").unwrap();

	let config = load_config_from_env();
	let mut database = load_database(DB_FILE);
	let client = reqwest::blocking::Client::new();

	pool.execute(move || {
		let new_profile = fetch_api(&config, &client);

		update_transaction(&new_profile, &mut database);
		write_database(&database);
	});

	for stream in listener.incoming() {
		let stream = stream.unwrap();

		pool.execute(|| {
			handle_connection(stream);
		});
	}
}
