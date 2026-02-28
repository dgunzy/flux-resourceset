use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

use crate::db::DataStoreError;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("resource not found")]
    NotFound,
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("internal server error: {0}")]
    Internal(String),
    #[error("data store error: {0}")]
    Store(#[from] DataStoreError),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            AppError::NotFound => (StatusCode::NOT_FOUND, "not_found", self.to_string()),
            AppError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized", self.to_string()),
            AppError::Forbidden => (StatusCode::FORBIDDEN, "forbidden", self.to_string()),
            AppError::Validation(msg) => (StatusCode::BAD_REQUEST, "validation_error", msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, "conflict", msg.clone()),
            AppError::Internal(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "internal server error".to_string(),
            ),
            AppError::Store(err) => match err {
                DataStoreError::NotFound(_) => {
                    (StatusCode::NOT_FOUND, "not_found", err.to_string())
                }
                DataStoreError::Conflict(_) => (StatusCode::CONFLICT, "conflict", err.to_string()),
                DataStoreError::Io(_) | DataStoreError::Json(_) | DataStoreError::Sqlx(_) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "store_error",
                    "data store error".to_string(),
                ),
            },
        };

        let body = axum::Json(serde_json::json!({
            "error": code,
            "message": message,
        }));

        (status, body).into_response()
    }
}
