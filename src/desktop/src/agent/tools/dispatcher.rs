//! Tool dispatch surface — the trait the tool executor and
//! delegate-style tools use to invoke another tool by name.
//!
//! The dispatcher exists to break the cycle where a `ToolContext`
//! carried an `Arc<ArcSwap<ToolRegistry>>` so `execute_tool` could
//! find the tool. With the dispatcher, only the executor and any
//! tool that genuinely needs to dispatch sub-calls (the MCP adapter
//! in practice) hold a dispatcher handle; the rest of the tools
//! receive a [`ToolExecContext`](super::context::ToolContext) with
//! no registry back-reference.

use crate::agent::tools::Safety;
use crate::config::AppConfig;
use std::sync::Arc;

/// Service bundle handed to tools alongside their arguments.
/// Today this is exactly the read-only view of the live
/// `AppConfig`; future additions (e.g. a typed credential store)
/// go here.
#[derive(Clone, Debug)]
pub struct ToolServices {
    /// Live configuration. Cheap to clone (single `Arc`).
    pub config: Arc<AppConfig>,
}

impl ToolServices {
    /// Build a `ToolServices` from the given config.
    pub fn new(config: Arc<AppConfig>) -> Self {
        Self { config }
    }
}

/// Result of a tool dispatch. Success carries the LLM-facing JSON
/// payload; failure carries an error message safe to render to the
/// LLM.
#[derive(Clone, Debug)]
pub enum ToolOutcome {
    /// Tool returned successfully. The contained JSON is what the
    /// LLM sees.
    Ok(serde_json::Value),
    /// Tool returned an error. The message is the LLM-facing
    /// string; the source is for tracing only.
    Err(ToolError),
}

impl ToolOutcome {
    /// Construct an `Ok` outcome.
    pub fn ok(value: serde_json::Value) -> Self {
        Self::Ok(value)
    }

    /// Construct an `Err` outcome from a message.
    pub fn err(message: impl Into<String>) -> Self {
        Self::Err(ToolError {
            message: message.into(),
        })
    }

    /// Return the inner JSON value, mapping `Err` to the JSON form
    /// `{"error": "<message>"}`. Useful when the caller wants a
    /// `serde_json::Value` regardless of outcome.
    pub fn into_json(self) -> serde_json::Value {
        match self {
            Self::Ok(v) => v,
            Self::Err(e) => serde_json::json!({"error": e.message}),
        }
    }

    /// Return `Ok(value)` for `Ok` outcomes and `Err(message)` for
    /// `Err` outcomes, preserving the error message. Useful when
    /// the caller wants to distinguish success from failure but
    /// not consume the structured `ToolError`.
    pub fn into_json_result(self) -> Result<serde_json::Value, String> {
        match self {
            Self::Ok(v) => Ok(v),
            Self::Err(e) => Err(e.message),
        }
    }
}

/// A tool-dispatch error. Currently a string; promoted to a struct
/// so future error-kind metadata (retryable, etc.) has somewhere
/// to live.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolError {
    /// Human-readable message safe to send back to the LLM.
    pub message: String,
}

impl ToolError {
    /// Build a new error with the given message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ToolError {}

/// Dispatch a tool by name. Implemented by the tool registry; held
/// by the executor and by tools that genuinely need to invoke
/// another tool (today: the MCP adapter).
pub trait ToolDispatcher: Send + Sync {
    /// Invoke `name` with the given JSON arguments and return the
    /// outcome. The dispatcher is responsible for finding the
    /// tool, calling `Tool::execute`, and translating the
    /// `Result<_, String>` return into a [`ToolOutcome`].
    fn dispatch(
        &self,
        name: &str,
        args: &str,
        ctx: &crate::agent::tools::context::ToolContext,
    ) -> ToolOutcome;

    /// Look up the [`Safety`] classification of `name`. Unknown
    /// names are conservatively classified as
    /// [`Safety::Mutating`].
    fn safety(&self, name: &str) -> Safety;
}
