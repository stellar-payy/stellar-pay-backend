use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use stellar_pay_core::PaymentStatus;
use stellar_pay_onchain::NullOnChainRecorder;
use stellar_pay_payments::{InMemoryPaymentRepository, PaymentRepository, PaymentService};
use stellar_pay_reconciliation::ReconciliationWorker;
use stellar_pay_stellar::StellarPayment;
use stellar_pay_stellar::testing::MockStellarClient;
use stellar_pay_webhooks::{InMemoryWebhookEventRepository, WebhookService};

const DESTINATION: &str = "GDESTINATIONPLACEHOLDERXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX";

fn webhook_service() -> Arc<WebhookService> {
    let repository = Arc::new(InMemoryWebhookEventRepository::new());
    return Arc::new(WebhookService::new(
        repository,
        "http://localhost/webhook".to_string(),
        "test-secret".to_string(),
    ));
}

#[tokio::test]
async fn run_once_detects_matching_payment() {
    let repository = Arc::new(InMemoryPaymentRepository::new());
    let service = PaymentService::new(repository.clone(), DESTINATION.to_string());

    let payment = service
        .create_payment(
            "10".to_string(),
            "recon-order-1".to_string(),
            None,
            Some("memo123".to_string()),
            None,
        )
        .await
        .expect("payment should be created");

    let stellar_payment = StellarPayment {
        transaction_hash: "tx-1".to_string(),
        from: "GSENDER".to_string(),
        to: DESTINATION.to_string(),
        amount: "10".to_string(),
        asset: "native".to_string(),
        memo: Some("memo123".to_string()),
        created_at: Utc::now(),
    };

    let stellar_client = Arc::new(MockStellarClient::new().with_payment(stellar_payment));
    let onchain_recorder = Arc::new(NullOnChainRecorder::new());

    let worker = ReconciliationWorker::new(
        repository.clone(),
        stellar_client,
        webhook_service(),
        onchain_recorder,
        Duration::from_secs(3600),
    );

    worker
        .run_once()
        .await
        .expect("reconciliation pass should succeed");

    let updated = repository
        .find_by_id(payment.id)
        .await
        .expect("lookup should succeed")
        .expect("payment should exist");

    assert_eq!(updated.status, PaymentStatus::Detected);
    assert_eq!(updated.stellar_tx_hash.as_deref(), Some("tx-1"));
}

#[tokio::test]
async fn run_once_leaves_unmatched_payment_pending() {
    let repository = Arc::new(InMemoryPaymentRepository::new());
    let service = PaymentService::new(repository.clone(), DESTINATION.to_string());

    let payment = service
        .create_payment(
            "10".to_string(),
            "recon-order-2".to_string(),
            None,
            Some("memo456".to_string()),
            None,
        )
        .await
        .expect("payment should be created");

    let stellar_client = Arc::new(MockStellarClient::new());
    let onchain_recorder = Arc::new(NullOnChainRecorder::new());

    let worker = ReconciliationWorker::new(
        repository.clone(),
        stellar_client,
        webhook_service(),
        onchain_recorder,
        Duration::from_secs(3600),
    );

    worker
        .run_once()
        .await
        .expect("reconciliation pass should succeed");

    let updated = repository
        .find_by_id(payment.id)
        .await
        .expect("lookup should succeed")
        .expect("payment should exist");

    assert_eq!(updated.status, PaymentStatus::Pending);
}

#[tokio::test]
async fn run_once_expires_stale_pending_payment() {
    let repository = Arc::new(InMemoryPaymentRepository::new());
    let service = PaymentService::new(repository.clone(), DESTINATION.to_string());

    let payment = service
        .create_payment(
            "10".to_string(),
            "recon-order-3".to_string(),
            None,
            Some("memo789".to_string()),
            None,
        )
        .await
        .expect("payment should be created");

    let stellar_client = Arc::new(MockStellarClient::new());
    let onchain_recorder = Arc::new(NullOnChainRecorder::new());

    // A zero-duration expiry window makes every pending payment immediately
    // stale, exercising the expiry branch without needing to backdate
    // `created_at`.
    let worker = ReconciliationWorker::new(
        repository.clone(),
        stellar_client,
        webhook_service(),
        onchain_recorder,
        Duration::from_secs(0),
    );

    worker
        .run_once()
        .await
        .expect("reconciliation pass should succeed");

    let updated = repository
        .find_by_id(payment.id)
        .await
        .expect("lookup should succeed")
        .expect("payment should exist");

    assert_eq!(updated.status, PaymentStatus::Expired);
}
