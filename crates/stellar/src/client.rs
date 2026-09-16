use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use thiserror::Error;

use crate::payments::StellarPayment;
use crate::transactions::{StellarAccount, StellarBalance, StellarTransaction};

// === Errors

#[derive(Debug, Error)]
pub enum StellarError {
    #[error("request failed: {0}")]
    Request(String),

    #[error("resource not found")]
    NotFound,

    #[error("decode error: {0}")]
    Decode(String),
}

// === Client trait

#[async_trait]
pub trait StellarClient: Send + Sync {
    async fn get_transaction(&self, hash: &str) -> Result<StellarTransaction, StellarError>;

    async fn get_account(&self, account_id: &str) -> Result<StellarAccount, StellarError>;

    // Matching is memo-based: a single shared destination account cannot be
    // correlated by (destination, amount, time-window) alone, since two
    // payers could send the same amount around the same time.
    async fn find_payment(
        &self,
        destination: &str,
        memo: &str,
        since: DateTime<Utc>,
    ) -> Result<Option<StellarPayment>, StellarError>;
}

// === Horizon REST DTOs (internal wire shapes, not the domain types)

#[derive(Debug, Deserialize)]
struct HorizonTransaction {
    hash: String,
    successful: bool,
    ledger: u32,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
struct HorizonTransactionDetail {
    memo: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HorizonAccount {
    account_id: String,
    sequence: String,
    balances: Vec<HorizonBalance>,
}

#[derive(Debug, Deserialize)]
struct HorizonBalance {
    balance: String,
    asset_type: String,
}

#[derive(Debug, Deserialize)]
struct HorizonPaymentsPage {
    #[serde(rename = "_embedded")]
    embedded: HorizonEmbedded,
}

#[derive(Debug, Deserialize)]
struct HorizonEmbedded {
    records: Vec<HorizonPaymentRecord>,
}

#[derive(Debug, Deserialize)]
struct HorizonPaymentRecord {
    #[serde(rename = "type")]
    payment_type: String,
    to: Option<String>,
    from: Option<String>,
    amount: Option<String>,
    asset_type: Option<String>,
    transaction_hash: String,
    created_at: DateTime<Utc>,
}

// === Horizon client (real implementation)

pub struct HorizonClient {
    http_client: reqwest::Client,
    base_url: String,
}

impl HorizonClient {
    pub fn new(base_url: String) -> Self {
        return HorizonClient {
            http_client: reqwest::Client::new(),
            base_url,
        };
    }

    async fn get_transaction_memo(&self, hash: &str) -> Result<Option<String>, StellarError> {
        let url = format!("{}/transactions/{}", self.base_url, hash);

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|err| StellarError::Request(err.to_string()))?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(StellarError::NotFound);
        }

        let body: HorizonTransactionDetail = response
            .json()
            .await
            .map_err(|err| StellarError::Decode(err.to_string()))?;

        return Ok(body.memo);
    }
}

#[async_trait]
impl StellarClient for HorizonClient {
    async fn get_transaction(&self, hash: &str) -> Result<StellarTransaction, StellarError> {
        let url = format!("{}/transactions/{}", self.base_url, hash);

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|err| StellarError::Request(err.to_string()))?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(StellarError::NotFound);
        }

        let body: HorizonTransaction = response
            .json()
            .await
            .map_err(|err| StellarError::Decode(err.to_string()))?;

        return Ok(StellarTransaction {
            hash: body.hash,
            successful: body.successful,
            ledger: body.ledger,
            created_at: body.created_at,
        });
    }

    async fn get_account(&self, account_id: &str) -> Result<StellarAccount, StellarError> {
        let url = format!("{}/accounts/{}", self.base_url, account_id);

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|err| StellarError::Request(err.to_string()))?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(StellarError::NotFound);
        }

        let body: HorizonAccount = response
            .json()
            .await
            .map_err(|err| StellarError::Decode(err.to_string()))?;

        let balances = body
            .balances
            .into_iter()
            .map(|balance| StellarBalance {
                asset: balance.asset_type,
                balance: balance.balance,
            })
            .collect();

        return Ok(StellarAccount {
            account_id: body.account_id,
            sequence: body.sequence,
            balances,
        });
    }

    async fn find_payment(
        &self,
        destination: &str,
        memo: &str,
        since: DateTime<Utc>,
    ) -> Result<Option<StellarPayment>, StellarError> {
        let url = format!("{}/accounts/{}/payments", self.base_url, destination);

        let response = self
            .http_client
            .get(&url)
            .query(&[("order", "desc"), ("limit", "50")])
            .send()
            .await
            .map_err(|err| StellarError::Request(err.to_string()))?;

        let body: HorizonPaymentsPage = response
            .json()
            .await
            .map_err(|err| StellarError::Decode(err.to_string()))?;

        for record in body.embedded.records {
            if record.payment_type != "payment" {
                continue;
            }

            if record.created_at < since {
                break;
            }

            if record.to.as_deref() != Some(destination) {
                continue;
            }

            let tx_memo = self.get_transaction_memo(&record.transaction_hash).await?;

            if tx_memo.as_deref() != Some(memo) {
                continue;
            }

            return Ok(Some(StellarPayment {
                transaction_hash: record.transaction_hash,
                from: record.from.unwrap_or_default(),
                to: record.to.unwrap_or_default(),
                amount: record.amount.unwrap_or_default(),
                asset: record.asset_type.unwrap_or_default(),
                memo: tx_memo,
                created_at: record.created_at,
            }));
        }

        return Ok(None);
    }
}

// === Test doubles

#[cfg(feature = "testing")]
pub mod testing {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use async_trait::async_trait;
    use chrono::{DateTime, Utc};

    use super::{StellarClient, StellarError};
    use crate::payments::StellarPayment;
    use crate::transactions::{StellarAccount, StellarTransaction};

    #[derive(Default)]
    pub struct MockStellarClient {
        transactions: Mutex<HashMap<String, StellarTransaction>>,
        payments: Mutex<Vec<StellarPayment>>,
    }

    impl MockStellarClient {
        pub fn new() -> Self {
            return MockStellarClient::default();
        }

        pub fn with_transaction(self, transaction: StellarTransaction) -> Self {
            self.transactions
                .lock()
                .expect("mock transactions lock should not be poisoned")
                .insert(transaction.hash.clone(), transaction);

            return self;
        }

        pub fn with_payment(self, payment: StellarPayment) -> Self {
            self.payments
                .lock()
                .expect("mock payments lock should not be poisoned")
                .push(payment);

            return self;
        }
    }

    #[async_trait]
    impl StellarClient for MockStellarClient {
        async fn get_transaction(&self, hash: &str) -> Result<StellarTransaction, StellarError> {
            let transactions = self
                .transactions
                .lock()
                .expect("mock transactions lock should not be poisoned");

            return transactions
                .get(hash)
                .cloned()
                .ok_or(StellarError::NotFound);
        }

        async fn get_account(&self, _account_id: &str) -> Result<StellarAccount, StellarError> {
            return Err(StellarError::NotFound);
        }

        async fn find_payment(
            &self,
            destination: &str,
            memo: &str,
            since: DateTime<Utc>,
        ) -> Result<Option<StellarPayment>, StellarError> {
            let payments = self
                .payments
                .lock()
                .expect("mock payments lock should not be poisoned");

            let found = payments
                .iter()
                .find(|payment| {
                    payment.to == destination
                        && payment.memo.as_deref() == Some(memo)
                        && payment.created_at >= since
                })
                .cloned();

            return Ok(found);
        }
    }
}
