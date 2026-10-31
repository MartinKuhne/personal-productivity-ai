//! Tests for [`AgentContextBuilder`] construction and defaults (`agent::context`).

use super::*;

use std::path::Path;

#[test]
fn test_agent_context_creation() {
    let agent_config = AgentConfig::default();

    let ctx = AgentContextBuilder::new(agent_config.clone(), Uuid::new_v4(), "hello".to_string())
        .with_file_observer(std::sync::Arc::new(
            crate::tools::observer::DefaultFileObserver,
        ))
        .with_observer(Arc::new(crate::events::RecordingObserver::new()))
        .with_active_paths(Some(PathBuf::from("test.md")), None)
        .with_system_prompts(Vec::new())
        .build();
    assert_eq!(ctx.agent_config.models(), agent_config.models());
    assert!(ctx.active_file.as_deref() == Some(Path::new("test.md")));
    assert_eq!(ctx.prompt, "hello");
}
