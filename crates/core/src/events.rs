use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::payment::Payment;

// === Payment event

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PaymentEvent {
    PaymentCreated { payment: Payment },
    PaymentDetected { payment: Payment },
    PaymentConfirmed { payment: Payment },
    PaymentFailed { payment: Payment },
    PaymentExpired { payment: Payment },
}

impl PaymentEvent {
    pub fn payment(&self) -> &Payment {
        return match self {
            PaymentEvent::PaymentCreated { payment }
            | PaymentEvent::PaymentDetected { payment }
            | PaymentEvent::PaymentConfirmed { payment }
            | PaymentEvent::PaymentFailed { payment }
            | PaymentEvent::PaymentExpired { payment } => payment,
        };
    }

    pub fn event_type(&self) -> &'static str {
        return match self {
            PaymentEvent::PaymentCreated { .. } => "payment.created",
            PaymentEvent::PaymentDetected { .. } => "payment.detected",
            PaymentEvent::PaymentConfirmed { .. } => "payment.confirmed",
            PaymentEvent::PaymentFailed { .. } => "payment.failed",
            PaymentEvent::PaymentExpired { .. } => "payment.expired",
        };
    }
}

// === Webhook envelope

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebhookEnvelope {
    pub id: Uuid,
    #[serde(rename = "type")]
    pub event_type: String,
    pub created_at: DateTime<Utc>,
    pub data: serde_json::Value,
}
