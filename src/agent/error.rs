//! Error types for the agent subsystem — covers network, HTTP, JSON, IO, tool, and config error variants.
//! Unit tests live in the sibling `error_tests.rs` sidecar.

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("Network error: {0}. Please check your internet connection.")]
    NetworkError(String),

    #[error("HTTP {status} error: {body}")]
    HttpError { status: u16, body: String },

    #[error("Failed to parse response: {0}. The API may have returned an unexpected format.")]
    JsonParseError(String),

    #[error("Invalid response from API: {0}. The API format may be incompatible.")]
    InvalidResponseSchema(String),

    #[error("API key is not configured. Please set your API key in the settings.")]
    MissingApiKey,

    #[error("File system error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Tool execution failed: {0}")]
    ToolError(String),

    #[error("Request timed out. The server may be overloaded or unreachable.")]
    Timeout,

    #[error("Failed to serialize data: {0}")]
    SerializationError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Runtime error: {0}")]
    RuntimeError(String),
}

impl AgentError {
    pub fn is_retryable(&self) -> bool {
        match self {
            AgentError::NetworkError(_) => true,
            AgentError::HttpError { status, .. } => *status >= 500 || *status == 429,
            AgentError::Timeout => true,
            AgentError::IoError(_)
            | AgentError::JsonParseError(_)
            | AgentError::InvalidResponseSchema(_)
            | AgentError::MissingApiKey
            | AgentError::ToolError(_)
            | AgentError::SerializationError(_)
            | AgentError::ConfigError(_)
            | AgentError::RuntimeError(_) => false,
        }
    }

    pub fn user_message(&self) -> String {
        format!("{}", self)
    }
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
