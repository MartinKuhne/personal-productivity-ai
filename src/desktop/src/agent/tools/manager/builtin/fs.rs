//! Filesystem tool implementations for the tool registry.

use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::dtos;
use crate::app::vfs::behaviour::ContentLibraryExt;
use crate::config::AppConfig;
use std::any::TypeId;

use super::super::pagination::paginate_in_range;
use super::json_schema;
use super::strings;

/// Default `limit` for the `read_lines` tool. 0-indexed line slice;
/// the first call without args returns the first 100 lines of the
/// file. Matches the list-paginated tools' default for vocabulary
/// consistency.
const DEFAULT_WINDOW_NOTE_LIMIT: usize = 100;

/// Tool that replaces exact text occurrences in a file.
pub(crate) struct PatchNoteTool;
impl Tool for PatchNoteTool {
    fn name(&self) -> &'static str {
        "patch_note"
    }
    fn description(&self) -> &'static str {
        strings::PATCH_NOTE_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::PatchNoteInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::PatchNoteInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.filesystem
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
    fn name(&self) -> &'static str {
        "search_notes"
    }
    fn description(&self) -> &'static str {
        strings::SEARCH_NOTES_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::SearchNotesInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::SearchNotesInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.filesystem
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
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
    fn name(&self) -> &'static str {
        "read_tags"
    }
    fn description(&self) -> &'static str {
        strings::READ_TAGS_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::ReadTagsInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::ReadTagsInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.filesystem
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
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
    fn name(&self) -> &'static str {
        "list_notes_by_tag"
    }
    fn description(&self) -> &'static str {
        strings::LIST_NOTES_BY_TAG_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::ListNotesByTagInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::ListNotesByTagInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.filesystem
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
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
    fn name(&self) -> &'static str {
        "list_notes"
    }
    fn description(&self) -> &'static str {
        strings::LIST_NOTES_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::ListNotesInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::ListNotesInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.filesystem
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
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
    fn name(&self) -> &'static str {
        "read_note"
    }
    fn description(&self) -> &'static str {
        strings::READ_NOTE_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::ReadNoteInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::ReadNoteInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.filesystem
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
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
    fn name(&self) -> &'static str {
        "window_note"
    }
    fn description(&self) -> &'static str {
        strings::WINDOW_NOTE_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::WindowNoteInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::WindowNoteInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.filesystem
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
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
    fn name(&self) -> &'static str {
        "create_note"
    }
    fn description(&self) -> &'static str {
        strings::CREATE_NOTE_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::CreateNoteInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::CreateNoteInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.filesystem
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
    fn name(&self) -> &'static str {
        "insert_into_note"
    }
    fn description(&self) -> &'static str {
        strings::INSERT_INTO_NOTE_DESCRIPTION
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::InsertIntoNoteInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::InsertIntoNoteInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.filesystem
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
