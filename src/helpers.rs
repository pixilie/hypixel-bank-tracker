use std::{
	collections::HashMap,
	time::{SystemTime, UNIX_EPOCH},
};

use crate::{models::Profile, Operation, Username};

pub(crate) fn get_max_balance(profile: &Profile) -> String {
	let bank_level = HashMap::from([
		("BANK_UPGRADE_STARTER", 5_000_000u64),
		("BANK_UPGRADE_GOLD", 100_000_000u64),
		("BANK_UPGRADE_DELUXE", 250_000_000u64),
		("BANK_UPGRADE_SUPER_DELUXE", 500_000_000u64),
		("BANK_UPGRADE_PREMIER", 1_000_000_000u64),
		("BANK_UPGRADE_LUXURIOUS", 6_000_000_000u64),
		("BANK_UPGRADE_PALATIAL", 60_000_000_000u64),
	]);

	let max_balance = profile
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
		.map(|value| value.to_string());

	max_balance.map_or_else(|| "Unknown".to_string(), |value| value)
}

pub(crate) fn process_user_balance_evolution(
	operations: &[(u128, Operation)],
) -> HashMap<&Username, f64> {
	let current = SystemTime::now()
		.duration_since(UNIX_EPOCH)
		.unwrap()
		.as_millis();

	let recent_transactions = operations
		.iter()
		.skip_while(|(timestamp, _)| current - timestamp > 24 * 3600 * 1000)
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

	recent_transactions
}

pub(crate) fn format_completion_percentage(balance: f64, max_balance: &str) -> String {
	max_balance.parse::<f64>().map_or_else(
		|_| "Unkwow".to_string(),
		|max_balance| format!("{:.2}%", (balance / max_balance) * 100.0),
	)
}
