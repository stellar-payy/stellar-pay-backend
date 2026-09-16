use axum::Router;
use axum::routing::{get, post};

use crate::handlers::create_payment::create_payment;
use crate::handlers::get_payment::get_payment;
use crate::handlers::get_payment_status::get_payment_status;
use crate::handlers::health::health;
use crate::handlers::list_payment_webhook_events::list_payment_webhook_events;
use crate::handlers::list_payments::list_payments;
use crate::state::AppState;

// axum 0.8 path syntax uses `{id}`, not `:id`.
pub fn build_router(state: AppState) -> Router {
    return Router::new()
        .route("/health", get(health))
        .route("/v1/payments", post(create_payment).get(list_payments))
        .route("/v1/payments/{id}", get(get_payment))
        .route("/v1/payments/{id}/status", get(get_payment_status))
        .route(
            "/v1/payments/{id}/webhook-events",
            get(list_payment_webhook_events),
        )
        .with_state(state);
}
