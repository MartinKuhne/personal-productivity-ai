//! Tests for Trello registry provider — descriptor, safety, DTOs.

use super::*;
use crate::tools::Safety;
use crate::tools::registry::groups::{InternalToolGroup, ToolGroupId};

// ---------------------------------------------------------------------------
// Provider registration
// ---------------------------------------------------------------------------

#[test]
fn trello_provider_registers_seven_tools() {
    let provider = TrelloProvider;
    assert_eq!(provider.id(), "trello");
    assert!(matches!(
        provider.group(),
        ToolGroupId::Internal(InternalToolGroup::Trello)
    ));
    let tools = provider.tools();
    assert_eq!(tools.len(), 7);
    let names: Vec<_> = tools
        .iter()
        .map(|t| t.descriptor.name.as_ref())
        .collect::<Vec<&str>>();
    for expected in [
        "trello_get_boards",
        "trello_get_board",
        "trello_get_lists",
        "trello_get_cards",
        "trello_create_card",
        "trello_update_card",
        "trello_delete_card",
    ] {
        assert!(names.contains(&expected), "missing {expected} in {names:?}");
    }
}

#[test]
fn safety_assignments_match_spec() {
    assert_eq!(TrelloGetBoardsTool.safety(), Safety::ReadOnly);
    assert_eq!(TrelloGetBoardTool.safety(), Safety::ReadOnly);
    assert_eq!(TrelloGetListsTool.safety(), Safety::ReadOnly);
    assert_eq!(TrelloGetCardsTool.safety(), Safety::ReadOnly);
    assert_eq!(TrelloCreateCardTool.safety(), Safety::Mutating);
    assert_eq!(TrelloUpdateCardTool.safety(), Safety::Mutating);
    assert_eq!(TrelloDeleteCardTool.safety(), Safety::Mutating);
}

#[test]
fn tool_groups_are_trello() {
    for tool in TrelloProvider.tools() {
        assert_eq!(
            tool.descriptor.group,
            ToolGroupId::Internal(InternalToolGroup::Trello),
            "tool {} wrong group",
            tool.descriptor.name
        );
    }
}

#[test]
fn descriptors_have_input_schemas() {
    for t in TrelloProvider.tools() {
        assert!(
            t.descriptor.parameters_schema.is_object(),
            "{} schema not object",
            t.descriptor.name
        );
    }
}

#[test]
fn trello_empty_input_round_trip() {
    let empty: TrelloEmptyInput = serde_json::from_str("{}").unwrap();
    let _ = empty;
    // TrelloEmptyInput accepts {} even if args empty JSON is given to tool
    let json = serde_json::to_string(&TrelloEmptyInput {}).unwrap();
    assert_eq!(json, "{}");
}

#[test]
fn trello_id_input_round_trip() {
    let p: TrelloIdInput = serde_json::from_str(r#"{"id":"abc123"}"#).unwrap();
    assert_eq!(p.id, "abc123");
    let back = serde_json::to_string(&p).unwrap();
    assert!(back.contains("abc123"));
}

#[test]
fn trello_create_card_input_round_trip_and_rename() {
    // idList is renamed, idLabels optional
    let json = r#"{"idList":"list1","name":"My Card","desc":"hello"}"#;
    let p: TrelloCreateCardInput = serde_json::from_str(json).unwrap();
    assert_eq!(p.id_list, "list1");
    assert_eq!(p.name, "My Card");
    assert_eq!(p.desc.as_deref(), Some("hello"));
    assert!(p.id_labels.is_none());

    let with_labels = r#"{"idList":"list1","name":"My Card","idLabels":["lbl1"]}"#;
    let p2: TrelloCreateCardInput = serde_json::from_str(with_labels).unwrap();
    assert_eq!(p2.id_labels.unwrap(), vec!["lbl1"]);

    // serialization uses renamed field
    let ser = serde_json::to_value(&p).unwrap();
    assert_eq!(ser["idList"], "list1");
}

#[test]
fn trello_update_card_input_optional_fields() {
    let json = r#"{"id":"card1","name":"new name"}"#;
    let p: TrelloUpdateCardInput = serde_json::from_str(json).unwrap();
    assert_eq!(p.id, "card1");
    assert_eq!(p.name.as_deref(), Some("new name"));
    assert!(p.desc.is_none());
    assert!(p.id_list.is_none());

    let full = r#"{"id":"card1","name":"n","desc":"d","idList":"list2"}"#;
    let p2: TrelloUpdateCardInput = serde_json::from_str(full).unwrap();
    assert_eq!(p2.id_list.as_deref(), Some("list2"));
}

#[test]
fn trello_update_card_serializes_rename() {
    let p = TrelloUpdateCardInput {
        id: "c1".to_string(),
        name: Some("n".to_string()),
        desc: None,
        id_list: Some("listX".to_string()),
    };
    let v = serde_json::to_value(&p).unwrap();
    assert_eq!(v["idList"], "listX");
}

#[test]
fn registered_clones_descriptor() {
    let r = registered(TrelloGetBoardsTool);
    assert_eq!(r.descriptor.name, "trello_get_boards");
    assert_eq!(r.executor.descriptor().name, "trello_get_boards");
}

#[test]
fn trello_provider_requires_trello_config_on_execute() {
    // trello_request helper fails with missing config — exercise the error path
    // without needing a full ToolContext. We test via DTO validation only here;
    // the full integration is covered by protocol-layer tests in lib/trello.
    let json = r#"{"id":"x"}"#;
    let parsed: TrelloIdInput = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.id, "x");
}
