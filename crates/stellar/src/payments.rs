use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// === Stellar payment

// A single native-XLM payment operation observed on the classic Stellar
// ledger, already filtered down from the raw Horizon payments feed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StellarPayment {
    pub transaction_hash: String,
    pub from: String,
    pub to: String,
    pub amount: String,
    pub asset: String,
    pub memo: Option<String>,
    pub created_at: DateTime<Utc>,
}
