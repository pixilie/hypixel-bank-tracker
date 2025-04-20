#![allow(dead_code)]
use askama::Template;
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

pub(crate) struct User {
	pub(crate) name: Username,
	pub(crate) balance: f64,
	pub(crate) delta: f64,
}

const ZERO_REF: &f64 = &0.0;
const FIVE_M_REF: &f64 = &5_000_000.0;

#[derive(Template)]
#[template(path = "index.html")]
pub(crate) struct BankerTemplate {
	pub(crate) users: Vec<User>,
	pub(crate) operations: Vec<(u128, Operation)>,
	pub(crate) bank_interests: f64,
	pub(crate) balance: f64,
	pub(crate) max_balance: u64,
	pub(crate) completion_percentage: String,
	pub(crate) last_check_timestamp: u128,
	pub(crate) last_transaction_timestamp: u128,
	pub(crate) drift: f64,
	pub(crate) total_operations: usize,
}
