use std::{
	collections::HashMap,
	io::{BufRead, BufReader, Write},
	net::TcpStream,
	time::{SystemTime, UNIX_EPOCH},
};

use crate::{models::Profile, Operation, Username};

pub(crate) fn get_max_balance(profile: &Profile) -> Option<u64> {
	let mut bank_level = HashMap::new();
	// bank_level.from([

	// ]);
	bank_level.insert("BANK_UPGRADE_STARTER", 5_000_000);
	bank_level.insert("BANK_UPGRADE_GOLD", 100_000_000);
	bank_level.insert("BANK_UPGRADE_DELUXE", 250_000_000);
	bank_level.insert("BANK_UPGRADE_SUPER_DELUXE", 500_000_000);
	bank_level.insert("BANK_UPGRADE_PREMIER", 1_000_000_000);
	bank_level.insert("BANK_UPGRADE_LUXURIOUS", 6_000_000_000);
	bank_level.insert("BANK_UPGRADE_PALATIAL", 60_000_000_000);

	profile
		.members
		.values()
		.filter_map(|member| {
			member
				.leveling
				.completed_tasks
				.iter()
				.filter_map(|task| bank_level.get(task.as_str()))
				.max()
		})
		.max()
		.copied()
}

fn process_user_balance_evolution(operations: Vec<(u128, Operation)>) -> Vec<(Username, f64)> {
	let current = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.unwrap()
		.as_millis();

	let recent_transactions = operations
		.into_iter()
		.take_while(|(timestamp, _)| current - timestamp > 24 * 3600 * 1000)
		.fold(HashMap::new(), |mut delta_hm, (_, operation)| {
			match operation {
				Operation::PlayerPurse {
					amount, username, ..
				} => {
					*delta_hm.entry(username).or_insert(0.0) += amount;
				}
				Operation::PlayerTransfer {
					amount,
					receiver,
					sender,
					..
				} => {
					*delta_hm.entry(receiver).or_insert(0.0) += amount;
					*delta_hm.entry(sender).or_insert(0.0) -= amount;
				}
				_ => {}
			}
			delta_hm
		});

	recent_transactions.into_iter().collect()
}

pub(crate) fn handle_connection(mut stream: TcpStream) {
	let _ = BufReader::new(&stream)
		.lines()
		.map(|result| result.unwrap())
		.take_while(|line| !line.is_empty())
		.collect::<Vec<_>>();

	let response = "HTTP/1.1 200 OK\r\n\r\n";
	stream.write_all(response.as_bytes()).unwrap();
}
