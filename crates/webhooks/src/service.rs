use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use stellar_pay_core::{PaymentEvent, RepositoryError, WebhookEnvelope};
use uuid::Uuid;

use crate::backoff::next_backoff;
use crate::repository::{WebhookEventRecord, WebhookEventRepository};
use crate::signer::sign_payload;

const SIGNATURE_HEADER: &str = "X-Stellar-Pay-Signature";
const FETCH_BATCH_SIZE: i64 = 50;

// === Webhook service

pub struct WebhookService {
    repository: Arc<dyn WebhookEventRepository>,
    http_client: reqwest::Client,
    webhook_url: String,
    signing_secret: String,
}

impl WebhookService {
    pub fn new(
        repository: Arc<dyn WebhookEventRepository>,
        webhook_url: String,
        signing_secret: String,
    ) -> Self {
        return WebhookService {
            repository,
            http_client: reqwest::Client::new(),
            webhook_url,
            signing_secret,
        };
    }

    pub async fn enqueue_event(&self, event: &PaymentEvent) -> Result<(), RepositoryError> {
        let payment_id = event.payment().id;
        let event_type = event.event_type().to_string();
        let envelope = build_envelope(event);

        let payload = serde_json::to_value(&envelope).map_err(|err| {
            RepositoryError::Database(format!("failed to encode webhook payload: {err}"))
        })?;

        self.repository
            .enqueue(payment_id, event_type, payload)
            .await?;

        return Ok(());
    }

    pub async fn deliver_due(&self) -> Result<(), RepositoryError> {
        let due = self
            .repository
            .fetch_due(Utc::now(), FETCH_BATCH_SIZE)
            .await?;

        for record in due {
            self.deliver_one(record).await;
        }

        return Ok(());
    }

    async fn deliver_one(&self, record: WebhookEventRecord) {
        let body = serde_json::to_vec(&record.payload).unwrap_or_default();
        let signature = sign_payload(&self.signing_secret, &body);
        let header_value = format!("sha256={signature}");

        let result = self
            .http_client
            .post(&self.webhook_url)
            .header(SIGNATURE_HEADER, header_value)
            .header("Content-Type", "application/json")
            .body(body)
            .send()
            .await;

        let delivered = matches!(&result, Ok(response) if response.status().is_success());

        if delivered {
            if let Err(err) = self.repository.mark_delivered(record.id, Utc::now()).await {
                tracing::error!(?err, event_id = %record.id, "failed to mark webhook event delivered");
            }
            return;
        }

        if let Err(err) = &result {
            tracing::warn!(?err, event_id = %record.id, "webhook delivery request failed");
        }

        let attempts_made = (record.attempts + 1) as u32;
        let next_attempt_at =
            next_backoff(attempts_made).map(|delay| Utc::now() + to_chrono_duration(delay));

        if let Err(err) = self
            .repository
            .mark_failed(record.id, next_attempt_at)
            .await
        {
            tracing::error!(?err, event_id = %record.id, "failed to record webhook delivery failure");
        }
    }
}

fn to_chrono_duration(delay: Duration) -> chrono::Duration {
    return chrono::Duration::from_std(delay).unwrap_or_else(|_| chrono::Duration::zero());
}

fn build_envelope(event: &PaymentEvent) -> WebhookEnvelope {
    return WebhookEnvelope {
        id: Uuid::new_v4(),
        event_type: event.event_type().to_string(),
        created_at: Utc::now(),
        data: serde_json::json!({ "payment": event.payment() }),
    };
}
