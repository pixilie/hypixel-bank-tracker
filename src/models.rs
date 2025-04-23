#![allow(dead_code)]
use askama::Template;
use chrono::{Local, TimeZone};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display};

use crate::Operation;

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Hash, Clone)]
pub(crate) struct Uuid(String);

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Hash, Clone)]
pub(crate) struct Username(String);

impl Username {
	pub(crate) fn new(username: String) -> Self {
		if username.starts_with('§') {
			// § is a two byte character followed by a one byte character
			Self(username[3..].to_string())
		} else {
			Self(username)
		}
	}

	pub(crate) fn as_str(&self) -> &str {
		&self.0
	}
}

impl Display for Username {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct ProfileResponse {
	pub(crate) success: bool,
	pub(crate) profile: Profile,
}

pub(crate) struct Config {
	pub(crate) hypixel_api_key: String,
	pub(crate) profile_uuid: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[expect(clippy::struct_field_names)]
pub(crate) struct Profile {
	pub(crate) profile_id: String,
	pub(crate) community_upgrades: CommunityUpgrades,
	pub(crate) created_at: u128,
	pub(crate) members: HashMap<Uuid, Member>,
	pub(crate) banking: Banking,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub(crate) struct Member {
	pub(crate) leveling: Leveling,
	// non-exhaustive
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub(crate) struct Leveling {
	pub(crate) completed_tasks: Vec<String>,
	// non-exhaustive
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub(crate) struct CommunityUpgrades {
	// non-exhaustive
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub(crate) struct Banking {
	pub(crate) balance: f64,
	pub(crate) transactions: Vec<Transaction>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub(crate) struct Transaction {
	pub(crate) amount: f64,
	pub(crate) timestamp: u128,
	pub(crate) action: TransactionAction,
	pub(crate) initiator_name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub(crate) enum TransactionAction {
	Deposit,
	Withdraw,
}

pub(crate) struct UserBalance<'a> {
	pub(crate) name: &'a Username,
	pub(crate) balance: f64,
}

pub(crate) struct UserDelta<'a> {
	pub(crate) name: &'a Username,
	pub(crate) delta: f64,
}

#[derive(Template)]
#[template(path = "index.html")]
pub(crate) struct BankerTemplate<'a> {
	pub(crate) users: Vec<UserBalance<'a>>,
	pub(crate) operations: &'a [(u128, Operation)],
	pub(crate) deltas: Vec<UserDelta<'a>>,
	pub(crate) bank_interests: f64,
	pub(crate) balance: f64,
	pub(crate) max_balance: String,
	pub(crate) completion_percentage: String,
	pub(crate) last_check_timestamp: u128,
	pub(crate) last_transaction_timestamp: u128,
	pub(crate) drift: f64,
	pub(crate) total_operations: usize,
}

#[expect(clippy::unused_self, reason = "askama template works with methods")]
impl BankerTemplate<'_> {
	fn format_number(&self, n: f64) -> String {
		let rounded = n.round();
		let int_part = rounded.to_string();

		let spaced_int = int_part
			.chars()
			.rev()
			.collect::<Vec<_>>()
			.chunks(3)
			.map(|chunk| chunk.iter().collect::<String>())
			.collect::<Vec<_>>()
			.join(" ")
			.chars()
			.rev()
			.collect::<String>();

		spaced_int
	}

	fn format_timestamp(&self, ts: u128) -> String {
		if let Ok(secs) = i64::try_from(ts / 1000) {
			if let Some(datetime) = Local.timestamp_opt(secs, 0).single() {
				return datetime.format("%H:%M %d/%m").to_string();
			}
		}
		String::from("Invalid timestamp")
	}
}
