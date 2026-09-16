use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use stellar_pay_core::{PaymentError, RepositoryError};

// === API error

pub struct ApiError {
    status: StatusCode,
    message: String,
}

#[derive(Serialize)]
struct ApiErrorBody {
    error: String,
}

impl ApiError {
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        return ApiError {
            status,
            message: message.into(),
        };
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ApiErrorBody {
            error: self.message,
        };
        return (self.status, Json(body)).into_response();
    }
}

impl From<PaymentError> for ApiError {
    fn from(err: PaymentError) -> Self {
        let status = match &err {
            PaymentError::InvalidAmount(_) | PaymentError::UnsupportedAsset => {
                StatusCode::BAD_REQUEST
            }
            PaymentError::NotFound(_) => StatusCode::NOT_FOUND,
            PaymentError::DuplicateReference(_) => StatusCode::CONFLICT,
            PaymentError::InvalidTransition { .. } => StatusCode::CONFLICT,
            PaymentError::Repository(repo_err) => match repo_err {
                RepositoryError::NotFound => StatusCode::NOT_FOUND,
                RepositoryError::Conflict(_) => StatusCode::CONFLICT,
                RepositoryError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            },
        };

        return ApiError::new(status, err.to_string());
    }
}

impl From<RepositoryError> for ApiError {
    fn from(err: RepositoryError) -> Self {
        return ApiError::from(PaymentError::from(err));
    }
}
