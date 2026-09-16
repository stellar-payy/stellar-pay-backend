use std::sync::Arc;
use std::time::Duration;

use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;

use stellar_pay_onchain::NullOnChainRecorder;
use stellar_pay_payments::{PaymentService, PostgresPaymentRepository};
use stellar_pay_reconciliation::ReconciliationWorker;
use stellar_pay_stellar::HorizonClient;
use stellar_pay_webhooks::{PostgresWebhookEventRepository, WebhookService, run_delivery_loop};

use crate::routes::build_router;
use crate::state::AppState;

const DEFAULT_BIND_ADDRESS: &str = "0.0.0.0:3000";
const RECONCILIATION_INTERVAL: Duration = Duration::from_secs(30);
const WEBHOOK_DELIVERY_INTERVAL: Duration = Duration::from_secs(15);
const RECONCILIATION_EXPIRY_WINDOW: Duration = Duration::from_secs(60 * 60);

// === Server configuration

pub struct ServerConfig {
    pub database_url: String,
    pub bind_address: String,
    pub horizon_url: String,
    pub payment_address: String,
    pub webhook_url: String,
    pub webhook_secret: String,
}

impl ServerConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = std::env::var("DATABASE_URL")?;
        let bind_address =
            std::env::var("BIND_ADDRESS").unwrap_or_else(|_| DEFAULT_BIND_ADDRESS.to_string());
        let horizon_url = std::env::var("STELLAR_HORIZON_URL")?;
        let payment_address = std::env::var("STELLAR_PAYMENT_ADDRESS")?;
        let webhook_url = std::env::var("WEBHOOK_URL")?;
        let webhook_secret = std::env::var("WEBHOOK_SECRET")?;

        return Ok(ServerConfig {
            database_url,
            bind_address,
            horizon_url,
            payment_address,
            webhook_url,
            webhook_secret,
        });
    }
}

// === Server bootstrap

// Does not auto-run migrations on boot, only `cli migrate` does, to avoid
// surprise schema changes on restart.
pub async fn run_server() -> anyhow::Result<()> {
    let config = ServerConfig::from_env()?;

    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    let payment_repository = Arc::new(PostgresPaymentRepository::new(pool.clone()));
    let webhook_event_repository: Arc<dyn stellar_pay_webhooks::WebhookEventRepository> =
        Arc::new(PostgresWebhookEventRepository::new(pool.clone()));
    let stellar_client = Arc::new(HorizonClient::new(config.horizon_url.clone()));
    let onchain_recorder = Arc::new(NullOnChainRecorder::new());

    let payment_service = Arc::new(PaymentService::new(
        payment_repository.clone(),
        config.payment_address.clone(),
    ));

    let webhook_service = Arc::new(WebhookService::new(
        webhook_event_repository.clone(),
        config.webhook_url.clone(),
        config.webhook_secret.clone(),
    ));

    let reconciliation_worker = Arc::new(ReconciliationWorker::new(
        payment_repository.clone(),
        stellar_client.clone(),
        webhook_service.clone(),
        onchain_recorder.clone(),
        RECONCILIATION_EXPIRY_WINDOW,
    ));

    {
        let worker = reconciliation_worker.clone();
        tokio::spawn(async move {
            worker.run_loop(RECONCILIATION_INTERVAL).await;
        });
    }

    {
        let service = webhook_service.clone();
        tokio::spawn(async move {
            run_delivery_loop(service, WEBHOOK_DELIVERY_INTERVAL).await;
        });
    }

    let state = AppState {
        payment_service,
        webhook_event_repository,
    };
    let router = build_router(state);
    let listener = TcpListener::bind(&config.bind_address).await?;

    tracing::info!(address = %config.bind_address, "stellar-pay-api listening");

    axum::serve(listener, router).await?;

    return Ok(());
}
