//! Unit tests for centralized user-command execution.

use crate::config::{AppConfig, ContentLibrary};

#[test]
fn prompt_discovery_returns_tagged_prompt_candidates_from_content_libraries() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("summarize.md"),
        "---\ntitle: Summarize\ntags: [prompt]\n---\nSummarize the selected files.",
    )
    .unwrap();
    let config = AppConfig {
        content_libraries: vec![ContentLibrary {
            root_folder: temp.path().to_string_lossy().into_owned(),
            name: "Test Library".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        }],
        ..AppConfig::default()
    };

    let prompts = crate::agent::batch::prompts::discover_prompts(&config);

    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0].display_name, "Test Library / summarize.md");
    assert_eq!(prompts[0].content, "Summarize the selected files.");
}
