use std::sync::Arc;

use stellar_pay_payments::PaymentService;
use stellar_pay_webhooks::WebhookEventRepository;

// === App state

// Handlers only ever touch the service or the webhook event repository, no
// SQL/Stellar calls in handlers.
#[derive(Clone)]
pub struct AppState {
    pub payment_service: Arc<PaymentService>,
    pub webhook_event_repository: Arc<dyn WebhookEventRepository>,
}
