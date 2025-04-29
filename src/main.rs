#![allow(dead_code)]

use askama::Template;
use chrono::Local;
use core::time;
use helpers::{format_completion_percentage, get_max_balance, process_user_balance_evolution};
use models::{
	BankerTemplate, Banking, Config, DataFile, Operation, Profile, ProfileResponse, Transaction,
	UserBalance, UserDelta, Username,
};
use parking_lot::Mutex;
use reqwest::blocking::Client;
use rouille::{router, Response};
use std::{
	fs::{self},
	sync::Arc,
	thread,
};
use url::Url;

mod helpers;
mod models;

const DB_FILE: &str = "data.json";
const DB_VERSION: u64 = 3;

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

	database.last_check_timestamp = Local::now().timestamp_millis();

	response.profile
}

fn update_transaction(profile: &Profile, database: &mut DataFile) {
	println!("TSC: Updating...");

	let banking: Banking = profile.clone().banking;

	let mut new_transactions = banking
		.transactions
		.into_iter()
		.skip_while(|transaction| transaction.timestamp <= database.last_transaction_timestamp)
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
		new_transactions.push((Local::now().timestamp_millis(), Operation::WeirdWaypoint));
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

#[expect(clippy::significant_drop_tightening)]
fn generate_template(database: &Arc<Mutex<DataFile>>, config: &Config) -> String {
	let database = database.lock();

	let mut users = database
		.users
		.iter()
		.map(|(username, balance)| UserBalance {
			name: username,
			balance: *balance,
		})
		.collect::<Vec<_>>();
	users.sort_by(|a, b| b.balance.total_cmp(&a.balance));

	let mut deltas = process_user_balance_evolution(&database.operations)
		.iter()
		.filter(|(_, delta)| **delta != 0.0)
		.map(|(name, delta)| UserDelta {
			name,
			delta: *delta,
		})
		.collect::<Vec<_>>();
	deltas.sort_by(|a, b| b.delta.total_cmp(&a.delta));

	let template = BankerTemplate {
		users,
		operations: database
			.operations
			.get(database.operations.len() - 25..)
			.unwrap_or(&database.operations[..]),
		deltas,
		bank_interests: database.bank_interests,
		balance: database.balance,
		max_balance: database.max_balance.clone(),
		completion_percentage: format_completion_percentage(
			database.balance,
			&database.max_balance,
		),
		last_check_timestamp: database.last_check_timestamp,
		last_transaction_timestamp: database.last_transaction_timestamp,
		drift: database.drift,
		total_operations: database.operations.len(),
		offset: config.offset,
	};

	template.render().unwrap()
}

#[expect(clippy::significant_drop_tightening)]
fn spawn_fetch_thread(config: Config, database: Arc<Mutex<DataFile>>, client: Client) {
	thread::spawn(move || loop {
		{
			let mut locked_database = database.lock();
			let new_profile = fetch_api(&config, &client, &mut locked_database);

			update_transaction(&new_profile, &mut locked_database);
			write_database(&locked_database);
		}

		thread::sleep(time::Duration::from_secs(600));
	});
}

fn main() {
	let config = Config::load();
	let database = Arc::new(Mutex::new(load_database(DB_FILE)));
	let client = reqwest::blocking::Client::new();

	spawn_fetch_thread(config.clone(), database.clone(), client);

	rouille::start_server("127.0.0.1:7878", move |request| {
		if let Some(request) = request.remove_prefix("/static") {
			return rouille::match_assets(&request, "static");
		}

		let response = router!(request,
			(GET) (/) => {
				Response::from_data("text/html", generate_template(&database, &config))
			},
			(POST) (/) => {
				Response::empty_404()
			},
			_ => Response::empty_404()
		);
		response
	});
}
