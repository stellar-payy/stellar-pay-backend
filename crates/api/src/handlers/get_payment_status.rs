use axum::Json;
use axum::extract::{Path, State};
use uuid::Uuid;

use crate::dto::payment::PaymentStatusResponse;
use crate::error::ApiError;
use crate::state::AppState;

pub async fn get_payment_status(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<PaymentStatusResponse>, ApiError> {
    let payment = state.payment_service.get_payment(id).await?;
    return Ok(Json(PaymentStatusResponse {
        id: payment.id,
        status: payment.status,
    }));
}
