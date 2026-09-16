use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use stellar_pay_core::RepositoryError;
use tokio::sync::Mutex;
use uuid::Uuid;

// === Webhook event record

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebhookEventStatus {
    Pending,
    Delivered,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEventRecord {
    pub id: Uuid,
    pub payment_id: Uuid,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub status: WebhookEventStatus,
    pub attempts: i32,
    pub next_attempt_at: Option<DateTime<Utc>>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

fn status_from_db(value: &str) -> Result<WebhookEventStatus, RepositoryError> {
    return match value {
        "pending" => Ok(WebhookEventStatus::Pending),
        "delivered" => Ok(WebhookEventStatus::Delivered),
        "failed" => Ok(WebhookEventStatus::Failed),
        other => Err(RepositoryError::Database(format!(
            "unknown webhook event status: {other}"
        ))),
    };
}

// === Repository trait

#[async_trait]
pub trait WebhookEventRepository: Send + Sync {
    async fn enqueue(
        &self,
        payment_id: Uuid,
        event_type: String,
        payload: serde_json::Value,
    ) -> Result<WebhookEventRecord, RepositoryError>;

    async fn fetch_due(
        &self,
        now: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<WebhookEventRecord>, RepositoryError>;

    async fn mark_delivered(
        &self,
        id: Uuid,
        delivered_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError>;

    async fn mark_failed(
        &self,
        id: Uuid,
        next_attempt_at: Option<DateTime<Utc>>,
    ) -> Result<(), RepositoryError>;

    async fn list_for_payment(
        &self,
        payment_id: Uuid,
    ) -> Result<Vec<WebhookEventRecord>, RepositoryError>;
}

// === In-memory implementation

pub struct InMemoryWebhookEventRepository {
    events: Mutex<HashMap<Uuid, WebhookEventRecord>>,
}

impl InMemoryWebhookEventRepository {
    pub fn new() -> Self {
        return InMemoryWebhookEventRepository {
            events: Mutex::new(HashMap::new()),
        };
    }
}

impl Default for InMemoryWebhookEventRepository {
    fn default() -> Self {
        return InMemoryWebhookEventRepository::new();
    }
}

#[async_trait]
impl WebhookEventRepository for InMemoryWebhookEventRepository {
    async fn enqueue(
        &self,
        payment_id: Uuid,
        event_type: String,
        payload: serde_json::Value,
    ) -> Result<WebhookEventRecord, RepositoryError> {
        let now = Utc::now();

        let record = WebhookEventRecord {
            id: Uuid::new_v4(),
            payment_id,
            event_type,
            payload,
            status: WebhookEventStatus::Pending,
            attempts: 0,
            next_attempt_at: Some(now),
            delivered_at: None,
            created_at: now,
            updated_at: now,
        };

        self.events.lock().await.insert(record.id, record.clone());

        return Ok(record);
    }

    async fn fetch_due(
        &self,
        now: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<WebhookEventRecord>, RepositoryError> {
        let events = self.events.lock().await;

        let mut due: Vec<WebhookEventRecord> = events
            .values()
            .filter(|event| {
                event.status == WebhookEventStatus::Pending
                    && event.next_attempt_at.map(|at| at <= now).unwrap_or(false)
            })
            .cloned()
            .collect();

        due.sort_by_key(|event| event.next_attempt_at);
        due.truncate(limit.max(0) as usize);

        return Ok(due);
    }

    async fn mark_delivered(
        &self,
        id: Uuid,
        delivered_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let mut events = self.events.lock().await;
        let event = events.get_mut(&id).ok_or(RepositoryError::NotFound)?;

        event.status = WebhookEventStatus::Delivered;
        event.delivered_at = Some(delivered_at);
        event.next_attempt_at = None;
        event.updated_at = delivered_at;

        return Ok(());
    }

    async fn mark_failed(
        &self,
        id: Uuid,
        next_attempt_at: Option<DateTime<Utc>>,
    ) -> Result<(), RepositoryError> {
        let mut events = self.events.lock().await;
        let event = events.get_mut(&id).ok_or(RepositoryError::NotFound)?;

        event.attempts += 1;
        event.next_attempt_at = next_attempt_at;
        event.status = if next_attempt_at.is_some() {
            WebhookEventStatus::Pending
        } else {
            WebhookEventStatus::Failed
        };
        event.updated_at = Utc::now();

        return Ok(());
    }

    async fn list_for_payment(
        &self,
        payment_id: Uuid,
    ) -> Result<Vec<WebhookEventRecord>, RepositoryError> {
        let events = self.events.lock().await;

        let mut matching: Vec<WebhookEventRecord> = events
            .values()
            .filter(|event| event.payment_id == payment_id)
            .cloned()
            .collect();

        matching.sort_by_key(|event| event.created_at);

        return Ok(matching);
    }
}

// === Postgres implementation

pub struct PostgresWebhookEventRepository {
    pool: PgPool,
}

impl PostgresWebhookEventRepository {
    pub fn new(pool: PgPool) -> Self {
        return PostgresWebhookEventRepository { pool };
    }
}

fn map_sqlx_error(err: sqlx::Error) -> RepositoryError {
    return RepositoryError::Database(err.to_string());
}

const WEBHOOK_EVENT_COLUMNS: &str = "id, payment_id, event_type, payload, status, attempts, next_attempt_at, delivered_at, created_at, updated_at";

#[derive(sqlx::FromRow)]
struct WebhookEventRow {
    id: Uuid,
    payment_id: Uuid,
    event_type: String,
    payload: serde_json::Value,
    status: String,
    attempts: i32,
    next_attempt_at: Option<DateTime<Utc>>,
    delivered_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<WebhookEventRow> for WebhookEventRecord {
    type Error = RepositoryError;

    fn try_from(row: WebhookEventRow) -> Result<Self, Self::Error> {
        return Ok(WebhookEventRecord {
            id: row.id,
            payment_id: row.payment_id,
            event_type: row.event_type,
            payload: row.payload,
            status: status_from_db(&row.status)?,
            attempts: row.attempts,
            next_attempt_at: row.next_attempt_at,
            delivered_at: row.delivered_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        });
    }
}

#[async_trait]
impl WebhookEventRepository for PostgresWebhookEventRepository {
    async fn enqueue(
        &self,
        payment_id: Uuid,
        event_type: String,
        payload: serde_json::Value,
    ) -> Result<WebhookEventRecord, RepositoryError> {
        let id = Uuid::new_v4();
        let now = Utc::now();

        let query = format!(
            "INSERT INTO webhook_events (id, payment_id, event_type, payload, status, attempts, next_attempt_at, created_at, updated_at)
             VALUES ($1, $2, $3, $4, 'pending', 0, $5, $6, $6)
             RETURNING {WEBHOOK_EVENT_COLUMNS}"
        );

        let row = sqlx::query_as::<_, WebhookEventRow>(&query)
            .bind(id)
            .bind(payment_id)
            .bind(event_type)
            .bind(payload)
            .bind(now)
            .bind(now)
            .fetch_one(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        return WebhookEventRecord::try_from(row);
    }

    async fn fetch_due(
        &self,
        now: DateTime<Utc>,
        limit: i64,
    ) -> Result<Vec<WebhookEventRecord>, RepositoryError> {
        let query = format!(
            "SELECT {WEBHOOK_EVENT_COLUMNS}
             FROM webhook_events
             WHERE status = 'pending' AND next_attempt_at <= $1
             ORDER BY next_attempt_at ASC
             LIMIT $2"
        );

        let rows = sqlx::query_as::<_, WebhookEventRow>(&query)
            .bind(now)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        return rows.into_iter().map(WebhookEventRecord::try_from).collect();
    }

    async fn mark_delivered(
        &self,
        id: Uuid,
        delivered_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "UPDATE webhook_events
             SET status = 'delivered', delivered_at = $1, next_attempt_at = NULL, updated_at = $1
             WHERE id = $2",
        )
        .bind(delivered_at)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        return Ok(());
    }

    async fn mark_failed(
        &self,
        id: Uuid,
        next_attempt_at: Option<DateTime<Utc>>,
    ) -> Result<(), RepositoryError> {
        let status = if next_attempt_at.is_some() {
            "pending"
        } else {
            "failed"
        };

        sqlx::query(
            "UPDATE webhook_events
             SET status = $1, attempts = attempts + 1, next_attempt_at = $2, updated_at = NOW()
             WHERE id = $3",
        )
        .bind(status)
        .bind(next_attempt_at)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(map_sqlx_error)?;

        return Ok(());
    }

    async fn list_for_payment(
        &self,
        payment_id: Uuid,
    ) -> Result<Vec<WebhookEventRecord>, RepositoryError> {
        let query = format!(
            "SELECT {WEBHOOK_EVENT_COLUMNS} FROM webhook_events WHERE payment_id = $1 ORDER BY created_at ASC"
        );

        let rows = sqlx::query_as::<_, WebhookEventRow>(&query)
            .bind(payment_id)
            .fetch_all(&self.pool)
            .await
            .map_err(map_sqlx_error)?;

        return rows.into_iter().map(WebhookEventRecord::try_from).collect();
    }
}
