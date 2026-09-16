use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};

use crate::dto::payment::{CreatePaymentRequest, PaymentResponse};
use crate::error::ApiError;
use crate::state::AppState;

const IDEMPOTENCY_KEY_HEADER: &str = "Idempotency-Key";

pub async fn create_payment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreatePaymentRequest>,
) -> Result<(StatusCode, Json<PaymentResponse>), ApiError> {
    let idempotency_key = headers
        .get(IDEMPOTENCY_KEY_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());

    let payment = state
        .payment_service
        .create_payment(
            request.amount,
            request.reference,
            request.destination,
            request.memo,
            idempotency_key,
        )
        .await?;

    return Ok((StatusCode::CREATED, Json(PaymentResponse::from(payment))));
}
