use axum::Json;
use axum::extract::{Path, State};
use uuid::Uuid;

use crate::dto::payment::{WebhookEventListResponse, WebhookEventResponse};
use crate::error::ApiError;
use crate::state::AppState;

pub async fn list_payment_webhook_events(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<WebhookEventListResponse>, ApiError> {
    // ensure the payment exists before returning its delivery history
    state.payment_service.get_payment(id).await?;

    let events = state.webhook_event_repository.list_for_payment(id).await?;
    let data = events.into_iter().map(WebhookEventResponse::from).collect();

    return Ok(Json(WebhookEventListResponse { data }));
}
