#![allow(dead_code)]

use askama::Template;
use dotenvy::{self, var};
use helpers::{get_max_balance, process_user_balance_evolution};
use models::{
	BankerTemplate, Banking, Config, Profile, ProfileResponse, Transaction, User, Username,
};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use std::{
	collections::HashMap,
	fmt::Display,
	fs,
	io::{BufRead, BufReader, Write},
	net::{TcpListener, TcpStream},
	sync::Arc,
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
	last_check_timestamp: u128,
	balance: f64,
	drift: f64,
	max_balance: Option<u64>,
	bank_interests: f64,
	users: HashMap<Username, f64>,
	operations: Vec<(u128, Operation)>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
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

fn fetch_api(config: &Config, client: &Client, database: &mut DataFile) -> Profile {
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

	database.last_check_timestamp = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.unwrap()
		.as_millis();

	response.profile
}

fn update_transaction(profile: &Profile, database: &mut DataFile) {
	println!("TSC: Updating...");

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
			"TSC: Updated, there are 50 new transactions, maybe some were not correctly registered"
		);
		new_transactions.push((
			SystemTime::now()
				.duration_since(UNIX_EPOCH)
				.unwrap()
				.as_millis(),
			Operation::WeirdWaypoint,
		));
	} else if new_transactions.is_empty() {
		println!("TSC: Updated, no new transactions");
	} else {
		println!("TSC: Updated, {} new transactions", new_transactions.len());
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

		database.last_transaction_timestamp = timestamp;
		database.operations.push((timestamp, operation));
	}

	let sum = database.users.clone().into_values().sum::<f64>() + database.bank_interests;
	let drift = (banking.balance - sum).abs();

	if drift > 1.0 {
		println!(
			"TSC DRIFT: found {drift} between balance: {0} and sum: {sum}",
			banking.balance
		);
	}

	database.drift = drift;
	database.max_balance = get_max_balance(profile);
	database.balance = banking.balance;
}

pub(crate) fn handle_connection(mut stream: TcpStream, template: &Arc<BankerTemplate>) {
	let reader = BufReader::new(&stream);
	let request_line = reader.lines().next().unwrap().unwrap();
	let request_path = request_line.split_whitespace().nth(1).unwrap_or("/");

	if request_path.starts_with("/static/") {
		let file_path = format!(".{request_path}");
		if let Ok(contents) = fs::read(&file_path) {
			let content_type = match file_path.rsplit('.').next().unwrap_or("") {
				"css" => "text/css",
				"js" => "application/javascript",
				"png" => "image/png",
				"jpg" | "jpeg" => "image/jpeg",
				_ => "application/octet-stream",
			};

			let response = format!(
				"HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\n\r\n",
				content_type,
				contents.len()
			);

			stream.write_all(response.as_bytes()).unwrap();
			stream.write_all(&contents).unwrap();
		} else {
			let response = "HTTP/1.1 404 NOT FOUND\r\n\r\n";
			stream.write_all(response.as_bytes()).unwrap();
		}
	} else {
		let body = template.as_ref().render().unwrap();
		let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=UTF-8\r\nContent-Length: {}\r\n\r\n{}",
            body.len(),
            body
        );

		stream.write_all(response.as_bytes()).unwrap();
	}
}

fn main() {
	let pool = ThreadPool::new(4);
	let listener = TcpListener::bind("127.0.0.1:7878").unwrap();

	let config = load_config_from_env();
	let mut database = load_database(DB_FILE);
	let client = reqwest::blocking::Client::new();

	let mut users = database
		.users
		.clone()
		.into_iter()
		.map(|(username, balance)| User {
			name: username.clone(),
			balance,
			delta: process_user_balance_evolution(&database.operations, &username),
		})
		.collect::<Vec<_>>();
	users.sort_by(|a, b| b.balance.total_cmp(&a.balance));

	let completion_percentage = format!(
		"{:.2}%",
		(database.balance / database.max_balance.unwrap() as f64) * 100.0
	);

	let template = BankerTemplate {
		users,
		operations: database.operations.iter().rev().take(25).cloned().collect(),
		bank_interests: database.bank_interests,
		balance: database.balance,
		max_balance: database.max_balance.unwrap(),
		completion_percentage,
		last_check_timestamp: database.last_check_timestamp,
		last_transaction_timestamp: database.last_transaction_timestamp,
		drift: database.drift,
		total_operations: database.operations.len(),
	};

	pool.execute(move || {
		let new_profile = fetch_api(&config, &client, &mut database);

		update_transaction(&new_profile, &mut database);
		write_database(&database);
	});

	let template = Arc::new(template);

	for stream in listener.incoming() {
		let stream = stream.unwrap();
		let template = Arc::clone(&template);

		pool.execute(move || {
			handle_connection(stream, &template);
		});
	}
}
