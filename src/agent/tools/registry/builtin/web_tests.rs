//! Tests for web registry provider — descriptor, safety, DTOs, config gate.

use super::*;
use crate::tools::Safety;
use crate::tools::registry::groups::{InternalToolGroup, ToolGroupId};

#[test]
fn web_provider_registers_three_tools() {
    let provider = WebProvider;
    assert_eq!(provider.id(), "web");
    assert!(matches!(
        provider.group(),
        ToolGroupId::Internal(InternalToolGroup::Web)
    ));
    let tools = provider.tools();
    assert_eq!(tools.len(), 3);
    let names: Vec<_> = tools
        .iter()
        .map(|t| t.descriptor.name.as_ref())
        .collect::<Vec<&str>>();
    for expected in ["web_delegate", "web_fetch", "web_search"] {
        assert!(names.contains(&expected), "missing {expected} in {names:?}");
    }
}

#[test]
fn safety_assignments_match_spec() {
    assert_eq!(WebDelegateTool.safety(), Safety::Mutating);
    assert_eq!(WebFetchTool.safety(), Safety::ReadOnly);
    assert_eq!(WebSearchTool.safety(), Safety::ReadOnly);
}

#[test]
fn tool_groups_are_web() {
    for tool in WebProvider.tools() {
        assert_eq!(
            tool.descriptor.group,
            ToolGroupId::Internal(InternalToolGroup::Web),
            "tool {} wrong group",
            tool.descriptor.name
        );
    }
}

#[test]
fn web_search_requires_searxng_config() {
    let cfg = WebSearchTool.descriptor().config.clone();
    assert_eq!(
        cfg.group,
        Some(ToolGroupId::Internal(InternalToolGroup::Web))
    );
    assert!(
        cfg.requires.iter().any(|p| matches!(
            p,
            crate::tools::descriptor::ConfigPredicate::SearxngConfigured
        )),
        "web_search must require SearxngConfigured"
    );
}

#[test]
fn descriptors_have_input_schemas() {
    for t in WebProvider.tools() {
        assert!(
            t.descriptor.parameters_schema.is_object(),
            "{} schema not object",
            t.descriptor.name
        );
    }
}

#[test]
fn dto_web_delegate_round_trip() {
    let p: dtos::WebDelegateInput = serde_json::from_str(r#"{"instruction":"find X"}"#).unwrap();
    assert_eq!(p.instruction, "find X");
}

#[test]
fn dto_web_fetch_round_trip() {
    let p: dtos::WebFetchInput = serde_json::from_str(r#"{"url":"https://example.com"}"#).unwrap();
    assert_eq!(p.url, "https://example.com");
    assert!(!p.headers);
    let with_headers: dtos::WebFetchInput =
        serde_json::from_str(r#"{"url":"https://example.com","headers":true,"cursor":"c"}"#)
            .unwrap();
    assert!(with_headers.headers);
    assert_eq!(with_headers.cursor.as_deref(), Some("c"));
}

#[test]
fn dto_web_search_round_trip() {
    let p: dtos::WebSearchInput = serde_json::from_str(r#"{"query":"hello"}"#).unwrap();
    assert_eq!(p.query, "hello");
    assert!(p.cursor.is_none());
}

#[test]
fn registered_clones_descriptor() {
    let r = registered(WebDelegateTool);
    assert_eq!(r.descriptor.name, "web_delegate");
    assert_eq!(r.executor.descriptor().name, "web_delegate");
}
