//! Error types for the agent subsystem — covers network, HTTP, JSON, IO, tool, and config error variants.

use std::fmt;

#[derive(Debug)]
pub enum AgentError {
    NetworkError(String),
    HttpError { status: u16, body: String },
    JsonParseError(String),
    InvalidResponseSchema(String),
    MissingApiKey,
    IoError(std::io::Error),
    ToolError(String),
    Timeout,
    SerializationError(String),
    ConfigError(String),
    RuntimeError(String),
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentError::NetworkError(msg) => write!(
                f,
                "Network error: {}. Please check your internet connection.",
                msg
            ),
            AgentError::HttpError { status, body } => write!(f, "HTTP {} error: {}", status, body),
            AgentError::JsonParseError(msg) => write!(
                f,
                "Failed to parse response: {}. The API may have returned an unexpected format.",
                msg
            ),
            AgentError::InvalidResponseSchema(msg) => write!(
                f,
                "Invalid response from API: {}. The API format may be incompatible.",
                msg
            ),
            AgentError::MissingApiKey => write!(
                f,
                "API key is not configured. Please set your API key in the settings."
            ),
            AgentError::IoError(err) => write!(f, "File system error: {}", err),
            AgentError::ToolError(msg) => write!(f, "Tool execution failed: {}", msg),
            AgentError::Timeout => write!(
                f,
                "Request timed out. The server may be overloaded or unreachable."
            ),
            AgentError::SerializationError(msg) => write!(f, "Failed to serialize data: {}", msg),
            AgentError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            AgentError::RuntimeError(msg) => write!(f, "Runtime error: {}", msg),
        }
    }
}

impl std::error::Error for AgentError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AgentError::IoError(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for AgentError {
    fn from(err: std::io::Error) -> Self {
        AgentError::IoError(err)
    }
}

impl AgentError {
    pub fn is_retryable(&self) -> bool {
        match self {
            AgentError::NetworkError(_) => true,
            AgentError::HttpError { status, .. } => *status >= 500 || *status == 429,
            AgentError::Timeout => true,
            AgentError::IoError(_) => false,
            AgentError::JsonParseError(_) => false,
            AgentError::InvalidResponseSchema(_) => false,
            AgentError::MissingApiKey => false,
            AgentError::ToolError(_) => false,
            AgentError::SerializationError(_) => false,
            AgentError::ConfigError(_) => false,
            AgentError::RuntimeError(_) => false,
        }
    }

    pub fn user_message(&self) -> String {
        format!("{}", self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_error_is_retryable() {
        let err = AgentError::NetworkError("connection refused".to_string());
        assert!(err.is_retryable());
    }

    #[test]
    fn test_http_500_is_retryable() {
        let err = AgentError::HttpError {
            status: 500,
            body: "Internal Server Error".to_string(),
        };
        assert!(err.is_retryable());
    }

    #[test]
    fn test_http_502_is_retryable() {
        let err = AgentError::HttpError {
            status: 502,
            body: "Bad Gateway".to_string(),
        };
        assert!(err.is_retryable());
    }

    #[test]
    fn test_http_503_is_retryable() {
        let err = AgentError::HttpError {
            status: 503,
            body: "Service Unavailable".to_string(),
        };
        assert!(err.is_retryable());
    }

    #[test]
    fn test_http_429_is_retryable() {
        let err = AgentError::HttpError {
            status: 429,
            body: "Too Many Requests".to_string(),
        };
        assert!(err.is_retryable());
    }

    #[test]
    fn test_http_400_not_retryable() {
        let err = AgentError::HttpError {
            status: 400,
            body: "Bad Request".to_string(),
        };
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_http_401_not_retryable() {
        let err = AgentError::HttpError {
            status: 401,
            body: "Unauthorized".to_string(),
        };
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_http_404_not_retryable() {
        let err = AgentError::HttpError {
            status: 404,
            body: "Not Found".to_string(),
        };
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_timeout_is_retryable() {
        let err = AgentError::Timeout;
        assert!(err.is_retryable());
    }

    #[test]
    fn test_io_error_not_retryable() {
        let err = AgentError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_json_parse_error_not_retryable() {
        let err = AgentError::JsonParseError("unexpected token".to_string());
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_missing_api_key_not_retryable() {
        let err = AgentError::MissingApiKey;
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_tool_error_not_retryable() {
        let err = AgentError::ToolError("invalid path".to_string());
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let agent_err: AgentError = io_err.into();
        match agent_err {
            AgentError::IoError(_) => {}
            _ => panic!("Expected IoError variant"),
        }
    }

    #[test]
    fn test_display_network_error() {
        let err = AgentError::NetworkError("DNS lookup failed".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("Network error"));
        assert!(msg.contains("DNS lookup failed"));
        assert!(msg.contains("internet connection"));
    }

    #[test]
    fn test_display_http_error() {
        let err = AgentError::HttpError {
            status: 500,
            body: "server error".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("500"));
        assert!(msg.contains("server error"));
    }

    #[test]
    fn test_display_missing_api_key() {
        let err = AgentError::MissingApiKey;
        let msg = format!("{}", err);
        assert!(msg.contains("API key"));
        assert!(msg.contains("not configured"));
    }

    #[test]
    fn test_display_io_error() {
        let err = AgentError::IoError(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        let msg = format!("{}", err);
        assert!(msg.contains("File system error"));
        assert!(msg.contains("file not found"));
    }

    #[test]
    fn test_display_timeout() {
        let err = AgentError::Timeout;
        let msg = format!("{}", err);
        assert!(msg.contains("timed out"));
    }

    #[test]
    fn test_user_message_contains_actionable_info() {
        let err = AgentError::MissingApiKey;
        let msg = err.user_message();
        assert!(msg.contains("API key"));
        assert!(msg.contains("settings"));
    }

    #[test]
    fn test_debug_impl() {
        let err = AgentError::NetworkError("test".to_string());
        let debug = format!("{:?}", err);
        assert!(debug.contains("NetworkError"));
    }

    #[test]
    fn test_error_trait_source() {
        let io_err = std::io::Error::new(std::io::ErrorKind::Other, "test");
        let err = AgentError::IoError(io_err);
        assert!(std::error::Error::source(&err).is_some());

        let err2 = AgentError::NetworkError("test".to_string());
        assert!(std::error::Error::source(&err2).is_none());
    }

    #[test]
    fn test_all_variants_display() {
        let variants: Vec<AgentError> = vec![
            AgentError::NetworkError("test".into()),
            AgentError::HttpError {
                status: 500,
                body: "test".into(),
            },
            AgentError::JsonParseError("test".into()),
            AgentError::InvalidResponseSchema("test".into()),
            AgentError::MissingApiKey,
            AgentError::IoError(std::io::Error::new(std::io::ErrorKind::Other, "test")),
            AgentError::ToolError("test".into()),
            AgentError::Timeout,
            AgentError::SerializationError("test".into()),
            AgentError::ConfigError("test".into()),
            AgentError::RuntimeError("test".into()),
        ];
        for v in &variants {
            let msg = format!("{}", v);
            assert!(!msg.is_empty(), "Display for {:?} should not be empty", v);
        }
    }
}
