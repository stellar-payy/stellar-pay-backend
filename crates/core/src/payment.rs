use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// === Payment

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payment {
    pub id: Uuid,
    pub reference: String,
    // Amounts stay as strings end to end. Stellar represents XLM with
    // fixed 7-decimal precision, and f64 cannot compare two amounts for
    // equality safely.
    pub amount: String,
    pub asset: Asset,
    pub destination: String,
    pub memo: Option<String>,
    pub status: PaymentStatus,
    pub stellar_tx_hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Payment {
    pub fn new(
        amount: String,
        reference: String,
        destination: String,
        memo: Option<String>,
    ) -> Self {
        let now = Utc::now();
        return Payment {
            id: Uuid::new_v4(),
            reference,
            amount,
            asset: Asset::Xlm,
            destination,
            memo,
            status: PaymentStatus::Pending,
            stellar_tx_hash: None,
            created_at: now,
            updated_at: now,
        };
    }
}

// === Asset

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Asset {
    #[serde(rename = "XLM")]
    Xlm,
}

// === Payment status

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentStatus {
    Pending,
    Detected,
    Confirmed,
    Failed,
    Expired,
}

impl PaymentStatus {
    // Only these transitions are valid. Anything else must be rejected by
    // the repository layer before a status update is persisted.
    pub fn can_transition_to(&self, next: &PaymentStatus) -> bool {
        return matches!(
            (self, next),
            (Self::Pending, Self::Detected)
                | (Self::Pending, Self::Expired)
                | (Self::Detected, Self::Confirmed)
                | (Self::Detected, Self::Failed)
        );
    }
}

// === Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_can_transition_to_detected() {
        assert!(PaymentStatus::Pending.can_transition_to(&PaymentStatus::Detected));
    }

    #[test]
    fn detected_cannot_transition_to_pending() {
        assert!(!PaymentStatus::Detected.can_transition_to(&PaymentStatus::Pending));
    }

    #[test]
    fn confirmed_cannot_transition_anywhere() {
        assert!(!PaymentStatus::Confirmed.can_transition_to(&PaymentStatus::Failed));
        assert!(!PaymentStatus::Confirmed.can_transition_to(&PaymentStatus::Expired));
    }

    #[test]
    fn payment_status_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&PaymentStatus::Pending).unwrap(),
            "\"pending\""
        );
    }

    #[test]
    fn asset_serializes_as_uppercase_xlm() {
        assert_eq!(serde_json::to_string(&Asset::Xlm).unwrap(), "\"XLM\"");
    }
}
