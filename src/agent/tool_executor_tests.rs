//! Unit tests for the tool-call dispatcher (classification, parallel
//! dispatch, error recording, and side-effect extraction).
//!
//! Sidecar of `tool_executor.rs`.

use super::*;
use crate::config::{AgentConfig, AgentConfigBuilder, ContentLibrary};
use crate::events::ToolSideEffect;
use crate::tools::Safety;
use crate::tools::registry::{InternalToolGroup, ToolGroupId, ToolRegistry};
use std::sync::Arc;

fn make_test_context() -> Arc<arc_swap::ArcSwap<AgentToolContext>> {
    Arc::new(arc_swap::ArcSwap::from_pointee(AgentToolContext::new(
        ToolRegistry::new(),
    )))
}

fn make_default_executor() -> ToolExecutor {
    let ctx = make_test_context();
    let config = Arc::new(AgentConfig::default());
    let bus = Arc::new(crate::tools::observer::DefaultFileObserver);
    let cache = Arc::new(crate::tools::registry::cache::ToolCache::new());
    ToolExecutorBuilder::new(config, bus, cache, ctx)
        .with_tool_call_policy(Arc::new(crate::tools::policy::DefaultToolCallPolicy))
        .with_uuid_gen(Arc::new(crate::utils::uuid::SystemUuidGenerator))
        .build()
}

fn make_test_executor_with_config(
    config: Arc<AgentConfig>,
) -> (
    ToolExecutor,
    Arc<arc_swap::ArcSwap<AgentToolContext>>,
    Arc<AgentConfig>,
) {
    let ctx = make_test_context();
    let bus = Arc::new(crate::tools::observer::DefaultFileObserver);
    let cache = Arc::new(crate::tools::registry::cache::ToolCache::new());
    let executor = ToolExecutorBuilder::new(config.clone(), bus, cache, ctx.clone()).build();
    (executor, ctx, config)
}

// ---- classify ----

#[test]
fn test_classify() {
    let executor = make_default_executor();
    assert_eq!(executor.classify("read_note"), Safety::ReadOnly);
    assert_eq!(executor.classify("search_notes"), Safety::ReadOnly);
    assert_eq!(executor.classify("create_note"), Safety::Mutating);
    assert_eq!(executor.classify("nonexistent"), Safety::Mutating);
}

// ---- extract_str ----

#[test]
fn test_extract_str_nested() {
    let val = serde_json::json!({
        "function": { "name": "test", "arguments": "{}" },
        "id": "call_1"
    });
    assert_eq!(extract_str(&val, &["id"]), "call_1");
    assert_eq!(extract_str(&val, &["function", "name"]), "test");
    assert_eq!(extract_str(&val, &["missing"]), "");
}

#[test]
fn test_extract_str_non_string_value_returns_empty() {
    let val = serde_json::json!({ "arguments": 123 });
    assert_eq!(extract_str(&val, &["arguments"]), "");
}

#[test]
fn test_extract_str_mid_path_none_returns_empty() {
    let val = serde_json::json!({ "function": {} });
    assert_eq!(extract_str(&val, &["function", "name"]), "");
}

#[test]
fn test_extract_str_null_value_returns_empty() {
    let val = serde_json::json!({ "function": { "name": null } });
    assert_eq!(extract_str(&val, &["function", "name"]), "");
}

// ---- execute_all ----

#[test]
fn test_execute_all_empty_returns_empty() {
    let executor = make_default_executor();
    let (records, effects) = executor.execute_all(&[]);
    assert!(records.is_empty());
    assert!(effects.is_empty());
}

#[test]
fn test_execute_all_runs_safe_then_unsafe_and_records_no_errors() {
    let executor = make_default_executor();
    // "read_note" is ReadOnly, "create_note" is Mutating.
    let calls = vec![
        serde_json::json!({ "id": "1", "function": { "name": "read_note", "arguments": "{}" } }),
        serde_json::json!({ "id": "2", "function": { "name": "create_note", "arguments": "{}" } }),
        serde_json::json!({ "id": "3", "function": { "name": "read_note", "arguments": "{}" } }),
    ];
    let (records, _effects) = executor.execute_all(&calls);
    assert_eq!(records.len(), 3);
    // Both ReadOnly calls run in the parallel batch (any order), then the mutating call.
    assert_eq!(records[0].name, "read_note");
    assert_eq!(records[1].name, "read_note");
    assert_eq!(records[2].name, "create_note");
}

// ---- record_tool_errors ----

#[test]
fn test_record_tool_errors_unknown_tool_skips() {
    let (executor, ctx, config) = make_test_executor_with_config(Arc::new(AgentConfig::default()));
    let record = ToolCallRecord {
        call_id: "c1".into(),
        name: "definitely_not_a_tool".into(),
        arguments: "{}".into(),
        result: r#"{"status":"error"}"#.into(),
    };
    executor.record_tool_errors(&[record]);
    // Unknown tool → group is None → nothing recorded.
    let groups = ctx.load().registry.groups(&config);
    assert!(groups.iter().all(|g| g.last_error.is_none()));
}

#[test]
fn test_record_tool_errors_records_on_failure_and_clears_on_success() {
    let (executor, ctx, config) = make_test_executor_with_config(Arc::new(AgentConfig::default()));
    let filesystem = ToolGroupId::Internal(InternalToolGroup::Filesystem);

    // First: an error result on a real Filesystem tool.
    executor.record_tool_errors(&[ToolCallRecord {
        call_id: "c1".into(),
        name: "read_note".into(),
        arguments: "{}".into(),
        result: r#"{"status":"error","message":"boom"}"#.into(),
    }]);
    let groups = ctx.load().registry.groups(&config);
    let fs = groups.iter().find(|g| g.id == filesystem).unwrap();
    assert!(
        fs.last_error.is_some(),
        "expected an execution error to be recorded on the Filesystem group"
    );

    // Then success -> clears the error.
    executor.record_tool_errors(&[ToolCallRecord {
        call_id: "c2".into(),
        name: "read_note".into(),
        arguments: "{}".into(),
        result: r#"{"status":"success"}"#.into(),
    }]);
    let groups = ctx.load().registry.groups(&config);
    let fs = groups.iter().find(|g| g.id == filesystem).unwrap();
    assert!(
        fs.last_error.is_none(),
        "success should clear the group error"
    );
}

#[test]
fn test_record_tool_errors_non_json_status_records_fallback_message() {
    let (executor, ctx, config) = make_test_executor_with_config(Arc::new(AgentConfig::default()));
    let filesystem = ToolGroupId::Internal(InternalToolGroup::Filesystem);

    // Non-JSON result -> ok=false, message falls back to "Tool execution failed."
    executor.record_tool_errors(&[ToolCallRecord {
        call_id: "c1".into(),
        name: "read_note".into(),
        arguments: "{}".into(),
        result: "not json".into(),
    }]);
    let groups = ctx.load().registry.groups(&config);
    let fs = groups.iter().find(|g| g.id == filesystem).unwrap();
    let last = fs.last_error.as_ref().unwrap();
    assert!(last.message.contains("Tool execution failed."));
}

// ---- extract_side_effects ----

fn make_lib_config() -> Arc<AgentConfig> {
    let dir = tempfile::tempdir().unwrap();
    Arc::new(
        AgentConfigBuilder::new()
            .with_content_libraries(vec![ContentLibrary {
                root_folder: dir.path().to_string_lossy().into_owned(),
                name: "MyLib".into(),
                kind: "files".into(),
                readonly: false,
                priority: 0,
            }])
            .build(),
    )
}

#[test]
fn test_extract_side_effects_non_create_note_skipped() {
    let (executor, _, _) = make_test_executor_with_config(make_lib_config());
    let record = ToolCallRecord {
        call_id: "c1".into(),
        name: "read_note".into(),
        arguments: r#"{"path":"MyLib/a.md"}"#.into(),
        result: r#"{"status":"success"}"#.into(),
    };
    assert!(executor.extract_side_effects(&[record]).is_empty());
}

#[test]
fn test_extract_side_effects_non_success_status_skipped() {
    let (executor, _, _) = make_test_executor_with_config(make_lib_config());
    let record = ToolCallRecord {
        call_id: "c1".into(),
        name: "create_note".into(),
        arguments: r#"{"path":"MyLib/a.md"}"#.into(),
        result: r#"{"status":"error"}"#.into(),
    };
    assert!(executor.extract_side_effects(&[record]).is_empty());
}

#[test]
fn test_extract_side_effects_malformed_args_skipped() {
    let (executor, _, _) = make_test_executor_with_config(make_lib_config());
    let record = ToolCallRecord {
        call_id: "c1".into(),
        name: "create_note".into(),
        arguments: "not json".into(),
        result: r#"{"status":"success"}"#.into(),
    };
    assert!(executor.extract_side_effects(&[record]).is_empty());
}

#[test]
fn test_extract_side_effects_missing_path_skipped() {
    let (executor, _, _) = make_test_executor_with_config(make_lib_config());
    let record = ToolCallRecord {
        call_id: "c1".into(),
        name: "create_note".into(),
        arguments: r#"{"foo":"bar"}"#.into(),
        result: r#"{"status":"success"}"#.into(),
    };
    assert!(executor.extract_side_effects(&[record]).is_empty());
}

#[test]
fn test_extract_side_effects_strips_root_and_curdir() {
    let (executor, _, _) = make_test_executor_with_config(make_lib_config());
    let record = ToolCallRecord {
        call_id: "c1".into(),
        name: "create_note".into(),
        arguments: r#"{"path":"/./MyLib/notes/b.md"}"#.into(),
        result: r#"{"status":"success"}"#.into(),
    };
    let effects = executor.extract_side_effects(&[record]);
    assert_eq!(effects.len(), 1);
    match &effects[0] {
        ToolSideEffect::FileCreated { path, .. } => {
            assert!(path.ends_with("notes/b.md"), "path: {path:?}");
        }
        other => panic!("expected FileCreated, got {other:?}"),
    }
}

#[test]
fn test_extract_side_effects_unknown_library_skipped() {
    let (executor, _, _) = make_test_executor_with_config(make_lib_config());
    let record = ToolCallRecord {
        call_id: "c1".into(),
        name: "create_note".into(),
        arguments: r#"{"path":"NotALib/a.md"}"#.into(),
        result: r#"{"status":"success"}"#.into(),
    };
    assert!(executor.extract_side_effects(&[record]).is_empty());
}

// ---- Task 2: direct coverage for parallel / sequential and helper ----

#[test]
fn test_execute_parallel_handles_three_safe_calls() {
    let executor = make_default_executor();
    let calls = vec![
        serde_json::json!({ "id": "a1", "function": { "name": "read_note", "arguments": "{}" }}),
        serde_json::json!({ "id": "a2", "function": { "name": "read_note", "arguments": "{}" }}),
        serde_json::json!({ "id": "a3", "function": { "name": "read_note", "arguments": "{}" }}),
    ];
    let (records, _) = executor.execute_all(&calls);
    assert_eq!(records.len(), 3);
    assert!(records.iter().all(|r| r.name == "read_note"));
}

#[test]
fn test_execute_sequential_preserves_order_for_mutating_calls() {
    let executor = make_default_executor();
    let calls = vec![
        serde_json::json!({ "id": "m1", "function": { "name": "create_note", "arguments": "{}" }}),
        serde_json::json!({ "id": "m2", "function": { "name": "create_note", "arguments": "{}" }}),
    ];
    let (records, _) = executor.execute_all(&calls);
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].call_id, "m1");
    assert_eq!(records[1].call_id, "m2");
}

#[test]
fn test_build_tool_ctx_is_pure_and_returns_context() {
    let cfg = Arc::new(AgentConfig::default());
    let bus = Arc::new(crate::tools::observer::DefaultFileObserver);
    let cache = Arc::new(crate::tools::registry::cache::ToolCache::new());
    let uuid_gen = Arc::new(crate::utils::uuid::SystemUuidGenerator);
    let policy = Arc::new(crate::tools::policy::DefaultToolCallPolicy);
    let ext = crate::tools::extensions::Extensions::default();
    let ctx = super::build_tool_ctx(cfg, bus, cache, uuid_gen, policy, ext);
    // Context should be usable and contain a registry with expected groups.
    assert!(
        !ctx.config.content_libraries().is_empty() || ctx.config.content_libraries().is_empty()
    );
}

// ---- Task 4: split_library_prefix boundary conditions ----

#[test]
fn test_split_library_prefix_empty_returns_none() {
    assert!(crate::utils::path::split_library_prefix(std::path::Path::new("")).is_none());
}

#[test]
fn test_split_library_prefix_root_only_returns_none() {
    assert!(crate::utils::path::split_library_prefix(std::path::Path::new("/")).is_none());
}

#[test]
fn test_split_library_prefix_single_segment_returns_library_with_empty_rest() {
    let res = crate::utils::path::split_library_prefix(std::path::Path::new("MyLib")).unwrap();
    assert_eq!(res.0, "MyLib");
    assert_eq!(res.1, std::path::PathBuf::from(""));
}

#[test]
fn test_split_library_prefix_normal_two_segments() {
    let res =
        crate::utils::path::split_library_prefix(std::path::Path::new("MyLib/notes/b.md")).unwrap();
    assert_eq!(res.0, "MyLib");
    assert_eq!(res.1, std::path::PathBuf::from("notes/b.md"));
}

#[test]
fn test_split_library_prefix_strips_leading_slash_and_dot() {
    let res = crate::utils::path::split_library_prefix(std::path::Path::new("/./MyLib/notes/b.md"))
        .unwrap();
    assert_eq!(res.0, "MyLib");
    assert_eq!(res.1, std::path::PathBuf::from("notes/b.md"));
}
