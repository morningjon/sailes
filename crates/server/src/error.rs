use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    BadRequest(String),
    #[error("sign in to continue")]
    Unauthorized,
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    pub fn bad(msg: impl Into<String>) -> Self {
        AppError::BadRequest(msg.into())
    }
}

impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match &err {
            sqlx::Error::RowNotFound => AppError::NotFound,
            sqlx::Error::Database(db) => match db.code().as_deref() {
                Some("23505") => AppError::Conflict(unique_message(db.constraint())),
                Some("23503") => AppError::BadRequest("A linked record doesn't exist.".into()),
                Some("23514") | Some("22P02") | Some("22007") | Some("22008") => {
                    AppError::BadRequest(format!("Invalid value: {}", db.message()))
                }
                _ => AppError::Internal(err.into()),
            },
            _ => AppError::Internal(err.into()),
        }
    }
}

fn unique_message(constraint: Option<&str>) -> String {
    match constraint {
        Some("contacts_email_uq") => "A contact with this email already exists.".into(),
        Some("companies_domain_uq") => "A company with this domain already exists.".into(),
        Some("users_email_key") => "A user with this email already exists.".into(),
        _ => "This record already exists.".into(),
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::Internal(e) => {
                tracing::error!(error = ?e, "internal error");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };
        let message = match &self {
            AppError::Internal(_) => {
                "Something went wrong on the server. Check the server logs.".to_string()
            }
            other => other.to_string(),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}
