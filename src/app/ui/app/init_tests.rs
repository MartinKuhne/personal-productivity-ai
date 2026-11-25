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
fn test_empty_state_labels_batch_dirs_with_library_names_in_order() {
    let config = crate::config::AppConfig {
        content_libraries: vec![
            crate::config::ContentLibrary {
                root_folder: r"C:\notes\alpha".to_string(),
                name: "Alpha".to_string(),
                kind: "text".to_string(),
                readonly: false,
                priority: 0,
            },
            crate::config::ContentLibrary {
                root_folder: r"D:\vault\beta".to_string(),
                name: "Beta".to_string(),
                kind: "text".to_string(),
                readonly: true,
                priority: 1,
            },
        ],
        ..Default::default()
    };
    let app = FastMdApp::empty_state(config.clone());
    let dialog = &app.orchestrator.dialogs.batch_dialog_config;

    assert_eq!(
        dialog.available_dirs,
        vec![
            std::path::PathBuf::from(r"C:\notes\alpha"),
            std::path::PathBuf::from(r"D:\vault\beta"),
        ]
    );
    assert_eq!(
        dialog.available_dir_labels.len(),
        dialog.available_dirs.len(),
        "labels must stay index-aligned with the directories"
    );
    for (index, library) in config.content_libraries.iter().enumerate() {
        assert_eq!(
            dialog.available_dir_labels[index], library.name,
            "label {index} must be the name of content library {index}"
        );
    }
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
        1, // once in `empty_state_via_bus`
        "empty_state_via_bus must use the shared tool_context, not recreate it inline"
    );
}
