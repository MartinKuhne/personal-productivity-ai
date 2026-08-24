//! Filesystem tool implementations and provider for the tool registry.

use crate::config::ContentLibraryExt;
use crate::tools::Tool;
use crate::tools::context::ToolContext;
use crate::tools::dtos;
use crate::tools::provider::{RegisteredTool, ToolProvider};
use crate::tools::registry::groups::{InternalToolGroup, ToolGroupId};
use fastmd_tool_macros::ToolDescriptor;
use std::sync::Arc;

use super::super::pagination::paginate_in_range;
use super::strings;

/// Default `limit` for the `read_lines` tool. 0-indexed line slice;
/// the first call without args returns the first 100 lines of the
/// file. Matches the list-paginated tools' default for vocabulary
/// consistency.
const DEFAULT_WINDOW_NOTE_LIMIT: usize = 100;

/// Tool that replaces exact text occurrences in a file.
#[derive(ToolDescriptor)]
#[tool(
    name = "patch_note",
    desc = strings::PATCH_NOTE_DESCRIPTION,
    input = dtos::PatchNoteInput,
    safety = crate::tools::Safety::Mutating,
    group = Filesystem,
    execute_with = execute_patch_note,
)]
pub(crate) struct PatchNoteTool;
fn execute_patch_note(
    _self: &PatchNoteTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::PatchNoteInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    let path = ctx.resolve_writable(&input.path)?;
    ctx.check_write_allowed(&path)?;
    let observer = ctx.file_observer();
    crate::tools::filesystem::tool_patch_note(
        ctx,
        &path.to_string_lossy(),
        &input.old_string,
        &input.new_string,
        &*observer,
    )
    .map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Tool that greps query strings case-insensitively across Markdown files.
#[derive(ToolDescriptor)]
#[tool(
    name = "search_notes",
    desc = strings::SEARCH_NOTES_DESCRIPTION,
    input = dtos::SearchNotesInput,
    safety = crate::tools::Safety::ReadOnly,
    group = Filesystem,
    execute_with = execute_search_notes,
)]
pub(crate) struct SearchNotesTool;
fn execute_search_notes(
    _self: &SearchNotesTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::SearchNotesInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;

    if let Some(cursor) = &input.cursor {
        let page = ctx.cache().search_notes_sessions.next_page(cursor)?;
        let matches = if page.items.is_empty() {
            "No matches found.".to_string()
        } else {
            page.items.join("\n")
        };
        return Ok(serde_json::to_value(dtos::SearchNotesResponse {
            matches,
            total: page.total,
            cursor: page.cursor,
            hint: page.hint,
        })
        .unwrap());
    }

    let mut all_matches: Vec<String> = Vec::new();
    let mut libs: Vec<_> = ctx.config.content_libraries().iter().collect();
    libs.sort_by_key(|b| std::cmp::Reverse(b.priority));
    for lib in libs {
        if let Ok(mut matches) = crate::tools::filesystem::tool_search_notes(
            ctx,
            &lib.root_path(),
            &lib.name,
            &input.query,
        ) {
            all_matches.append(&mut matches);
        }
    }

    if all_matches.is_empty() {
        return Ok(serde_json::to_value(dtos::SearchNotesResponse {
            matches: "No matches found.".to_string(),
            total: 0,
            cursor: None,
            hint: Some(strings::FINAL_PAGE_HINT.to_string()),
        })
        .unwrap());
    }

    let page = ctx
        .cache()
        .search_notes_sessions
        .create_session(all_matches, ctx.uuid_gen().as_ref());
    Ok(serde_json::to_value(dtos::SearchNotesResponse {
        matches: page.items.join("\n"),
        total: page.total,
        cursor: page.cursor,
        hint: page.hint,
    })
    .unwrap())
}

/// Tool that reads all unique tags defined in front-matter headers.
#[derive(ToolDescriptor)]
#[tool(
    name = "read_tags",
    desc = strings::READ_TAGS_DESCRIPTION,
    input = dtos::ReadTagsInput,
    safety = crate::tools::Safety::ReadOnly,
    group = Filesystem,
    execute_with = execute_read_tags,
)]
pub(crate) struct ReadTagsTool;
fn execute_read_tags(
    _self: &ReadTagsTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let _: dtos::ReadTagsInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    let mut all_tags = std::collections::BTreeSet::new();
    for lib in ctx.config.content_libraries() {
        if let Ok(res) = crate::tools::filesystem::tool_read_tags(ctx, &lib.root_path()) {
            for tag in res.tags {
                all_tags.insert(tag);
            }
        }
    }
    Ok(serde_json::to_value(dtos::ReadTagsResponse {
        tags: all_tags.into_iter().collect(),
    })
    .unwrap())
}

/// Tool that lists Markdown files containing a specific front-matter tag.
#[derive(ToolDescriptor)]
#[tool(
    name = "list_notes_by_tag",
    desc = strings::LIST_NOTES_BY_TAG_DESCRIPTION,
    input = dtos::ListNotesByTagInput,
    safety = crate::tools::Safety::ReadOnly,
    group = Filesystem,
    execute_with = execute_list_notes_by_tag,
)]
pub(crate) struct ListNotesByTagTool;
fn execute_list_notes_by_tag(
    _self: &ListNotesByTagTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::ListNotesByTagInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;

    if let Some(cursor) = &input.cursor {
        let page = ctx.cache().list_notes_by_tag_sessions.next_page(cursor)?;
        return Ok(serde_json::to_value(dtos::ListNotesByTagResponse {
            files: page.items,
            total: page.total,
            cursor: page.cursor,
            hint: page.hint,
        })
        .unwrap());
    }

    let mut all_matches: Vec<String> = Vec::new();
    for lib in ctx.config.content_libraries() {
        match crate::tools::filesystem::tool_list_notes_by_tag(
            ctx,
            &lib.root_path(),
            &lib.name,
            &input.tag,
        ) {
            Ok(mut files) => all_matches.append(&mut files),
            Err(e) => {
                tracing::warn!(name = "tool.list_files_by_tag.lib_failed", lib = %lib.name, error = %e, "list_files_by_tag failed for a single library; continuing with the others")
            }
        }
    }
    all_matches.sort();
    all_matches.dedup();

    if all_matches.is_empty() {
        return Ok(serde_json::to_value(dtos::ListNotesByTagResponse {
            files: Vec::new(),
            total: 0,
            cursor: None,
            hint: Some(strings::FINAL_PAGE_HINT.to_string()),
        })
        .unwrap());
    }

    let page = ctx
        .cache()
        .list_notes_by_tag_sessions
        .create_session(all_matches, ctx.uuid_gen().as_ref());
    Ok(serde_json::to_value(dtos::ListNotesByTagResponse {
        files: page.items,
        total: page.total,
        cursor: page.cursor,
        hint: page.hint,
    })
    .unwrap())
}

/// Tool that lists Markdown files in a directory.
#[derive(ToolDescriptor)]
#[tool(
    name = "list_notes",
    desc = strings::LIST_NOTES_DESCRIPTION,
    input = dtos::ListNotesInput,
    safety = crate::tools::Safety::ReadOnly,
    group = Filesystem,
    execute_with = execute_list_notes,
)]
pub(crate) struct ListNotesTool;
fn execute_list_notes(
    _self: &ListNotesTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::ListNotesInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    let offset = input.offset.unwrap_or(0);
    let limit = input
        .limit
        .unwrap_or(super::super::pagination::DEFAULT_LIST_NOTES_BY_TAG_LIMIT);
    let all_matches: Vec<String> = match ctx.resolve_virtual_path(&input.path, false)? {
        Some(resolved) => {
            crate::tools::filesystem::tool_list_notes(ctx, &resolved.path, &input.path)?
        }
        None => {
            let mut libs: Vec<String> = ctx
                .config
                .content_libraries()
                .iter()
                .map(|lib| lib.name.clone())
                .collect();
            libs.sort();
            libs
        }
    };
    let total = all_matches.len();
    let plural = if input.path == "/" || input.path == "." {
        "libraries"
    } else {
        "files"
    };
    let (page_files, hint) = paginate_in_range(&all_matches, offset, limit, total, plural);
    Ok(serde_json::to_value(dtos::ListNotesResponse {
        files: page_files,
        total,
        hint,
    })
    .unwrap())
}

/// Tool that reads the entire content of a file.
#[derive(ToolDescriptor)]
#[tool(
    name = "read_note",
    desc = strings::READ_NOTE_DESCRIPTION,
    input = dtos::ReadNoteInput,
    safety = crate::tools::Safety::ReadOnly,
    group = Filesystem,
    execute_with = execute_read_note,
)]
pub(crate) struct ReadNoteTool;
fn execute_read_note(
    _self: &ReadNoteTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::ReadNoteInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    let path = ctx
        .resolve_virtual_path(&input.path, false)?
        .ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?
        .path;
    crate::tools::filesystem::tool_read_note(ctx, &path.to_string_lossy()).map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Tool that reads a contiguous slice of lines from a file.
#[derive(ToolDescriptor)]
#[tool(
    name = "window_note",
    desc = strings::WINDOW_NOTE_DESCRIPTION,
    input = dtos::WindowNoteInput,
    safety = crate::tools::Safety::ReadOnly,
    group = Filesystem,
    execute_with = execute_window_note,
)]
pub(crate) struct WindowNoteTool;
fn execute_window_note(
    _self: &WindowNoteTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::WindowNoteInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    let path = ctx
        .resolve_virtual_path(&input.path, false)?
        .ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?
        .path;
    let offset = input.offset.unwrap_or(0);
    let limit = input.limit.unwrap_or(DEFAULT_WINDOW_NOTE_LIMIT);
    crate::tools::filesystem::tool_window_note(ctx, &path.to_string_lossy(), offset, limit).map(
        |r| serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()})),
    )
}

/// Tool that creates a new file.
#[derive(ToolDescriptor)]
#[tool(
    name = "create_note",
    desc = strings::CREATE_NOTE_DESCRIPTION,
    input = dtos::CreateNoteInput,
    safety = crate::tools::Safety::Mutating,
    group = Filesystem,
    execute_with = execute_create_note,
)]
pub(crate) struct CreateNoteTool;
fn execute_create_note(
    _self: &CreateNoteTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::CreateNoteInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    let path = ctx.resolve_writable(&input.path)?;
    ctx.check_write_allowed(&path)?;
    let observer = ctx.file_observer();
    crate::tools::filesystem::tool_create_note(
        ctx,
        &path.to_string_lossy(),
        &input.content,
        &*observer,
    )
    .map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Tool that inserts lines into a file at a 0-indexed offset.
#[derive(ToolDescriptor)]
#[tool(
    name = "insert_into_note",
    desc = strings::INSERT_INTO_NOTE_DESCRIPTION,
    input = dtos::InsertIntoNoteInput,
    safety = crate::tools::Safety::Mutating,
    group = Filesystem,
    execute_with = execute_insert_into_note,
)]
pub(crate) struct InsertIntoNoteTool;
fn execute_insert_into_note(
    _self: &InsertIntoNoteTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::InsertIntoNoteInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    let path = ctx.resolve_writable(&input.path)?;
    ctx.check_write_allowed(&path)?;
    let observer = ctx.file_observer();
    crate::tools::filesystem::tool_insert_into_note(
        ctx,
        &path.to_string_lossy(),
        input.offset,
        &input.lines,
        &*observer,
    )
    .map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Tool that moves or renames a markdown-formatted note.
#[derive(ToolDescriptor)]
#[tool(
    name = "move_note",
    desc = strings::MOVE_NOTE_DESCRIPTION,
    input = dtos::MoveNoteInput,
    safety = crate::tools::Safety::Mutating,
    group = Filesystem,
    execute_with = execute_move_note,
)]
pub(crate) struct MoveNoteTool;
fn execute_move_note(
    _self: &MoveNoteTool,
    ctx: &ToolContext,
    args: &str,
) -> Result<serde_json::Value, String> {
    let input: dtos::MoveNoteInput =
        serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
    let source_path = ctx.resolve_writable(&input.source)?;
    ctx.check_write_allowed(&source_path)?;
    let target_path = ctx.resolve_writable(&input.target)?;
    ctx.check_write_allowed(&target_path)?;
    let observer = ctx.file_observer();
    crate::tools::filesystem::tool_move_note(
        ctx,
        &source_path.to_string_lossy(),
        &target_path.to_string_lossy(),
        &*observer,
    )
    .map(|r| {
        serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
    })
}

/// Self-registering provider for the filesystem family.
pub(crate) struct FilesystemProvider;
impl ToolProvider for FilesystemProvider {
    fn id(&self) -> &'static str {
        "filesystem"
    }
    fn group(&self) -> ToolGroupId {
        ToolGroupId::Internal(InternalToolGroup::Filesystem)
    }
    fn tools(&self) -> Vec<RegisteredTool> {
        vec![
            registered(PatchNoteTool),
            registered(SearchNotesTool),
            registered(ReadTagsTool),
            registered(ListNotesByTagTool),
            registered(ListNotesTool),
            registered(ReadNoteTool),
            registered(WindowNoteTool),
            registered(CreateNoteTool),
            registered(InsertIntoNoteTool),
            registered(MoveNoteTool),
        ]
    }
}

fn registered<T: Tool + 'static>(tool: T) -> RegisteredTool {
    RegisteredTool {
        descriptor: Arc::new(tool.descriptor().clone()),
        executor: Arc::new(tool),
    }
}
