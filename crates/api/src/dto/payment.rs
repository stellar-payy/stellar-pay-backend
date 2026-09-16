use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use stellar_pay_core::{Asset, Payment, PaymentStatus};
use stellar_pay_webhooks::{WebhookEventRecord, WebhookEventStatus};
use uuid::Uuid;

// === Requests

#[derive(Debug, Deserialize)]
pub struct CreatePaymentRequest {
    pub amount: String,
    pub reference: String,
    pub destination: Option<String>,
    pub memo: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListPaymentsQuery {
    pub page: Option<u32>,
    pub per_page: Option<u32>,
    pub status: Option<PaymentStatus>,
}

// === Payment responses

#[derive(Debug, Serialize)]
pub struct PaymentResponse {
    pub id: Uuid,
    pub reference: String,
    pub amount: String,
    pub asset: Asset,
    pub destination: String,
    pub memo: Option<String>,
    pub status: PaymentStatus,
    pub stellar_tx_hash: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<Payment> for PaymentResponse {
    fn from(payment: Payment) -> Self {
        return PaymentResponse {
            id: payment.id,
            reference: payment.reference,
            amount: payment.amount,
            asset: payment.asset,
            destination: payment.destination,
            memo: payment.memo,
            status: payment.status,
            stellar_tx_hash: payment.stellar_tx_hash,
            created_at: payment.created_at,
            updated_at: payment.updated_at,
        };
    }
}

#[derive(Debug, Serialize)]
pub struct PaymentStatusResponse {
    pub id: Uuid,
    pub status: PaymentStatus,
}

#[derive(Debug, Serialize)]
pub struct PaymentListResponse {
    pub data: Vec<PaymentResponse>,
    pub page: u32,
    pub per_page: u32,
    pub total: i64,
}

// === Webhook event responses

#[derive(Debug, Serialize)]
pub struct WebhookEventResponse {
    pub id: Uuid,
    pub payment_id: Uuid,
    pub event_type: String,
    pub status: WebhookEventStatus,
    pub attempts: i32,
    pub next_attempt_at: Option<DateTime<Utc>>,
    pub delivered_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl From<WebhookEventRecord> for WebhookEventResponse {
    fn from(record: WebhookEventRecord) -> Self {
        return WebhookEventResponse {
            id: record.id,
            payment_id: record.payment_id,
            event_type: record.event_type,
            status: record.status,
            attempts: record.attempts,
            next_attempt_at: record.next_attempt_at,
            delivered_at: record.delivered_at,
            created_at: record.created_at,
        };
    }
}

#[derive(Debug, Serialize)]
pub struct WebhookEventListResponse {
    pub data: Vec<WebhookEventResponse>,
}
