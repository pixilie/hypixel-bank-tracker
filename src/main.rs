#![allow(dead_code)]

use core::panic;
use dotenvy::{self, var};
use helpers::get_max_balance;
use models::{Banking, Config, Profile, ProfileResponse, Username};
use reqwest::blocking::Client;
use serde::Deserialize;
use serde_json::from_str;
use std::{
	collections::HashMap,
	fmt::Display,
	fs,
	time::{SystemTime, UNIX_EPOCH},
};
use url::Url;

mod helpers;
mod models;

const DB_FILE: &str = "data.json";
const DB_VERSION: u64 = 3;

#[derive(Debug, Deserialize)]
pub(crate) struct DataFile {
	version: u64,
	last_transaction_timestamp: u128,
	balance: f64,
	drift: f64,
	max_balance: u64,
	bank_interests: f64,
	users: HashMap<Username, f64>,
	operations: Vec<(u128, Operation)>,
}

#[derive(Debug, Deserialize)]
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
				amount,
				username,
				repeat_count,
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
				repeat_count,
			} => write!(
				f,
				"TSF NEW: {} has transfered {} coins to {}",
				sender, amount, receiver
			),
			Self::WeirdWaypoint => writeln!(f, "Weirdwaypoint"),
			Self::BankInterests { amount } => writeln!(f, "BANK INTEREST: {} ¤", amount),
		}
	}
}

fn load_database(file_path: &str) -> DataFile {
	let content = fs::read_to_string(file_path).expect("Should have been able to read the file");
	let database = from_str::<DataFile>(content.as_str());

	database.unwrap()
}

fn load_config_from_env() -> Config {
	Config {
		hypixel_api_key: var("HYPIXEL_API_KEY").unwrap(),
		profile_uuid: var("PROFILE_UUID").unwrap(),
	}
}

fn fetch_api(config: Config, client: Client, database: &mut DataFile) -> Profile {
	println!(
		"Fetching fresh information for profile {0}",
		config.profile_uuid
	);

	database.last_transaction_timestamp = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.expect("...")
		.as_millis();

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

fn update_transaction(profile: Profile, database: &mut DataFile) {
	println!("TSC: Updating");

	let banking: Banking = profile.clone().banking;

	let mut new_transactions = banking
		.transactions
		.into_iter()
		// CHECK: Transacction filtering order
		.take_while(|transaction| transaction.timestamp > database.last_transaction_timestamp)
		.map(|transaction| match transaction.action {
			models::TransactionAction::Deposit => match transaction.initiator_name.as_str() {
				"Bank Interest" | "Bank Interest (x2)" => Operation::BankInterests {
					amount: transaction.amount,
				},
				_ => Operation::PlayerPurse {
					amount: transaction.amount,
					username: transaction.initiator_name,
					repeat_count: 1,
				},
			},
			models::TransactionAction::Withdraw => Operation::PlayerPurse {
				amount: -transaction.amount,
				username: transaction.initiator_name,
				repeat_count: 1,
			},
		})
		.collect::<Vec<_>>();

	if new_transactions.len() > 50 {
		println!(
			"TSC WARN: there are 50 new transactions, maybe some were not correctly registered"
		);
		new_transactions.push(Operation::WeirdWaypoint);
	}

	let last_transaction_timestamp = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.expect("...")
		.as_millis();

	for new_transaction in new_transactions {
		println!("{new_transaction}");

		match new_transaction {
			Operation::PlayerPurse {
				amount,
				username,
				repeat_count,
			} => {
				let user_balance = database.users.entry(username).or_insert(0.0);
				*user_balance += amount;
			}

			Operation::BankInterests { amount } => {
				database.bank_interests += amount;
			}

			Operation::PlayerTransfer {
				amount,
				receiver,
				sender,
				repeat_count,
			} => {
				//TODO: Transfer with bank interests
				if (sender == receiver)
					| !database.users.contains_key(&sender)
					| !database.users.contains_key(&receiver)
				{
					return;
				}

				match database.users.get_disjoint_mut([&sender, &receiver]) {
					[Some(sender_balance), Some(receiver_balance)] => {
						*sender_balance -= amount;
						*receiver_balance += amount
					}
					_ => panic!("An error occured"),
				}

				database.operations.push((
					last_transaction_timestamp,
					Operation::PlayerTransfer {
						amount,
						receiver,
						sender,
						repeat_count,
					},
				));
			}

			Operation::WeirdWaypoint => {
				database
					.operations
					.push((last_transaction_timestamp, Operation::WeirdWaypoint));
			}
		}
	}

	let sum: f64 = database.users.clone().into_values().sum();
	let drift = (banking.balance - sum).abs();

	if drift > 1.0 {
		println!(
			"TSC DRIFT: found {drift} between balance: {0} and sum: {sum}",
			banking.balance
		);
	}

	database.last_transaction_timestamp = last_transaction_timestamp;
	database.drift = drift;
	database.max_balance = get_max_balance(profile);
}

fn main() {
	let config = load_config_from_env();
	let mut database = load_database(DB_FILE);
	let client = reqwest::blocking::Client::new();

	let new_profile = fetch_api(config, client, &mut database);
	update_transaction(new_profile, &mut database);
}
