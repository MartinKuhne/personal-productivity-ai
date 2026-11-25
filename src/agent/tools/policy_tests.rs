//! Tests for the tool write-permission policy trait, its default allow-all impl and ext wrapper.

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
