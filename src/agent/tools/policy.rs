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
mod tests {
    use super::*;

    struct RejectingPolicy;

    impl ToolCallPolicy for RejectingPolicy {
        fn check_write_allowed(&self, _path: &Path) -> Result<(), String> {
            Err("blocked by test policy".to_string())
        }
    }

    #[test]
    fn default_policy_allows_all_paths() {
        let policy = DefaultToolCallPolicy;
        assert!(
            policy
                .check_write_allowed(Path::new("/any/path.md"))
                .is_ok()
        );
    }

    #[test]
    fn default_policy_accepts_dotfiles_and_relative_paths() {
        let policy = DefaultToolCallPolicy;
        assert!(policy.check_write_allowed(Path::new("notes.md")).is_ok());
        assert!(policy.check_write_allowed(Path::new(".hidden")).is_ok());
    }

    #[test]
    fn rejecting_policy_propagates_error_through_ext() {
        let ext = ToolCallPolicyExt(std::sync::Arc::new(RejectingPolicy));
        let err = ext
            .0
            .check_write_allowed(Path::new("/x.md"))
            .expect_err("rejecting policy must block");
        assert_eq!(err, "blocked by test policy");
    }
}
