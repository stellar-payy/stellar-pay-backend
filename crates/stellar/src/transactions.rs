use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// === Stellar transaction

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StellarTransaction {
    pub hash: String,
    pub successful: bool,
    pub ledger: u32,
    pub created_at: DateTime<Utc>,
}

// === Stellar account

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StellarAccount {
    pub account_id: String,
    pub sequence: String,
    pub balances: Vec<StellarBalance>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StellarBalance {
    pub asset: String,
    pub balance: String,
}
