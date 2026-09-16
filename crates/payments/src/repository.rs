use std::collections::HashMap;
use std::str::FromStr;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use stellar_pay_core::{Asset, Payment, PaymentStatus, RepositoryError};
use tokio::sync::Mutex;
use uuid::Uuid;

// === Transition guard

// The single source of truth for which transitions are legal is
// `PaymentStatus::can_transition_to`. This derives the set of "from" states
// that are valid for a given target, so both repository implementations
// share one atomic guard instead of duplicating the state machine.
const ALL_STATUSES: [PaymentStatus; 5] = [
    PaymentStatus::Pending,
    PaymentStatus::Detected,
    PaymentStatus::Confirmed,
    PaymentStatus::Failed,
    PaymentStatus::Expired,
];

fn valid_from_states(target: PaymentStatus) -> Vec<PaymentStatus> {
    return ALL_STATUSES
        .into_iter()
        .filter(|from| from.can_transition_to(&target))
        .collect();
}

fn status_to_db(status: PaymentStatus) -> &'static str {
    return match status {
        PaymentStatus::Pending => "pending",
        PaymentStatus::Detected => "detected",
        PaymentStatus::Confirmed => "confirmed",
        PaymentStatus::Failed => "failed",
        PaymentStatus::Expired => "expired",
    };
}

fn status_from_db(value: &str) -> Result<PaymentStatus, RepositoryError> {
    return match value {
        "pending" => Ok(PaymentStatus::Pending),
        "detected" => Ok(PaymentStatus::Detected),
        "confirmed" => Ok(PaymentStatus::Confirmed),
        "failed" => Ok(PaymentStatus::Failed),
        "expired" => Ok(PaymentStatus::Expired),
        other => Err(RepositoryError::Database(format!(
            "unknown payment status: {other}"
        ))),
    };
}

// === Repository trait

#[async_trait]
pub trait PaymentRepository: Send + Sync {
    async fn create_with_idempotency_key(
        &self,
        payment: Payment,
        idempotency_key: Option<String>,
    ) -> Result<Payment, RepositoryError>;

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Payment>, RepositoryError>;

    async fn find_by_reference(&self, reference: &str) -> Result<Option<Payment>, RepositoryError>;

    async fn find_by_idempotency_key(&self, key: &str) -> Result<Option<Payment>, RepositoryError>;

    async fn update_status(
        &self,
        id: Uuid,
        next: PaymentStatus,
        stellar_tx_hash: Option<String>,
    ) -> Result<Payment, RepositoryError>;

    async fn list_pending_for_reconciliation(&self) -> Result<Vec<Payment>, RepositoryError>;

    async fn list_paginated(
        &self,
        status_filter: Option<PaymentStatus>,
        page: u32,
        per_page: u32,
    ) -> Result<Vec<Payment>, RepositoryError>;

    async fn count(&self, status_filter: Option<PaymentStatus>) -> Result<i64, RepositoryError>;
}

// === In-memory implementation

#[derive(Default)]
struct InMemoryState {
    payments: HashMap<Uuid, Payment>,
    idempotency_keys: HashMap<String, Uuid>,
}

pub struct InMemoryPaymentRepository {
    state: Mutex<InMemoryState>,
}

impl InMemoryPaymentRepository {
    pub fn new() -> Self {
        return InMemoryPaymentRepository {
            state: Mutex::new(InMemoryState::default()),
        };
    }
}

impl Default for InMemoryPaymentRepository {
    fn default() -> Self {
        return InMemoryPaymentRepository::new();
    }
}

#[async_trait]
impl PaymentRepository for InMemoryPaymentRepository {
    async fn create_with_idempotency_key(
        &self,
        payment: Payment,
        idempotency_key: Option<String>,
    ) -> Result<Payment, RepositoryError> {
        let mut state = self.state.lock().await;

        if state
            .payments
            .values()
            .any(|existing| existing.reference == payment.reference)
        {
            return Err(RepositoryError::Conflict(format!(
                "reference already exists: {}",
                payment.reference
            )));
        }

        state.payments.insert(payment.id, payment.clone());

        if let Some(key) = idempotency_key {
            state.idempotency_keys.insert(key, payment.id);
        }

        return Ok(payment);
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Payment>, RepositoryError> {
        let state = self.state.lock().await;
        return Ok(state.payments.get(&id).cloned());
    }

    async fn find_by_reference(&self, reference: &str) -> Result<Option<Payment>, RepositoryError> {
        let state = self.state.lock().await;
        return Ok(state
            .payments
            .values()
            .find(|payment| payment.reference == reference)
            .cloned());
    }

    async fn find_by_idempotency_key(&self, key: &str) -> Result<Option<Payment>, RepositoryError> {
        let state = self.state.lock().await;

        let Some(id) = state.idempotency_keys.get(key) else {
            return Ok(None);
        };

        return Ok(state.payments.get(id).cloned());
    }

    async fn update_status(
        &self,
        id: Uuid,
        next: PaymentStatus,
        stellar_tx_hash: Option<String>,
    ) -> Result<Payment, RepositoryError> {
        let mut state = self.state.lock().await;
        let allowed = valid_from_states(next);

        let payment = state
            .payments
            .get_mut(&id)
            .ok_or(RepositoryError::NotFound)?;

        if !allowed.contains(&payment.status) {
            return Err(RepositoryError::Conflict(format!(
                "cannot transition payment {id} to {next:?}"
            )));
        }

        payment.status = next;
        payment.updated_at = Utc::now();

        if let Some(tx_hash) = stellar_tx_hash {
            payment.stellar_tx_hash = Some(tx_hash);
        }

        return Ok(payment.clone());
    }

    async fn list_pending_for_reconciliation(&self) -> Result<Vec<Payment>, RepositoryError> {
        let state = self.state.lock().await;

        let mut pending: Vec<Payment> = state
            .payments
            .values()
            .filter(|payment| {
                matches!(
                    payment.status,
                    PaymentStatus::Pending | PaymentStatus::Detected
                )
            })
            .cloned()
            .collect();

        pending.sort_by_key(|payment| payment.created_at);

        return Ok(pending);
    }

    async fn list_paginated(
        &self,
        status_filter: Option<PaymentStatus>,
        page: u32,
        per_page: u32,
    ) -> Result<Vec<Payment>, RepositoryError> {
        let state = self.state.lock().await;

        let mut matching: Vec<Payment> = state
            .payments
            .values()
            .filter(|payment| match status_filter {
                Some(status) => payment.status == status,
                None => true,
            })
            .cloned()
            .collect();

        matching.sort_by_key(|payment| std::cmp::Reverse(payment.created_at));

        let offset = ((page.max(1) - 1) * per_page) as usize;

        return Ok(matching
            .into_iter()
            .skip(offset)
            .take(per_page as usize)
            .collect());
    }

    async fn count(&self, status_filter: Option<PaymentStatus>) -> Result<i64, RepositoryError> {
        let state = self.state.lock().await;

        let total = state
            .payments
            .values()
            .filter(|payment| match status_filter {
                Some(status) => payment.status == status,
                None => true,
            })
            .count();

        return Ok(total as i64);
    }
}

// === Postgres implementation

pub struct PostgresPaymentRepository {
    pool: PgPool,
}

impl PostgresPaymentRepository {
    pub fn new(pool: PgPool) -> Self {
        return PostgresPaymentRepository { pool };
    }
}

fn map_sqlx_error(err: sqlx::Error) -> RepositoryError {
    if let sqlx::Error::Database(db_err) = &err
        && db_err.code().as_deref() == Some("23505")
    {
        return RepositoryError::Conflict(db_err.message().to_string());
    }

    return RepositoryError::Database(err.to_string());
}

const PAYMENT_COLUMNS: &str = "id, reference, amount, asset, destination, memo, status, stellar_tx_hash, created_at, updated_at";

#[derive(sqlx::FromRow)]
struct PaymentRow {
    id: Uuid,
    reference: String,
    amount: Decimal,
    asset: String,
    destination: String,
    memo: Option<String>,
    status: String,
    stellar_tx_hash: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<PaymentRow> for Payment {
    type Error = RepositoryError;

    fn try_from(row: PaymentRow) -> Result<Self, Self::Error> {
        if row.asset != "XLM" {
            return Err(RepositoryError::Database(format!(
                "unknown asset: {}",
                row.asset
            )));
        }

        return Ok(Payment {
            id: row.id,
            reference: row.reference,
            amount: row.amount.normalize().to_string(),
            asset: Asset::Xlm,
            destination: row.destination,
            memo: row.memo,
            status: status_from_db(&row.status)?,
            stellar_tx_hash: row.stellar_tx_hash,
            created_at: row.created_at,
            updated_at: row.updated_at,
        });
    }
}

#[async_trait]
impl PaymentRepository for PostgresPaymentRepository {
    async fn create_with_idempotency_key(
        &self,
        payment: Payment,
        idempotency_key: Option<String>,
    ) -> Result<Payment, RepositoryError> {
        let amount = Decimal::from_str(&payment.amount)
            .map_err(|err| RepositoryError::Database(format!("invalid amount: {err}")))?;

        let mut tx = self.pool.begin().await.map_err(map_sqlx_error)?;

        let query = format!(
            "INSERT INTO payments (id, reference, amount, asset, destination, memo, status, stellar_tx_hash, created_at, updated_at)
             VALUES ($1, $2, $3, 'XLM', $4, $5, $6, $7, $8, $9)
             RETURNING {PAYMENT_COLUMNS}"
        );

        let row = sqlx::query_as::<_, PaymentRow>(&query)
            .bind(payment.id)
            .bind(&payment.reference)
            .bind(amount)
            .bind(&payment.destination)
            .bind(&payment.memo)
            .bind(status_to_db(payment.status))
            .bind(&payment.stellar_tx_hash)
            .bind(payment.created_at)
            .bind(payment.updated_at)
            .fetch_one(&mut *tx)
            .await
            .map_err(map_sqlx_error)?;

        if let Some(key) = idempotency_key {
            sqlx::query("INSERT INTO idempotency_keys (key, payment_id) VALUES ($1, $2)")
                .bind(key)
                .bind(payment.id)
                .execute(&mut *tx)
                .await
                .map_err(map_sqlx_error)?;
        }

        tx.commit().await.map_err(map_sqlx_error)?;

        return Payment::try_from(row);
    }

    async fn find_by_id(&self, id: Uuid) -> Result<Option<Payment>, RepositoryError> {
        let query = format!("SELECT {PAYMENT_COLUMNS} FROM payments WHERE id = $1");

        let row = sqlx::query_as::<_, PaymentRow>(&query)
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        return row.map(Payment::try_from).transpose();
    }

    async fn find_by_reference(&self, reference: &str) -> Result<Option<Payment>, RepositoryError> {
        let query = format!("SELECT {PAYMENT_COLUMNS} FROM payments WHERE reference = $1");

        let row = sqlx::query_as::<_, PaymentRow>(&query)
            .bind(reference)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        return row.map(Payment::try_from).transpose();
    }

    async fn find_by_idempotency_key(&self, key: &str) -> Result<Option<Payment>, RepositoryError> {
        let query = "SELECT p.id, p.reference, p.amount, p.asset, p.destination, p.memo, p.status, p.stellar_tx_hash, p.created_at, p.updated_at
             FROM payments p
             JOIN idempotency_keys k ON k.payment_id = p.id
             WHERE k.key = $1";

        let row = sqlx::query_as::<_, PaymentRow>(query)
            .bind(key)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        return row.map(Payment::try_from).transpose();
    }

    async fn update_status(
        &self,
        id: Uuid,
        next: PaymentStatus,
        stellar_tx_hash: Option<String>,
    ) -> Result<Payment, RepositoryError> {
        let allowed: Vec<String> = valid_from_states(next)
            .into_iter()
            .map(|status| status_to_db(status).to_string())
            .collect();

        let query = format!(
            "UPDATE payments
             SET status = $1, stellar_tx_hash = COALESCE($2, stellar_tx_hash), updated_at = NOW()
             WHERE id = $3 AND status = ANY($4)
             RETURNING {PAYMENT_COLUMNS}"
        );

        let row = sqlx::query_as::<_, PaymentRow>(&query)
            .bind(status_to_db(next))
            .bind(stellar_tx_hash)
            .bind(id)
            .bind(&allowed)
            .fetch_optional(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        if let Some(row) = row {
            return Payment::try_from(row);
        }

        let exists = self.find_by_id(id).await?;

        return match exists {
            Some(_) => Err(RepositoryError::Conflict(format!(
                "cannot transition payment {id} to {next:?}"
            ))),
            None => Err(RepositoryError::NotFound),
        };
    }

    async fn list_pending_for_reconciliation(&self) -> Result<Vec<Payment>, RepositoryError> {
        let query = format!(
            "SELECT {PAYMENT_COLUMNS} FROM payments WHERE status IN ('pending', 'detected') ORDER BY created_at ASC"
        );

        let rows = sqlx::query_as::<_, PaymentRow>(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        return rows.into_iter().map(Payment::try_from).collect();
    }

    async fn list_paginated(
        &self,
        status_filter: Option<PaymentStatus>,
        page: u32,
        per_page: u32,
    ) -> Result<Vec<Payment>, RepositoryError> {
        let offset = ((page.max(1) - 1) * per_page) as i64;
        let limit = per_page as i64;

        let rows = match status_filter {
            Some(status) => {
                let query = format!(
                    "SELECT {PAYMENT_COLUMNS} FROM payments WHERE status = $1 ORDER BY created_at DESC OFFSET $2 LIMIT $3"
                );
                sqlx::query_as::<_, PaymentRow>(&query)
                    .bind(status_to_db(status))
                    .bind(offset)
                    .bind(limit)
                    .fetch_all(&self.pool)
                    .await
            }
            None => {
                let query = format!(
                    "SELECT {PAYMENT_COLUMNS} FROM payments ORDER BY created_at DESC OFFSET $1 LIMIT $2"
                );
                sqlx::query_as::<_, PaymentRow>(&query)
                    .bind(offset)
                    .bind(limit)
                    .fetch_all(&self.pool)
                    .await
            }
        }
        .map_err(map_sqlx_error)?;

        return rows.into_iter().map(Payment::try_from).collect();
    }

    async fn count(&self, status_filter: Option<PaymentStatus>) -> Result<i64, RepositoryError> {
        let total: (i64,) = match status_filter {
            Some(status) => {
                sqlx::query_as("SELECT COUNT(*) FROM payments WHERE status = $1")
                    .bind(status_to_db(status))
                    .fetch_one(&self.pool)
                    .await
            }
            None => {
                sqlx::query_as("SELECT COUNT(*) FROM payments")
                    .fetch_one(&self.pool)
                    .await
            }
        }
        .map_err(map_sqlx_error)?;

        return Ok(total.0);
    }
}
