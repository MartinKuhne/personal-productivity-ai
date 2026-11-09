//! Write-permission policy callbacks injected into agent tools from the application context.
//!
//! Unit tests live in the sibling `policy_tests.rs` sidecar.

use std::path::Path;

/// Callbacks injected into tools from the application context.
pub trait ToolCallPolicy: Send + Sync + 'static {
    /// Check whether a given path is allowed to be written to. Returns an error string if blocked.
    fn check_write_allowed(&self, path: &Path) -> Result<(), String>;
}

#[derive(Debug)]
pub struct DefaultToolCallPolicy;

impl ToolCallPolicy for DefaultToolCallPolicy {
    fn check_write_allowed(&self, _path: &Path) -> Result<(), String> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct ToolCallPolicyExt(pub std::sync::Arc<dyn ToolCallPolicy>);

impl std::fmt::Debug for ToolCallPolicyExt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ToolCallPolicyExt")
            .field(&"<dyn ToolCallPolicy>")
            .finish()
    }
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
