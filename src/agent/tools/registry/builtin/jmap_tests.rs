//! Tests for JMAP registry provider — descriptor, safety, DTOs, email spec.

use super::*;
use crate::tools::Safety;
use crate::tools::registry::groups::{InternalToolGroup, ToolGroupId};

#[test]
fn jmap_provider_registers_three_tools() {
    let provider = JmapProvider;
    assert_eq!(provider.id(), "jmap");
    assert!(matches!(
        provider.group(),
        ToolGroupId::Internal(InternalToolGroup::Email)
    ));
    let tools = provider.tools();
    assert_eq!(tools.len(), 3);
    let names: Vec<_> = tools
        .iter()
        .map(|t| t.descriptor.name.as_ref())
        .collect::<Vec<&str>>();
    for expected in ["search_email", "get_email_by_id", "send_email"] {
        assert!(names.contains(&expected), "missing {expected} in {names:?}");
    }
}

#[test]
fn safety_assignments_match_spec() {
    assert_eq!(SearchEmailTool.safety(), Safety::ReadOnly);
    assert_eq!(GetEmailByIdTool.safety(), Safety::ReadOnly);
    assert_eq!(SendEmailTool.safety(), Safety::Mutating);
}

#[test]
fn tool_groups_are_email() {
    for tool in JmapProvider.tools() {
        assert_eq!(
            tool.descriptor.group,
            ToolGroupId::Internal(InternalToolGroup::Email),
            "tool {} wrong group",
            tool.descriptor.name
        );
    }
}

#[test]
fn descriptors_require_email_config() {
    for t in JmapProvider.tools() {
        assert!(
            t.descriptor.config.requires.iter().any(|p| matches!(
                p,
                crate::tools::descriptor::ConfigPredicate::JmapClientsPresent
            )),
            "{} missing JmapClientsPresent",
            t.descriptor.name
        );
    }
}

#[test]
fn descriptors_have_input_schemas() {
    for t in JmapProvider.tools() {
        assert!(
            t.descriptor.parameters_schema.is_object(),
            "{} schema not object",
            t.descriptor.name
        );
    }
}

#[test]
fn dto_search_email_round_trip() {
    let p: dtos::SearchEmailInput =
        serde_json::from_str(r#"{"keyword":"hello","folder":"INBOX"}"#).unwrap();
    assert_eq!(p.keyword.as_deref(), Some("hello"));
    assert_eq!(p.folder.as_deref(), Some("INBOX"));
    let empty: dtos::SearchEmailInput = serde_json::from_str(r#"{}"#).unwrap();
    assert!(empty.keyword.is_none());
}

#[test]
fn dto_get_email_by_id_round_trip() {
    let p: dtos::GetEmailByIdInput = serde_json::from_str(r#"{"id":"abc"}"#).unwrap();
    assert_eq!(p.id, "abc");
}

#[test]
fn dto_send_email_round_trip() {
    let p: dtos::SendEmailInput =
        serde_json::from_str(r#"{"to":"a@b.com","subject":"hi","body":"hello"}"#).unwrap();
    assert_eq!(p.to, "a@b.com");
    assert_eq!(p.subject, "hi");
}

#[test]
fn registered_clones_descriptor() {
    let r = registered(SearchEmailTool);
    assert_eq!(r.descriptor.name, "search_email");
    assert_eq!(r.executor.descriptor().name, "search_email");
}
