use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug)]
pub enum AppError {
    MissingCredentials,
    GitHubApiError(String),
    NimApiError(String),
    /// The requested NIM model is not served to this account (404/410).
    /// Separate from NimApiError so `chat` knows a different model may work,
    /// while a bad key or rate limit stops the walk immediately.
    NimModelUnavailable(String),
    DatabaseError(String),
    NotFound(String),
    BadRequest(String),
    Cancelled,
    Internal(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let msg = match self {
            AppError::MissingCredentials => "Missing GitHub token or NIM API key".to_string(),
            AppError::GitHubApiError(m) | AppError::NimApiError(m)
            | AppError::NimModelUnavailable(m) | AppError::DatabaseError(m) => m.clone(),
            AppError::NotFound(m) | AppError::BadRequest(m) | AppError::Internal(m) => m.clone(),
            AppError::Cancelled => "Analysis terminated by user".to_string(),
        };
        f.write_str(&msg)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::MissingCredentials => {
                (StatusCode::UNAUTHORIZED, "Missing GitHub token or NIM API key".into())
            }
            AppError::GitHubApiError(msg) => (StatusCode::BAD_GATEWAY, msg),
            AppError::NimApiError(msg) => (StatusCode::BAD_GATEWAY, msg),
            AppError::NimModelUnavailable(msg) => (StatusCode::BAD_GATEWAY, msg),
            AppError::DatabaseError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::Cancelled => (StatusCode::ACCEPTED, "Analysis terminated".into()),
            AppError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
        };

        (status, Json(json!({ "error": message }))).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        AppError::DatabaseError(e.to_string())
    }
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Internal(e.to_string())
    }
}
