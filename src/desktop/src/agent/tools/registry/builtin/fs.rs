//! Filesystem tool implementations and provider for the tool registry.

use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::dtos;
use crate::agent::tools::provider::{RegisteredTool, ToolProvider};
use crate::agent::tools::registry::groups::{InternalToolGroup, ToolGroupId};
use crate::app::vfs::behaviour::ContentLibraryExt;
use std::sync::Arc;

use super::super::pagination::paginate_in_range;
use super::strings;

/// Default `limit` for the `read_lines` tool. 0-indexed line slice;
/// the first call without args returns the first 100 lines of the
/// file. Matches the list-paginated tools' default for vocabulary
/// consistency.
const DEFAULT_WINDOW_NOTE_LIMIT: usize = 100;

/// Tool that replaces exact text occurrences in a file.
pub(crate) struct PatchNoteTool;
impl Tool for PatchNoteTool {
    crate::tool_descriptor! {
        name: "patch_note",
        desc: strings::PATCH_NOTE_DESCRIPTION,
        input: dtos::PatchNoteInput,
        safety: crate::agent::tools::Safety::Mutating,
        group: Filesystem,
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::PatchNoteInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let path = ctx.resolve_writable(&input.path)?;
        if ctx.is_pdf_backed(&path) {
            return Err(crate::ui::strings::PDF_BACKED_ERROR.to_string());
        }
        let producer = ctx.file_event_producer();
        crate::agent::tools::filesystem::tool_patch_note(
            &path.to_string_lossy(),
            &input.old_string,
            &input.new_string,
            &producer,
        )
        .map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that greps query strings case-insensitively across Markdown files.
pub(crate) struct SearchNotesTool;
impl Tool for SearchNotesTool {
    crate::tool_descriptor! {
        name: "search_notes",
        desc: strings::SEARCH_NOTES_DESCRIPTION,
        input: dtos::SearchNotesInput,
        safety: crate::agent::tools::Safety::ReadOnly,
        group: Filesystem,
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::SearchNotesInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let mut all_matches: Vec<String> = Vec::new();
        let mut libs: Vec<_> = ctx.config.content_libraries.iter().collect();
        libs.sort_by_key(|b| std::cmp::Reverse(b.priority));
        for lib in libs {
            if let Ok(mut matches) = crate::agent::tools::filesystem::tool_search_notes(
                &lib.root_path(),
                &lib.name,
                &input.query,
            ) {
                all_matches.append(&mut matches);
            }
        }
        let total = all_matches.len();
        let limit = crate::agent::tools::filesystem::DEFAULT_SEARCH_NOTES_MAX_RESULTS;
        let truncated = total > limit;
        all_matches.truncate(limit);
        if truncated {
            all_matches.push(format!(
                "... (results truncated at {limit} matches; refine the query with narrower terms or delegate to a sub-agent to analyse a specific file)"
            ));
        }
        let matches = if all_matches.is_empty() {
            "No matches found.".to_string()
        } else {
            all_matches.join("\n")
        };
        Ok(serde_json::to_value(dtos::SearchNotesResponse {
            matches,
            total,
            truncated,
        })
        .unwrap())
    }
}

/// Tool that reads all unique tags defined in front-matter headers.
pub(crate) struct ReadTagsTool;
impl Tool for ReadTagsTool {
    crate::tool_descriptor! {
        name: "read_tags",
        desc: strings::READ_TAGS_DESCRIPTION,
        input: dtos::ReadTagsInput,
        safety: crate::agent::tools::Safety::ReadOnly,
        group: Filesystem,
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let _: dtos::ReadTagsInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let mut all_tags = std::collections::BTreeSet::new();
        for lib in &ctx.config.content_libraries {
            if let Ok(res) = crate::agent::tools::filesystem::tool_read_tags(&lib.root_path()) {
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
}

/// Tool that lists Markdown files containing a specific front-matter tag.
pub(crate) struct ListNotesByTagTool;
impl Tool for ListNotesByTagTool {
    crate::tool_descriptor! {
        name: "list_notes_by_tag",
        desc: strings::LIST_NOTES_BY_TAG_DESCRIPTION,
        input: dtos::ListNotesByTagInput,
        safety: crate::agent::tools::Safety::ReadOnly,
        group: Filesystem,
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::ListNotesByTagInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let offset = input.offset.unwrap_or(0);
        let limit = input
            .limit
            .unwrap_or(super::super::pagination::DEFAULT_LIST_NOTES_BY_TAG_LIMIT);
        let mut all_matches: Vec<String> = Vec::new();
        for lib in &ctx.config.content_libraries {
            match crate::agent::tools::filesystem::tool_list_notes_by_tag(
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
        let total = all_matches.len();
        let (page_files, hint) =
            paginate_in_range(&all_matches, offset, limit, total, "tagged files");
        Ok(serde_json::to_value(dtos::ListNotesByTagResponse {
            files: page_files,
            total,
            hint,
        })
        .unwrap())
    }
}

/// Tool that lists Markdown files in a directory.
pub(crate) struct ListNotesTool;
impl Tool for ListNotesTool {
    crate::tool_descriptor! {
        name: "list_notes",
        desc: strings::LIST_NOTES_DESCRIPTION,
        input: dtos::ListNotesInput,
        safety: crate::agent::tools::Safety::ReadOnly,
        group: Filesystem,
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::ListNotesInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let offset = input.offset.unwrap_or(0);
        let limit = input
            .limit
            .unwrap_or(super::super::pagination::DEFAULT_LIST_NOTES_BY_TAG_LIMIT);
        let all_matches: Vec<String> = match ctx.resolve_virtual_path(&input.path, false)? {
            Some((path, _)) => {
                crate::agent::tools::filesystem::tool_list_notes(&path, &input.path)?
            }
            None => {
                let mut libs: Vec<String> = ctx
                    .config
                    .content_libraries
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
}

/// Tool that reads the entire content of a file.
pub(crate) struct ReadNoteTool;
impl Tool for ReadNoteTool {
    crate::tool_descriptor! {
        name: "read_note",
        desc: strings::READ_NOTE_DESCRIPTION,
        input: dtos::ReadNoteInput,
        safety: crate::agent::tools::Safety::ReadOnly,
        group: Filesystem,
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::ReadNoteInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let (path, _) = ctx
            .resolve_virtual_path(&input.path, false)?
            .ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
        crate::agent::tools::filesystem::tool_read_note(&path.to_string_lossy()).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that reads a contiguous slice of lines from a file.
pub(crate) struct WindowNoteTool;
impl Tool for WindowNoteTool {
    crate::tool_descriptor! {
        name: "window_note",
        desc: strings::WINDOW_NOTE_DESCRIPTION,
        input: dtos::WindowNoteInput,
        safety: crate::agent::tools::Safety::ReadOnly,
        group: Filesystem,
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::WindowNoteInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let (path, _) = ctx
            .resolve_virtual_path(&input.path, false)?
            .ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
        let offset = input.offset.unwrap_or(0);
        let limit = input.limit.unwrap_or(DEFAULT_WINDOW_NOTE_LIMIT);
        crate::agent::tools::filesystem::tool_window_note(&path.to_string_lossy(), offset, limit)
            .map(|r| {
                serde_json::to_value(r)
                    .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
            })
    }
}

/// Tool that creates a new file.
pub(crate) struct CreateNoteTool;
impl Tool for CreateNoteTool {
    crate::tool_descriptor! {
        name: "create_note",
        desc: strings::CREATE_NOTE_DESCRIPTION,
        input: dtos::CreateNoteInput,
        safety: crate::agent::tools::Safety::Mutating,
        group: Filesystem,
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::CreateNoteInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let path = ctx.resolve_writable(&input.path)?;
        if ctx.is_pdf_backed(&path) {
            return Err(crate::ui::strings::PDF_BACKED_ERROR.to_string());
        }
        let producer = ctx.file_event_producer();
        crate::agent::tools::filesystem::tool_create_note(
            &path.to_string_lossy(),
            &input.content,
            &producer,
        )
        .map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that inserts lines into a file at a 0-indexed offset.
pub(crate) struct InsertIntoNoteTool;
impl Tool for InsertIntoNoteTool {
    crate::tool_descriptor! {
        name: "insert_into_note",
        desc: strings::INSERT_INTO_NOTE_DESCRIPTION,
        input: dtos::InsertIntoNoteInput,
        safety: crate::agent::tools::Safety::Mutating,
        group: Filesystem,
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::InsertIntoNoteInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let path = ctx.resolve_writable(&input.path)?;
        if ctx.is_pdf_backed(&path) {
            return Err(crate::ui::strings::PDF_BACKED_ERROR.to_string());
        }
        let producer = ctx.file_event_producer();
        crate::agent::tools::filesystem::tool_insert_into_note(
            &path.to_string_lossy(),
            input.offset,
            &input.lines,
            &producer,
        )
        .map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
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
        ]
    }
}

fn registered<T: Tool + 'static>(tool: T) -> RegisteredTool {
    RegisteredTool {
        descriptor: Arc::new(tool.descriptor().clone()),
        executor: Arc::new(tool),
    }
}
