use std::sync::Arc;

use stellar_pay_payments::{InMemoryPaymentRepository, PaymentService};

const DESTINATION: &str = "GDESTINATIONPLACEHOLDERXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX";

fn service() -> PaymentService {
    let repository = Arc::new(InMemoryPaymentRepository::new());
    return PaymentService::new(repository, DESTINATION.to_string());
}

#[tokio::test]
async fn same_idempotency_key_returns_same_payment() {
    let service = service();

    let first = service
        .create_payment(
            "10".to_string(),
            "order-idem-1".to_string(),
            None,
            None,
            Some("key-1".to_string()),
        )
        .await
        .expect("first call should create a payment");

    let second = service
        .create_payment(
            "10".to_string(),
            "order-idem-1-again".to_string(),
            None,
            None,
            Some("key-1".to_string()),
        )
        .await
        .expect("second call should replay the same payment");

    assert_eq!(first.id, second.id);
    assert_eq!(first.reference, second.reference);
}

#[tokio::test]
async fn distinct_idempotency_keys_create_distinct_payments() {
    let service = service();

    let first = service
        .create_payment(
            "10".to_string(),
            "order-idem-2".to_string(),
            None,
            None,
            Some("key-a".to_string()),
        )
        .await
        .expect("payment should be created");

    let second = service
        .create_payment(
            "10".to_string(),
            "order-idem-3".to_string(),
            None,
            None,
            Some("key-b".to_string()),
        )
        .await
        .expect("payment should be created");

    assert_ne!(first.id, second.id);
}

#[tokio::test]
async fn missing_idempotency_key_does_not_dedupe() {
    let service = service();

    let first = service
        .create_payment(
            "10".to_string(),
            "order-idem-4".to_string(),
            None,
            None,
            None,
        )
        .await
        .expect("payment should be created");

    let second = service
        .create_payment(
            "10".to_string(),
            "order-idem-5".to_string(),
            None,
            None,
            None,
        )
        .await
        .expect("payment should be created");

    assert_ne!(first.id, second.id);
}

// Requires `docker compose up -d` and `cargo run -p stellar-pay-cli --
// migrate` beforehand, then `cargo test -- --ignored`.
#[ignore]
#[tokio::test]
async fn postgres_repository_enforces_idempotency_key_uniqueness() {
    use sqlx::postgres::PgPoolOptions;
    use stellar_pay_payments::PostgresPaymentRepository;

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/stellar_pay".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("database should be reachable");

    let repository = Arc::new(PostgresPaymentRepository::new(pool));
    let service = PaymentService::new(repository, DESTINATION.to_string());

    let key = format!("pg-key-{}", uuid::Uuid::new_v4());

    let first = service
        .create_payment(
            "10".to_string(),
            format!("pg-order-{}", uuid::Uuid::new_v4()),
            None,
            None,
            Some(key.clone()),
        )
        .await
        .expect("payment should be created against postgres");

    let second = service
        .create_payment(
            "20".to_string(),
            format!("pg-order-{}", uuid::Uuid::new_v4()),
            None,
            None,
            Some(key),
        )
        .await
        .expect("replay should return the original payment");

    assert_eq!(first.id, second.id);
}
