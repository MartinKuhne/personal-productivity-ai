//! Tests for YAML registry provider — descriptor, safety, DTOs.

use super::*;
use crate::tools::Safety;
use crate::tools::registry::groups::{InternalToolGroup, ToolGroupId};

#[test]
fn yaml_provider_registers_two_tools() {
    let provider = YamlProvider;
    assert_eq!(provider.id(), "yaml");
    assert!(matches!(
        provider.group(),
        ToolGroupId::Internal(InternalToolGroup::Filesystem)
    ));
    let tools = provider.tools();
    assert_eq!(tools.len(), 2);
    let names: Vec<_> = tools
        .iter()
        .map(|t| t.descriptor.name.as_ref())
        .collect::<Vec<&str>>();
    assert!(names.contains(&"read_yaml_header"));
    assert!(names.contains(&"write_yaml_header"));
}

#[test]
fn safety_assignments_match_spec() {
    assert_eq!(ReadYamlHeaderTool.safety(), Safety::ReadOnly);
    assert_eq!(WriteYamlHeaderTool.safety(), Safety::Mutating);
}

#[test]
fn tool_groups_are_filesystem() {
    for tool in YamlProvider.tools() {
        assert_eq!(
            tool.descriptor.group,
            ToolGroupId::Internal(InternalToolGroup::Filesystem),
            "tool {} wrong group",
            tool.descriptor.name
        );
    }
}

#[test]
fn descriptors_have_input_schemas() {
    for t in YamlProvider.tools() {
        assert!(
            t.descriptor.parameters_schema.is_object(),
            "{} schema not object",
            t.descriptor.name
        );
    }
}

#[test]
fn dto_read_yaml_header_round_trip() {
    let p: dtos::ReadYamlHeaderInput = serde_json::from_str(r#"{"path":"a/b.md"}"#).unwrap();
    assert_eq!(p.path, "a/b.md");
}

#[test]
fn dto_write_yaml_header_round_trip() {
    let p: dtos::WriteYamlHeaderInput =
        serde_json::from_str(r#"{"path":"a/b.md","title":"T","tags":["t1"]}"#).unwrap();
    assert_eq!(p.path, "a/b.md");
    assert_eq!(p.title.as_deref(), Some("T"));
    assert_eq!(p.tags.unwrap(), vec!["t1"]);
    // header-date rename
    let with_date: dtos::WriteYamlHeaderInput =
        serde_json::from_str(r#"{"path":"a/b.md","header-date":"2024-01-01"}"#).unwrap();
    assert_eq!(with_date.header_date.as_deref(), Some("2024-01-01"));
}

#[test]
fn registered_clones_descriptor() {
    let r = registered(ReadYamlHeaderTool);
    assert_eq!(r.descriptor.name, "read_yaml_header");
    assert_eq!(r.executor.descriptor().name, "read_yaml_header");
}
