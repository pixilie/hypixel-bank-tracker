use core::panic;
use std::collections::HashMap;

use crate::models::Profile;

pub(crate) fn get_max_balance(profile: Profile) -> u64 {
	let bank_upgrades: HashMap<&str, u64> = [
		("BANK_UPGRADE_STARTER", 5_000_000),
		("BANK_UPGRADE_GOLD", 100_000_000),
		("BANK_UPGRADE_DELUXE", 250_000_000),
		("BANK_UPGRADE_SUPER_DELUXE", 500_000_000),
		("BANK_UPGRADE_PREMIER", 1_000_000_000),
		("BANK_UPGRADE_LUXURIOUS", 6_000_000_000),
		("BANK_UPGRADE_PALATIAL", 60_000_000_000),
	]
	.iter()
	.cloned()
	.collect();

	let mut active_uuid = match profile.members.keys().next() {
		Some(uuid) => uuid,
		None => panic!("An error occured while retreiving the first member of the coop"),
	};

	profile.members.iter().for_each(|(uuid, member)| {
		member.leveling.completed_tasks.iter().for_each(|task| {
			if bank_upgrades.contains_key(task.as_str()) {
				active_uuid = uuid
			}
		})
	});

	let completed_tasks = match profile.members.get(active_uuid) {
		Some(member) => &member.leveling.completed_tasks,
		None => panic!("No users found in coop"),
	};

	completed_tasks.iter().fold(0, |max_balance, item| {
		bank_upgrades
			.get(item.as_str())
			.map_or(max_balance, |&value| max_balance.max(value))
	})
}
