use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub error: ApiErrorDetail,
}

#[derive(Debug, Serialize)]
pub struct ApiErrorDetail {
    pub code: String,
    pub message: String,
}

impl ApiError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            error: ApiErrorDetail {
                code: code.into(),
                message: message.into(),
            },
        }
    }

    pub fn bad_request(message: impl Into<String>) -> (StatusCode, Json<Self>) {
        (StatusCode::BAD_REQUEST, Json(Self::new("BAD_REQUEST", message)))
    }

    pub fn unavailable(code: impl Into<String>, message: impl Into<String>) -> (StatusCode, Json<Self>) {
        (StatusCode::SERVICE_UNAVAILABLE, Json(Self::new(code, message)))
    }

    pub fn not_found(message: impl Into<String>) -> (StatusCode, Json<Self>) {
        (StatusCode::NOT_FOUND, Json(Self::new("NOT_FOUND", message)))
    }
}

pub type ApiResult<T> = Result<T, (StatusCode, Json<ApiError>)>;
