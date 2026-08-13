//! Unit tests for [`super::tool_context`].

use super::tool_context::AgentToolContext;
use crate::agent::tools::registry::ToolRegistry;

#[test]
fn test_new_wraps_registry() {
    let registry = ToolRegistry::new();
    let bundle = AgentToolContext::new(registry);
    // The wrapped registry is the same as a fresh one — both
    // contain the default-registered tools.
    assert!(bundle.registry.descriptor("read_note").is_some());
}

#[test]
fn test_clone_is_cheap() {
    // `ToolRegistry` is `Clone` (the catalog is `BTreeMap<String,
    // RegisteredTool>` which clones via `Arc` bumps). The bundle
    // is therefore `Clone` and cheap.
    let bundle = AgentToolContext::new(ToolRegistry::new());
    let cloned = bundle.clone();
    assert!(cloned.registry.descriptor("read_note").is_some());
}
