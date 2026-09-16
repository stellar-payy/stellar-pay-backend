use axum::Json;
use axum::response::IntoResponse;
use serde_json::json;

pub async fn health() -> impl IntoResponse {
    return Json(json!({ "status": "ok" }));
}
