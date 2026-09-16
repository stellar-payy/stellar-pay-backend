use std::sync::Arc;

use stellar_pay_core::{PaymentError, PaymentStatus};
use stellar_pay_payments::{InMemoryPaymentRepository, PaymentService};

const DESTINATION: &str = "GDESTINATIONPLACEHOLDERXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX";

fn service() -> PaymentService {
    let repository = Arc::new(InMemoryPaymentRepository::new());
    return PaymentService::new(repository, DESTINATION.to_string());
}

#[tokio::test]
async fn create_payment_happy_path() {
    let service = service();

    let payment = service
        .create_payment(
            "10.0000000".to_string(),
            "order-1".to_string(),
            None,
            None,
            None,
        )
        .await
        .expect("payment should be created");

    assert_eq!(payment.reference, "order-1");
    assert_eq!(payment.destination, DESTINATION);
    assert_eq!(payment.status, PaymentStatus::Pending);
    assert!(payment.memo.is_some());
}

#[tokio::test]
async fn create_payment_rejects_duplicate_reference() {
    let service = service();

    service
        .create_payment("10".to_string(), "order-2".to_string(), None, None, None)
        .await
        .expect("first payment should be created");

    let result = service
        .create_payment("20".to_string(), "order-2".to_string(), None, None, None)
        .await;

    assert!(matches!(result, Err(PaymentError::DuplicateReference(_))));
}

#[tokio::test]
async fn create_payment_rejects_invalid_amount() {
    let service = service();

    let result = service
        .create_payment("-5".to_string(), "order-3".to_string(), None, None, None)
        .await;

    assert!(matches!(result, Err(PaymentError::InvalidAmount(_))));
}

#[tokio::test]
async fn confirm_requires_detected_first() {
    let service = service();

    let payment = service
        .create_payment("10".to_string(), "order-4".to_string(), None, None, None)
        .await
        .expect("payment should be created");

    let result = service.confirm_payment(payment.id).await;

    assert!(matches!(
        result,
        Err(PaymentError::InvalidTransition { .. })
    ));
}

#[tokio::test]
async fn full_lifecycle_transitions() {
    let service = service();

    let payment = service
        .create_payment("10".to_string(), "order-5".to_string(), None, None, None)
        .await
        .expect("payment should be created");

    let detected = service
        .mark_detected(payment.id, "tx-hash".to_string())
        .await
        .expect("payment should transition to detected");
    assert_eq!(detected.status, PaymentStatus::Detected);
    assert_eq!(detected.stellar_tx_hash.as_deref(), Some("tx-hash"));

    let confirmed = service
        .confirm_payment(payment.id)
        .await
        .expect("payment should transition to confirmed");
    assert_eq!(confirmed.status, PaymentStatus::Confirmed);
}
