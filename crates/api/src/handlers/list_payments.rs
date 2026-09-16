use axum::Json;
use axum::extract::{Query, State};

use crate::dto::payment::{ListPaymentsQuery, PaymentListResponse, PaymentResponse};
use crate::error::ApiError;
use crate::state::AppState;

const DEFAULT_PAGE: u32 = 1;
const DEFAULT_PER_PAGE: u32 = 20;
const MAX_PER_PAGE: u32 = 100;

pub async fn list_payments(
    State(state): State<AppState>,
    Query(query): Query<ListPaymentsQuery>,
) -> Result<Json<PaymentListResponse>, ApiError> {
    let page = query.page.unwrap_or(DEFAULT_PAGE).max(1);
    let per_page = query
        .per_page
        .unwrap_or(DEFAULT_PER_PAGE)
        .clamp(1, MAX_PER_PAGE);

    let (payments, total) = state
        .payment_service
        .list_payments(query.status, page, per_page)
        .await?;
    let data = payments.into_iter().map(PaymentResponse::from).collect();

    return Ok(Json(PaymentListResponse {
        data,
        page,
        per_page,
        total,
    }));
}
