use axum::Json;
use axum::extract::{Path, State};
use uuid::Uuid;

use crate::dto::payment::PaymentResponse;
use crate::error::ApiError;
use crate::state::AppState;

pub async fn get_payment(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<PaymentResponse>, ApiError> {
    let payment = state.payment_service.get_payment(id).await?;
    return Ok(Json(PaymentResponse::from(payment)));
}
