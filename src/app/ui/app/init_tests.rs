#[test]
fn test_init_rs_does_not_recreate_tool_context() {
    let content = std::fs::read_to_string("src/app/ui/app/init.rs").unwrap();
    // Strip whitespace to avoid formatting issues
    let normalized = content
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    let count = normalized.matches("std::sync::Arc::new(arc_swap::ArcSwap::from_pointee(crate::agent::AgentToolContext::new").count();
    assert_eq!(
        count,
        2, // once in `new`, once in `empty_state_via_bus`
        "FastMdApp::new and empty_state_via_bus must use the shared tool_context, not recreate it inline"
    );
}
