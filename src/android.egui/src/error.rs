//! Crate-wide error type.
//!
//! Used by every other module so callers only need to handle one error type.
//! All variants carry a `String` for context; we trade structured data for a
//! flat enum that works well with `?` and `Box<dyn Error>` consumers.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("HTTP error: {0}")]
    Http(String),

    #[error("JSON error: {0}")]
    Json(String),

    #[error("Microsoft Graph error: {0}")]
    Graph(String),

    #[error("OAuth / auth error: {0}")]
    Auth(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("I/O error: {0}")]
    Io(String),

    #[error("crypto error: {0}")]
    Crypto(String),

    #[error("invalid input: {0}")]
    Invalid(String),
}

impl From<ureq::Error> for AppError {
    fn from(e: ureq::Error) -> Self {
        match e {
            ureq::Error::Status(code, response) => {
                let body = response
                    .into_string()
                    .unwrap_or_else(|_| "<unreadable>".to_string());
                AppError::Http(format!("status {code}: {body}"))
            }
            ureq::Error::Transport(t) => AppError::Http(t.to_string()),
        }
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Json(e.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

pub type AppResult<T> = std::result::Result<T, AppError>;
