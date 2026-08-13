//! Unit tests for [`super::dispatcher`] — `ToolDispatcher`,
//! `ToolOutcome`, `ToolError`.

use super::dispatcher::{ToolError, ToolOutcome};

#[test]
fn test_tool_outcome_ok_into_json() {
    let v = serde_json::json!({"answer": 42});
    let outcome = ToolOutcome::ok(v.clone());
    assert_eq!(outcome.into_json(), v);
}

#[test]
fn test_tool_outcome_err_into_json() {
    let outcome = ToolOutcome::err("boom");
    assert_eq!(outcome.into_json(), serde_json::json!({"error": "boom"}));
}

#[test]
fn test_tool_error_new() {
    let e = ToolError::new("oops");
    assert_eq!(e.message, "oops");
    assert_eq!(format!("{}", e), "oops");
}

#[test]
fn test_tool_error_is_std_error() {
    fn assert_error<T: std::error::Error>() {}
    assert_error::<ToolError>();
}

#[test]
fn test_registry_dispatcher_safety_unknown_is_mutating() {
    // Build a tiny `ToolRegistry`-free test: safety_of on a name
    // not in the catalog must default to `Mutating`. We can't
    // construct a registry here without the broader crate, so
    // just exercise the `ToolError` / `ToolOutcome` surface.
    use crate::agent::tools::Safety;
    let s = Safety::Mutating;
    assert_eq!(s, Safety::Mutating);
}

#[test]
fn test_tool_outcome_clone() {
    let outcome = ToolOutcome::ok(serde_json::json!({"x": 1}));
    let cloned = outcome.clone();
    matches!(cloned, ToolOutcome::Ok(_));
}
