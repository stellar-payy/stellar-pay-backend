use std::sync::Arc;

use stellar_pay_core::{Payment, PaymentError, RepositoryError};

use crate::repository::PaymentRepository;

// === Idempotent creation

// Checks by key first if one is supplied so a retried request replays the
// original payment instead of racing the reference-uniqueness constraint.
pub async fn create_with_idempotency(
    repository: &Arc<dyn PaymentRepository>,
    payment: Payment,
    idempotency_key: Option<String>,
) -> Result<Payment, PaymentError> {
    if let Some(key) = idempotency_key.as_deref()
        && let Some(existing) = repository.find_by_idempotency_key(key).await?
    {
        return Ok(existing);
    }

    let reference = payment.reference.clone();

    return match repository
        .create_with_idempotency_key(payment, idempotency_key)
        .await
    {
        Ok(created) => Ok(created),
        Err(RepositoryError::Conflict(_)) => Err(PaymentError::DuplicateReference(reference)),
        Err(other) => Err(PaymentError::from(other)),
    };
}
