use std::sync::Arc;
use std::time::Duration;

use crate::service::WebhookService;

// === Delivery loop

pub async fn run_delivery_loop(service: Arc<WebhookService>, interval: Duration) {
    let mut ticker = tokio::time::interval(interval);

    loop {
        ticker.tick().await;

        if let Err(err) = service.deliver_due().await {
            tracing::error!(?err, "webhook delivery pass failed");
        }
    }
}
