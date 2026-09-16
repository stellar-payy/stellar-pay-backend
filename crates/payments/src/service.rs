use std::sync::Arc;

use stellar_pay_core::{Payment, PaymentError, PaymentStatus};
use uuid::Uuid;

use crate::amount::validate_amount;
use crate::idempotency::create_with_idempotency;
use crate::repository::PaymentRepository;

// === Payment service

pub struct PaymentService {
    repository: Arc<dyn PaymentRepository>,
    default_destination: String,
}

impl PaymentService {
    pub fn new(repository: Arc<dyn PaymentRepository>, default_destination: String) -> Self {
        return PaymentService {
            repository,
            default_destination,
        };
    }

    pub async fn create_payment(
        &self,
        amount: String,
        reference: String,
        destination: Option<String>,
        memo: Option<String>,
        idempotency_key: Option<String>,
    ) -> Result<Payment, PaymentError> {
        validate_amount(&amount)?;

        let destination = destination.unwrap_or_else(|| self.default_destination.clone());
        let memo = memo.or_else(|| Some(generate_memo()));

        let payment = Payment::new(amount, reference, destination, memo);

        let created = create_with_idempotency(&self.repository, payment, idempotency_key).await?;

        return Ok(created);
    }

    pub async fn mark_detected(
        &self,
        id: Uuid,
        stellar_tx_hash: String,
    ) -> Result<Payment, PaymentError> {
        return self
            .transition(id, PaymentStatus::Detected, Some(stellar_tx_hash))
            .await;
    }

    pub async fn confirm_payment(&self, id: Uuid) -> Result<Payment, PaymentError> {
        return self.transition(id, PaymentStatus::Confirmed, None).await;
    }

    pub async fn fail_payment(&self, id: Uuid) -> Result<Payment, PaymentError> {
        return self.transition(id, PaymentStatus::Failed, None).await;
    }

    pub async fn expire_payment(&self, id: Uuid) -> Result<Payment, PaymentError> {
        return self.transition(id, PaymentStatus::Expired, None).await;
    }

    pub async fn get_payment(&self, id: Uuid) -> Result<Payment, PaymentError> {
        let payment = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or_else(|| PaymentError::NotFound(id.to_string()))?;

        return Ok(payment);
    }

    pub async fn list_payments(
        &self,
        status_filter: Option<PaymentStatus>,
        page: u32,
        per_page: u32,
    ) -> Result<(Vec<Payment>, i64), PaymentError> {
        let payments = self
            .repository
            .list_paginated(status_filter, page, per_page)
            .await?;
        let total = self.repository.count(status_filter).await?;

        return Ok((payments, total));
    }

    async fn transition(
        &self,
        id: Uuid,
        target: PaymentStatus,
        stellar_tx_hash: Option<String>,
    ) -> Result<Payment, PaymentError> {
        let current = self
            .repository
            .find_by_id(id)
            .await?
            .ok_or_else(|| PaymentError::NotFound(id.to_string()))?;

        if !current.status.can_transition_to(&target) {
            return Err(PaymentError::InvalidTransition {
                from: current.status,
                to: target,
            });
        }

        let updated = self
            .repository
            .update_status(id, target, stellar_tx_hash)
            .await?;

        return Ok(updated);
    }
}

// Stellar MEMO_TEXT caps at 28 bytes, so a full UUID does not fit. Half of a
// v4 UUID's hex digits (64 bits) is enough entropy for a short-lived
// correlation code.
fn generate_memo() -> String {
    let full = Uuid::new_v4().simple().to_string();
    return full[..16].to_string();
}

// === Tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_memo_fits_stellar_memo_text_limit() {
        let memo = generate_memo();
        assert_eq!(memo.len(), 16);
        assert!(memo.len() <= 28);
    }
}
