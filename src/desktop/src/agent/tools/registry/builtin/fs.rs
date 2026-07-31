//! Filesystem tool implementations for the tool registry.

use crate::agent::tools::Tool;
use crate::agent::tools::context::ToolContext;
use crate::agent::tools::dtos;
use crate::app::vfs::library::ContentLibraryExt;
use crate::config::AppConfig;
use std::any::TypeId;

use super::super::pagination::paginate_in_range;
use super::json_schema;

/// Tool that replaces exact text occurrences in a file.
pub(crate) struct ReplaceTextTool;
impl Tool for ReplaceTextTool {
    fn name(&self) -> &'static str {
        "replace_text"
    }
    fn description(&self) -> &'static str {
        "Replace exact occurrences of old_string with new_string in a file."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::ReplaceTextInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::ReplaceTextInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.filesystem
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::ReplaceTextInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let path = ctx.resolve_writable(&input.path)?;
        let producer = ctx.file_event_producer();
        crate::agent::tools::filesystem::tool_replace_text(
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
pub(crate) struct GrepTool;
impl Tool for GrepTool {
    fn name(&self) -> &'static str {
        "grep"
    }
    fn description(&self) -> &'static str {
        "Search for a query string case-insensitively across all Markdown files in the workspace. Returns at most 200 matching lines; when the result is truncated, refine the query with narrower terms or delegate to a sub-agent to analyse a specific file."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::GrepInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::GrepInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.filesystem
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::GrepInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let mut all_matches: Vec<String> = Vec::new();
        let mut libs: Vec<_> = ctx.config.content_libraries.iter().collect();
        libs.sort_by_key(|b| std::cmp::Reverse(b.priority));
        for lib in libs {
            if let Ok(mut matches) = crate::agent::tools::filesystem::tool_grep(
                &lib.root_path(),
                &lib.name,
                &input.query,
            ) {
                all_matches.append(&mut matches);
            }
        }
        let total = all_matches.len();
        let limit = crate::agent::tools::filesystem::DEFAULT_GREP_MAX_RESULTS;
        let truncated = total > limit;
        all_matches.truncate(limit);
        if truncated {
            all_matches.push(format!(
                "... (results truncated at {} matches; refine the query with narrower terms or delegate to a sub-agent to analyse a specific file)",
                limit
            ));
        }
        let matches = if all_matches.is_empty() {
            "No matches found.".to_string()
        } else {
            all_matches.join("\n")
        };
        Ok(serde_json::to_value(dtos::GrepResponse {
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
        "Get all unique tags defined in front-matter headers of all Markdown files in the workspace."
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
pub(crate) struct ListFilesByTagTool;
impl Tool for ListFilesByTagTool {
    fn name(&self) -> &'static str {
        "list_files_by_tag"
    }
    fn description(&self) -> &'static str {
        "List Markdown files that contain a specific tag in their front-matter. Results are returned as a JSON array, paginated across all configured libraries (default page size 20); every response includes the total number of matching files so the caller can drive follow-up page requests."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::ListFilesByTagInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::ListFilesByTagInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.filesystem
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::ListFilesByTagInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let page = input.page.unwrap_or(1).max(1);
        let page_size = input
            .page_size
            .unwrap_or(crate::agent::tools::filesystem::DEFAULT_LIST_FILES_BY_TAG_PAGE_SIZE)
            .max(1);
        let mut all_matches: Vec<String> = Vec::new();
        for lib in &ctx.config.content_libraries {
            match crate::agent::tools::filesystem::tool_list_files_by_tag(
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
            paginate_in_range(&all_matches, page, page_size, total, "tagged files");
        Ok(serde_json::to_value(dtos::ListFilesByTagResponse {
            files: page_files,
            total,
            hint,
        })
        .unwrap())
    }
}

/// Tool that lists Markdown files in a directory.
pub(crate) struct ListFilesTool;
impl Tool for ListFilesTool {
    fn name(&self) -> &'static str {
        "list_files"
    }
    fn description(&self) -> &'static str {
        "List Markdown files in a directory (not recursive). Results are returned as a JSON array, paginated (default page size 20); every response includes the total number of files in the directory so the caller can drive follow-up page requests. With `path` set to \"/\" or \".\" returns the configured content libraries."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::ListFilesInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::ListFilesInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.filesystem
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::ListFilesInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let page = input.page.unwrap_or(1).max(1);
        let page_size = input
            .page_size
            .unwrap_or(crate::agent::tools::filesystem::DEFAULT_LIST_FILES_BY_TAG_PAGE_SIZE)
            .max(1);
        let all_matches: Vec<String> = match ctx.resolve_virtual_path(&input.path, false)? {
            Some((path, _)) => {
                crate::agent::tools::filesystem::tool_list_files(&path, &input.path)?
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
        let (page_files, hint) = paginate_in_range(&all_matches, page, page_size, total, plural);
        Ok(serde_json::to_value(dtos::ListFilesResponse {
            files: page_files,
            total,
            hint,
        })
        .unwrap())
    }
}

/// Tool that reads the entire content of a file.
pub(crate) struct ReadFileTool;
impl Tool for ReadFileTool {
    fn name(&self) -> &'static str {
        "read_file"
    }
    fn description(&self) -> &'static str {
        "Read the entire text contents of a file at the specified path. Prefer using the read_yaml_header tool if just a document summary is needed."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::ReadFileInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::ReadFileInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.filesystem
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::ReadFileInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let (path, _) = ctx
            .resolve_virtual_path(&input.path, false)?
            .ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
        crate::agent::tools::filesystem::tool_read_file(&path.to_string_lossy()).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that reads specific lines from a file.
pub(crate) struct ReadFileLinesTool;
impl Tool for ReadFileLinesTool {
    fn name(&self) -> &'static str {
        "read_file_lines"
    }
    fn description(&self) -> &'static str {
        "Read specific lines from a file (1-indexed)."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::ReadFileLinesInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::ReadFileLinesInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.filesystem
    }
    fn safety(&self) -> crate::agent::tools::Safety {
        crate::agent::tools::Safety::ReadOnly
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::ReadFileLinesInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let (path, _) = ctx
            .resolve_virtual_path(&input.path, false)?
            .ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
        crate::agent::tools::filesystem::tool_read_file_lines(
            &path.to_string_lossy(),
            input.start_line,
            input.end_line,
        )
        .map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that creates a new file.
pub(crate) struct CreateFileTool;
impl Tool for CreateFileTool {
    fn name(&self) -> &'static str {
        "create_file"
    }
    fn description(&self) -> &'static str {
        "Create a new file at the specified path with the provided content."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::CreateFileInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::CreateFileInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.filesystem
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::CreateFileInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let path = ctx.resolve_writable(&input.path)?;
        let producer = ctx.file_event_producer();
        crate::agent::tools::filesystem::tool_create_file(
            &path.to_string_lossy(),
            &input.content,
            &producer,
        )
        .map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that inserts lines into a file at a specific index.
pub(crate) struct InsertLinesTool;
impl Tool for InsertLinesTool {
    fn name(&self) -> &'static str {
        "insert_lines"
    }
    fn description(&self) -> &'static str {
        "Insert lines into a file at a specific 1-indexed line index."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::InsertLinesInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::InsertLinesInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.filesystem
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::InsertLinesInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let path = ctx.resolve_writable(&input.path)?;
        let producer = ctx.file_event_producer();
        crate::agent::tools::filesystem::tool_insert_lines(
            &path.to_string_lossy(),
            input.line_index,
            &input.lines,
            &producer,
        )
        .map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

/// Tool that deletes specific lines from a file.
pub(crate) struct DeleteLinesTool;
impl Tool for DeleteLinesTool {
    fn name(&self) -> &'static str {
        "delete_lines"
    }
    fn description(&self) -> &'static str {
        "Delete specific lines from a file (1-indexed, inclusive)."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::DeleteLinesInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        json_schema::<dtos::DeleteLinesInput>()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.tool_groups.filesystem
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::DeleteLinesInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let path = ctx.resolve_writable(&input.path)?;
        let producer = ctx.file_event_producer();
        crate::agent::tools::filesystem::tool_delete_lines(
            &path.to_string_lossy(),
            input.start_line,
            input.end_line,
            &producer,
        )
        .map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}
