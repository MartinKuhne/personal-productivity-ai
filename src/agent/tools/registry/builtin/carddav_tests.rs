//! Tests for CardDAV registry provider — descriptor, safety, DTOs, disabled spec.

use super::*;
use crate::tools::Safety;
use crate::tools::registry::groups::{InternalToolGroup, ToolGroupId};

#[test]
fn carddav_provider_registers_five_tools() {
    let provider = CardDavProvider;
    assert_eq!(provider.id(), "carddav");
    assert!(matches!(
        provider.group(),
        ToolGroupId::Internal(InternalToolGroup::Contacts)
    ));
    let tools = provider.tools();
    assert_eq!(tools.len(), 5);
    let names: Vec<_> = tools
        .iter()
        .map(|t| t.descriptor.name.as_ref())
        .collect::<Vec<&str>>();
    for expected in [
        "search_contact",
        "add_contact",
        "get_contact",
        "update_contact",
        "delete_contact",
    ] {
        assert!(names.contains(&expected), "missing {expected} in {names:?}");
    }
}

#[test]
fn safety_assignments_match_spec() {
    assert_eq!(SearchContactTool.safety(), Safety::ReadOnly);
    assert_eq!(GetContactTool.safety(), Safety::ReadOnly);
    assert_eq!(AddContactTool.safety(), Safety::Mutating);
    assert_eq!(UpdateContactTool.safety(), Safety::Mutating);
    assert_eq!(DeleteContactTool.safety(), Safety::Mutating);
}

#[test]
fn tool_groups_are_contacts() {
    // Search/Add/Get/Update use the normal contacts_spec group; Delete uses
    // the disabled spec but still reports the Contacts group.
    for tool in CardDavProvider.tools() {
        assert_eq!(
            tool.descriptor.group,
            ToolGroupId::Internal(InternalToolGroup::Contacts),
            "tool {} wrong group",
            tool.descriptor.name
        );
    }
}

#[test]
fn delete_contact_is_disabled_via_never_predicate() {
    // DeleteContact should never be enabled — it uses ConfigPredicate::Never.
    let cfg = DeleteContactTool.descriptor().config.clone();
    assert_eq!(
        cfg.group,
        Some(ToolGroupId::Internal(InternalToolGroup::Contacts))
    );
    assert!(
        cfg.requires
            .iter()
            .any(|p| matches!(p, crate::tools::descriptor::ConfigPredicate::Never)),
        "delete_contact spec must contain Never predicate"
    );
    // Direct helper also returns the same shape.
    let direct = delete_contact_disabled_spec(ToolGroupId::Internal(InternalToolGroup::Contacts));
    assert!(
        direct
            .requires
            .iter()
            .any(|p| matches!(p, crate::tools::descriptor::ConfigPredicate::Never)),
        "direct helper must contain Never"
    );
}

#[test]
fn descriptors_have_input_schemas() {
    for t in CardDavProvider.tools() {
        assert!(
            t.descriptor.parameters_schema.is_object(),
            "{} schema not object",
            t.descriptor.name
        );
    }
}

#[test]
fn dto_search_contact_round_trip() {
    let p: dtos::SearchContactInput = serde_json::from_str(r#"{"keyword":"alice"}"#).unwrap();
    assert_eq!(p.keyword, "alice");
    assert!(p.cursor.is_none());
}

#[test]
fn dto_add_contact_round_trip() {
    let p: dtos::AddContactInput =
        serde_json::from_str(r#"{"client":"personal","name":"Alice","email":"a@example.com"}"#)
            .unwrap();
    assert_eq!(p.client.as_deref(), Some("personal"));
    assert_eq!(p.name.as_deref(), Some("Alice"));
    assert_eq!(p.email.as_deref(), Some("a@example.com"));
}

#[test]
fn dto_get_contact_round_trip() {
    let p1: dtos::GetContactInput = serde_json::from_str(r#"{"id":"/addr/1.vcf"}"#).unwrap();
    assert_eq!(p1.href, "/addr/1.vcf");

    let p2: dtos::GetContactInput = serde_json::from_str(r#"{"href":"/addr/1.vcf"}"#).unwrap();
    assert_eq!(p2.href, "/addr/1.vcf");
}

#[test]
fn dto_update_contact_round_trip() {
    let p1: dtos::UpdateContactInput =
        serde_json::from_str(r#"{"id":"/addr/1.vcf","email":"new@example.com","client":"work"}"#)
            .unwrap();
    assert_eq!(p1.href, "/addr/1.vcf");
    assert_eq!(p1.client.as_deref(), Some("work"));
    assert_eq!(p1.email.as_deref(), Some("new@example.com"));

    let p2: dtos::UpdateContactInput =
        serde_json::from_str(r#"{"href":"/addr/1.vcf","email":"new@example.com"}"#).unwrap();
    assert_eq!(p2.href, "/addr/1.vcf");
}

#[test]
fn dto_delete_contact_round_trip() {
    let p1: dtos::DeleteContactInput =
        serde_json::from_str(r#"{"id":"/addr/1.vcf","client":"work"}"#).unwrap();
    assert_eq!(p1.href, "/addr/1.vcf");
    assert_eq!(p1.client.as_deref(), Some("work"));

    let p2: dtos::DeleteContactInput = serde_json::from_str(r#"{"href":"/addr/1.vcf"}"#).unwrap();
    assert_eq!(p2.href, "/addr/1.vcf");
}

#[test]
fn registered_clones_descriptor() {
    let r = registered(SearchContactTool);
    assert_eq!(r.descriptor.name, "search_contact");
    assert_eq!(r.executor.descriptor().name, "search_contact");
}
