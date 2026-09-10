//! Tests for filesystem registry provider — descriptor, safety, DTOs, pagination.

use super::*;
use crate::tools::Safety;
use crate::tools::registry::groups::ToolGroupId;

// ---------------------------------------------------------------------------
// Provider registration
// ---------------------------------------------------------------------------

#[test]
fn filesystem_provider_registers_ten_tools() {
    let provider = FilesystemProvider;
    assert_eq!(provider.id(), "filesystem");
    assert!(matches!(
        provider.group(),
        ToolGroupId::Internal(crate::tools::registry::groups::InternalToolGroup::Filesystem)
    ));
    let tools = provider.tools();
    assert_eq!(tools.len(), 10, "expected 10 filesystem tools");
    let names: Vec<_> = tools
        .iter()
        .map(|t| t.descriptor.name.as_ref())
        .collect::<Vec<&str>>();
    for expected in [
        "patch_note",
        "search_notes",
        "read_tags",
        "list_notes_by_tag",
        "list_notes",
        "read_note",
        "window_note",
        "create_note",
        "insert_into_note",
        "move_note",
    ] {
        assert!(names.contains(&expected), "missing {expected} in {names:?}");
    }
}

#[test]
fn safety_assignments_match_spec() {
    // Mutating: patch/create/insert/move
    assert_eq!(PatchNoteTool.safety(), Safety::Mutating);
    assert_eq!(CreateNoteTool.safety(), Safety::Mutating);
    assert_eq!(InsertIntoNoteTool.safety(), Safety::Mutating);
    assert_eq!(MoveNoteTool.safety(), Safety::Mutating);
    // ReadOnly: search/read/list/window
    for s in [
        SearchNotesTool.safety(),
        ReadTagsTool.safety(),
        ListNotesByTagTool.safety(),
        ListNotesTool.safety(),
        ReadNoteTool.safety(),
        WindowNoteTool.safety(),
    ] {
        assert_eq!(s, Safety::ReadOnly);
    }
}

#[test]
fn tool_groups_are_filesystem() {
    for tool in FilesystemProvider.tools() {
        assert_eq!(
            tool.descriptor.group,
            ToolGroupId::Internal(crate::tools::registry::groups::InternalToolGroup::Filesystem),
            "tool {} wrong group",
            tool.descriptor.name
        );
    }
}

#[test]
fn descriptors_have_input_schemas() {
    // Every descriptor must expose a JSON schema — regression guard.
    for t in FilesystemProvider.tools() {
        let schema = t.descriptor.parameters_schema.clone();
        // schema is a serde_json::Value; ensure it's an object.
        assert!(
            schema.is_object(),
            "{} schema not object",
            t.descriptor.name
        );
    }
}

#[test]
fn default_window_limit_is_100() {
    assert_eq!(DEFAULT_WINDOW_NOTE_LIMIT, 100);
}

// ---------------------------------------------------------------------------
// DTO round-trips — filesystem family
// ---------------------------------------------------------------------------

#[test]
fn dto_patch_note_round_trip() {
    let json = r#"{"path":"a/b.md","old_string":"foo","new_string":"bar"}"#;
    let p: dtos::PatchNoteInput = serde_json::from_str(json).unwrap();
    assert_eq!(p.path, "a/b.md");
    assert_eq!(p.old_string, "foo");
    assert_eq!(p.new_string, "bar");
}

#[test]
fn dto_search_notes_cursor_optional() {
    let without: dtos::SearchNotesInput = serde_json::from_str(r#"{"query":"hello"}"#).unwrap();
    assert!(without.cursor.is_none());
    let with: dtos::SearchNotesInput =
        serde_json::from_str(r#"{"query":"hello","cursor":"c1"}"#).unwrap();
    assert_eq!(with.cursor.as_deref(), Some("c1"));
}

#[test]
fn dto_list_notes_pagination_fields() {
    let j = r#"{"path":"/","offset":10,"limit":5}"#;
    let p: dtos::ListNotesInput = serde_json::from_str(j).unwrap();
    assert_eq!(p.offset, Some(10));
    assert_eq!(p.limit, Some(5));
    let j2 = r#"{"path":"/"}"#;
    let p2: dtos::ListNotesInput = serde_json::from_str(j2).unwrap();
    assert!(p2.offset.is_none());
    assert!(p2.limit.is_none());
}

#[test]
fn dto_read_note_requires_path() {
    let p: dtos::ReadNoteInput = serde_json::from_str(r#"{"path":"x.md"}"#).unwrap();
    assert_eq!(p.path, "x.md");
}

#[test]
fn execute_search_notes_uses_cursor_and_ignores_query() {
    let mut ctx = crate::tools::context::ToolContext::default();
    let cache = std::sync::Arc::new(crate::tools::registry::cache::ToolCache::new());
    ctx.extensions
        .insert(std::sync::Arc::new(crate::tools::context::ToolCacheExt(
            cache,
        )));
    let uuid = crate::utils::uuid::SystemUuidGenerator;
    let items: Vec<String> = (0..70).map(|i| format!("note line {i}")).collect();
    let first_page = ctx
        .cache()
        .search_notes_sessions
        .create_session(items, &uuid);
    let cursor = first_page.cursor.expect("session should have cursor");

    let res = super::execute_search_notes(
        &super::SearchNotesTool,
        &ctx,
        &format!(r#"{{"query":"ignored_query","cursor":"{cursor}"}}"#),
    )
    .expect("execute_search_notes should succeed with cursor even if query is present");

    assert_eq!(res["count"], 6);
    assert_eq!(res["total"], 70);
}

#[test]
fn execute_search_notes_rejects_missing_query_and_cursor() {
    let ctx = crate::tools::context::ToolContext::default();
    let err = super::execute_search_notes(&super::SearchNotesTool, &ctx, r#"{}"#).unwrap_err();
    assert!(err.contains("Give `query`"), "unexpected error: {err}");
}

#[test]
fn execute_list_notes_by_tag_uses_cursor_and_ignores_tag() {
    let mut ctx = crate::tools::context::ToolContext::default();
    let cache = std::sync::Arc::new(crate::tools::registry::cache::ToolCache::new());
    ctx.extensions
        .insert(std::sync::Arc::new(crate::tools::context::ToolCacheExt(
            cache,
        )));
    let uuid = crate::utils::uuid::SystemUuidGenerator;
    let items: Vec<String> = (0..70).map(|i| format!("notes/note_{i}.md")).collect();
    let first_page = ctx
        .cache()
        .list_notes_by_tag_sessions
        .create_session(items, &uuid);
    let cursor = first_page.cursor.expect("session should have cursor");

    let res = super::execute_list_notes_by_tag(
        &super::ListNotesByTagTool,
        &ctx,
        &format!(r#"{{"tag":"ignored_tag","cursor":"{cursor}"}}"#),
    )
    .expect("execute_list_notes_by_tag should succeed with cursor even if tag is present");

    assert_eq!(res["count"], 6);
    assert_eq!(res["total"], 70);
}

#[test]
fn execute_list_notes_by_tag_rejects_missing_tag_and_cursor() {
    let ctx = crate::tools::context::ToolContext::default();
    let err =
        super::execute_list_notes_by_tag(&super::ListNotesByTagTool, &ctx, r#"{}"#).unwrap_err();
    assert!(err.contains("Give `tag`"), "unexpected error: {err}");
}

#[test]
fn dto_window_note_defaults() {
    let p: dtos::WindowNoteInput = serde_json::from_str(r#"{"path":"x.md"}"#).unwrap();
    assert!(p.offset.is_none());
    assert!(p.limit.is_none());
    let p2: dtos::WindowNoteInput =
        serde_json::from_str(r#"{"path":"x.md","offset":2,"limit":20}"#).unwrap();
    assert_eq!(p2.offset, Some(2));
    assert_eq!(p2.limit, Some(20));
}

#[test]
fn dto_create_note_round_trip() {
    let j = r#"{"path":"n.md","content":"hello"}"#;
    let p: dtos::CreateNoteInput = serde_json::from_str(j).unwrap();
    assert_eq!(p.content, "hello");
}

#[test]
fn dto_insert_into_note_round_trip() {
    let j = r#"{"path":"n.md","offset":3,"lines":["a","b"]}"#;
    let p: dtos::InsertIntoNoteInput = serde_json::from_str(j).unwrap();
    assert_eq!(p.offset, 3);
    assert_eq!(p.lines, vec!["a", "b"]);
}

#[test]
fn dto_move_note_aliases() {
    // canonical source/target
    let p: dtos::MoveNoteInput =
        serde_json::from_str(r#"{"source":"a.md","target":"b.md"}"#).unwrap();
    assert_eq!(p.source, "a.md");
    assert_eq!(p.target, "b.md");
    // alias source_path/from and target_path/destination/to
    let p2: dtos::MoveNoteInput =
        serde_json::from_str(r#"{"source_path":"a.md","to":"b.md"}"#).unwrap();
    assert_eq!(p2.source, "a.md");
    assert_eq!(p2.target, "b.md");
    let p3: dtos::MoveNoteInput =
        serde_json::from_str(r#"{"from":"a.md","destination":"b.md"}"#).unwrap();
    assert_eq!(p3.source, "a.md");
    assert_eq!(p3.target, "b.md");
}

#[test]
fn dto_read_tags_and_list_by_tag() {
    let p: dtos::ReadTagsInput = serde_json::from_str(r#"{}"#).unwrap();
    let _ = p;
    let p2: dtos::ListNotesByTagInput = serde_json::from_str(r#"{"tag":"rust"}"#).unwrap();
    assert_eq!(p2.tag.as_deref(), Some("rust"));
    assert!(p2.cursor.is_none());
    let p3: dtos::ListNotesByTagInput =
        serde_json::from_str(r#"{"tag":"rust","cursor":"c"}"#).unwrap();
    assert_eq!(p3.cursor.as_deref(), Some("c"));
}

// ---------------------------------------------------------------------------
// registered helper — descriptor cloned independently
// ---------------------------------------------------------------------------

#[test]
fn registered_clones_descriptor() {
    let r = registered(PatchNoteTool);
    assert_eq!(r.descriptor.name, "patch_note");
    // executor holds same descriptor
    assert_eq!(r.executor.descriptor().name, "patch_note");
}
