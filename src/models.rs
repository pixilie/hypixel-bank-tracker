#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display};

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
