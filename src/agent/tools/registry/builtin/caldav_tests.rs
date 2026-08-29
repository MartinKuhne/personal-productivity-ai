//! Tests for CalDAV registry provider — descriptor, safety, DTOs.

use super::*;
use crate::tools::Safety;
use crate::tools::registry::groups::{InternalToolGroup, ToolGroupId};

#[test]
fn caldav_provider_registers_six_tools() {
    let provider = CalDavProvider;
    assert_eq!(provider.id(), "caldav");
    assert!(matches!(
        provider.group(),
        ToolGroupId::Internal(InternalToolGroup::Calendar)
    ));
    let tools = provider.tools();
    assert_eq!(tools.len(), 6);
    let names: Vec<_> = tools
        .iter()
        .map(|t| t.descriptor.name.as_ref())
        .collect::<Vec<&str>>();
    for expected in [
        "search_calendar",
        "get_calendar",
        "get_calendar_item",
        "add_calendar_item",
        "update_calendar_item",
        "delete_calendar_item",
    ] {
        assert!(names.contains(&expected), "missing {expected} in {names:?}");
    }
}

#[test]
fn safety_assignments_match_spec() {
    assert_eq!(SearchCalendarTool.safety(), Safety::ReadOnly);
    assert_eq!(GetCalendarTool.safety(), Safety::ReadOnly);
    assert_eq!(GetCalendarItemTool.safety(), Safety::ReadOnly);
    assert_eq!(AddCalendarItemTool.safety(), Safety::Mutating);
    assert_eq!(UpdateCalendarItemTool.safety(), Safety::Mutating);
    assert_eq!(DeleteCalendarItemTool.safety(), Safety::Mutating);
}

#[test]
fn tool_groups_are_calendar() {
    for tool in CalDavProvider.tools() {
        assert_eq!(
            tool.descriptor.group,
            ToolGroupId::Internal(InternalToolGroup::Calendar),
            "tool {} wrong group",
            tool.descriptor.name
        );
    }
}

#[test]
fn descriptors_have_input_schemas() {
    for t in CalDavProvider.tools() {
        assert!(
            t.descriptor.parameters_schema.is_object(),
            "{} schema not object",
            t.descriptor.name
        );
    }
}

#[test]
fn dto_search_calendar_round_trip() {
    let p: dtos::SearchCalendarInput = serde_json::from_str(r#"{"keyword":"meeting"}"#).unwrap();
    assert_eq!(p.keyword, "meeting");
    assert!(p.cursor.is_none());
    let with_cursor: dtos::SearchCalendarInput =
        serde_json::from_str(r#"{"keyword":"meeting","cursor":"c1"}"#).unwrap();
    assert_eq!(with_cursor.cursor.as_deref(), Some("c1"));
}

#[test]
fn dto_get_calendar_round_trip() {
    let p: dtos::GetCalendarInput =
        serde_json::from_str(r#"{"start_date":"2024-01-01","end_date":"2024-01-31"}"#).unwrap();
    assert_eq!(p.start_date, "2024-01-01");
    assert_eq!(p.end_date, "2024-01-31");
}

#[test]
fn dto_get_calendar_item_round_trip() {
    let p: dtos::GetCalendarItemInput =
        serde_json::from_str(r#"{"href":"/cal/event1.ics"}"#).unwrap();
    assert_eq!(p.href, "/cal/event1.ics");
}

#[test]
fn dto_add_calendar_item_round_trip() {
    let p: dtos::AddCalendarItemInput =
        serde_json::from_str(r#"{"summary":"Standup","start":"2024-01-01T09:00:00Z"}"#).unwrap();
    assert_eq!(p.summary.as_deref(), Some("Standup"));
    let v = serde_json::to_value(&p).unwrap();
    assert_eq!(v["summary"], "Standup");
}

#[test]
fn dto_update_calendar_item_round_trip() {
    let p: dtos::UpdateCalendarItemInput =
        serde_json::from_str(r#"{"id":"/cal/event1.ics","summary":"New"}"#).unwrap();
    assert_eq!(p.id, "/cal/event1.ics");
    assert_eq!(p.summary.as_deref(), Some("New"));
}

#[test]
fn dto_delete_calendar_item_round_trip() {
    let p: dtos::DeleteCalendarItemInput =
        serde_json::from_str(r#"{"id":"/cal/event1.ics"}"#).unwrap();
    assert_eq!(p.id, "/cal/event1.ics");
}

#[test]
fn registered_clones_descriptor() {
    let r = registered(SearchCalendarTool);
    assert_eq!(r.descriptor.name, "search_calendar");
    assert_eq!(r.executor.descriptor().name, "search_calendar");
}
