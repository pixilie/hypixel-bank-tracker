use std::{collections::HashMap, fmt::Display};

use askama::Template;
use chrono::{Local, TimeZone};
use dotenvy::var;
use serde::{Deserialize, Serialize};

// Custom models
#[derive(Clone)]
pub(crate) struct Config {
	pub(crate) hypixel_api_key: String,
	pub(crate) profile_uuid: String,
	pub(crate) port: String,
	pub(crate) offset: i64,
}

impl Config {
	pub(crate) fn load() -> Self {
		Self {
			hypixel_api_key: var("HYPIXEL_API_KEY").unwrap(),
			profile_uuid: var("PROFILE_UUID").unwrap(),
			port: var("PORT").unwrap(),
			offset: var("TIME_OFFSET").unwrap().parse::<i64>().unwrap(),
		}
	}
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct DataFile {
	pub(crate) version: u64,
	pub(crate) last_transaction_timestamp: i64,
	pub(crate) last_check_timestamp: i64,
	pub(crate) balance: f64,
	pub(crate) drift: f64,
	pub(crate) max_balance: String,
	pub(crate) bank_interests: f64,
	pub(crate) users: HashMap<Username, f64>,
	pub(crate) operations: Vec<(i64, Operation)>,
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
}

impl Display for Username {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{}", self.0)
	}
}

// Hypixel API related models
#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct ProfileResponse {
	pub(crate) success: bool,
	pub(crate) profile: Profile,
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
	pub(crate) timestamp: i64,
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

// Askama template related models
#[derive(Template)]
#[template(path = "index.html")]
pub(crate) struct BankerTemplate<'a> {
	pub(crate) users: Vec<UserBalance<'a>>,
	pub(crate) operations: &'a [(i64, Operation)],
	pub(crate) deltas: Vec<UserDelta<'a>>,
	pub(crate) bank_interests: f64,
	pub(crate) balance: f64,
	pub(crate) max_balance: String,
	pub(crate) completion_percentage: String,
	pub(crate) last_check_timestamp: i64,
	pub(crate) last_transaction_timestamp: i64,
	pub(crate) drift: f64,
	pub(crate) total_operations: usize,
	pub(crate) offset: i64,
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

	fn format_timestamp(&self, timestamp: i64) -> String {
		Local
			.timestamp_opt((timestamp + self.offset * 3600 * 1000) / 1000, 0)
			.unwrap()
			.format("%H:%M %d/%m")
			.to_string()
	}
}
