use crate::ui::app::FastMdApp;

#[test]
fn test_empty_state_builds_with_default_config() {
    let config = crate::config::AppConfig::default();
    let app = FastMdApp::empty_state(config.clone());
    assert_eq!(
        app.orchestrator.config.content_libraries.len(),
        config.content_libraries.len()
    );
    assert_eq!(
        app.orchestrator.inline_editor_enabled,
        config.inline_editor_enabled
    );
    assert!(app.orchestrator.tabs.tabs.is_empty());
    assert!(app.cached_tree_rows.is_none());
}

#[test]
fn test_empty_state_preserves_content_libraries() {
    let mut config = crate::config::AppConfig::default();
    config.content_libraries = vec![crate::config::ContentLibrary {
        root_folder: "C:/tmp/lib1".to_string(),
        name: "Lib1".to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 0,
    }];
    config.inline_editor_enabled = true;
    let app = FastMdApp::empty_state(config.clone());
    assert_eq!(app.orchestrator.content_libraries.len(), 1);
    assert_eq!(
        app.orchestrator.content_libraries[0].root_folder,
        "C:/tmp/lib1"
    );
    assert!(app.orchestrator.inline_editor_enabled);
    assert_eq!(
        app.orchestrator.dialogs.batch_dialog_config.available_dirs,
        vec![std::path::PathBuf::from("C:/tmp/lib1")]
    );
}

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
