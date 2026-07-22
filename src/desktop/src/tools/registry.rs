//! Tool registry — registers all available tools, dispatches execution by name, and produces the JSON-Schema tool list for the LLM.

use crate::config::AppConfig;
use crate::tools::context::ToolContext;
use crate::tools::Tool;
use std::collections::HashMap;

pub(crate) fn paginate_in_range<T: Clone>(
    items: &[T],
    page: usize,
    page_size: usize,
    total: usize,
    plural: &str,
) -> (Vec<T>, Option<String>) {
    if total == 0 {
        return (Vec::new(), Some(format!("No matching {plural} found.")));
    }
    let start = (page - 1).saturating_mul(page_size);
    if start >= total {
        return (
            Vec::new(),
            Some(format!(
                "No {plural} on page {page} (showing 0 of {total} total, page_size: {page_size})."
            )),
        );
    }
    let end = (start + page_size).min(total);
    (items[start..end].to_vec(), None)
}

pub struct ToolRegistry {
    tools: HashMap<&'static str, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
        };
        registry.register_all();
        registry
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        self.tools.insert(tool.name(), tool);
    }

    pub fn execute(
        &self,
        ctx: &ToolContext,
        name: &str,
        args: &str,
    ) -> Result<serde_json::Value, String> {
        let tool = self
            .tools
            .get(name)
            .ok_or_else(|| format!("Tool {} not found.", name))?;
        tool.execute(ctx, args)
    }

    pub fn get_schema(&self, config: &AppConfig, prompt: &str) -> serde_json::Value {
        let mut tools = Vec::new();
        for tool in self.tools.values() {
            if tool.is_enabled(config, prompt) {
                tools.push(serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool.name(),
                        "description": tool.description(),
                        "parameters": tool.parameters_schema()
                    }
                }));
            }
        }
        serde_json::Value::Array(tools)
    }

    fn register_all(&mut self) {
        self.register(Box::new(WebDelegateTool));
        self.register(Box::new(ReplaceTextTool));
        self.register(Box::new(GrepTool));
        self.register(Box::new(ReadTagsTool));
        self.register(Box::new(ListFilesByTagTool));
        self.register(Box::new(ListFilesTool));
        self.register(Box::new(ReadFileTool));
        self.register(Box::new(ReadFileLinesTool));
        self.register(Box::new(CreateFileTool));
        self.register(Box::new(InsertLinesTool));
        self.register(Box::new(DeleteLinesTool));
        self.register(Box::new(WebFetchTool));
        self.register(Box::new(ReadYamlHeaderTool));
        self.register(Box::new(WriteYamlHeaderTool));
        self.register(Box::new(WebSearchTool));
        self.register(Box::new(SearchCalendarTool));
        self.register(Box::new(GetCalendarTool));
        self.register(Box::new(GetCalendarItemTool));
        self.register(Box::new(AddCalendarItemTool));
        self.register(Box::new(UpdateCalendarItemTool));
        self.register(Box::new(DeleteCalendarItemTool));
        self.register(Box::new(SearchEmailTool));
        self.register(Box::new(GetEmailByIdTool));
        self.register(Box::new(SendEmailTool));
        self.register(Box::new(SearchContactTool));
        self.register(Box::new(AddContactTool));
        self.register(Box::new(GetContactTool));
        self.register(Box::new(CsvCreateTool));
        self.register(Box::new(CsvListTool));
        self.register(Box::new(CsvAddRowsTool));
        self.register(Box::new(CsvDeleteRowsTool));
        self.register(Box::new(CsvQueryTool));
    }
}

static TOOL_REGISTRY: std::sync::LazyLock<ToolRegistry> =
    std::sync::LazyLock::new(ToolRegistry::new);

pub fn get_tools_schema(config: &AppConfig, prompt: &str) -> serde_json::Value {
    TOOL_REGISTRY.get_schema(config, prompt)
}

pub fn execute_tool(ctx: &ToolContext, name: &str, args_str: &str) -> String {
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    let debug_mode = ctx
        .config
        .feature_flags
        .get("toolCallDebugMode")
        .copied()
        .unwrap_or(false);

    tracing::info!(name = "tool.registry.call", tool_name = %name, args = %args_str, "Executing tool call");
    let start_time = std::time::Instant::now();

    let result_raw: Result<serde_json::Value, String> =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            TOOL_REGISTRY.execute(ctx, name, args_str)
        }))
        .unwrap_or_else(|e| {
            let msg = if let Some(s) = e.downcast_ref::<&str>() {
                *s
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.as_str()
            } else {
                "Unknown panic"
            };
            Err(format!("Tool {} panicked: {}", name, msg))
        });

    let elapsed = start_time.elapsed();
    let response_dto = match result_raw {
        Ok(data) => {
            if debug_mode {
                let data_str = serde_json::to_string(&data)
                    .unwrap_or_else(|_| "<serialization error>".to_string());
                tracing::info!(name = "tool.registry.success", tool_name = %name, elapsed = ?elapsed, data = %data_str, "Tool execution succeeded");
            } else {
                tracing::info!(name = "tool.registry.success", tool_name = %name, elapsed = ?elapsed, "Tool execution succeeded");
            }
            crate::tools::dtos::ToolResponse::Success { data }
        }
        Err(err) => {
            tracing::error!(name = "tool.registry.failed", tool_name = %name, elapsed = ?elapsed, error = %err, "Tool execution failed. Operator should verify tool inputs.");
            crate::tools::dtos::ToolResponse::Error { message: err }
        }
    };

    serde_json::to_string(&response_dto).unwrap_or_else(|_| {
        r#"{"status":"error","message":"Failed to serialize tool response"}"#.to_string()
    })
}

// --- Tool struct implementations ---

use crate::tools::dtos;
use std::any::TypeId;

struct WebDelegateTool;
impl Tool for WebDelegateTool {
    fn name(&self) -> &'static str {
        "web_delegate"
    }
    fn description(&self) -> &'static str {
        "Delegate web searches and web fetches to a sub-agent. This protects your context window. Give clear instructions and it will return summarized information."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::WebDelegateInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(dtos::WebDelegateInput)).unwrap()
    }
    fn is_enabled(&self, _: &AppConfig, _: &str) -> bool {
        true
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::WebDelegateInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::tools::web::tool_web_delegate(ctx.config, &input.instruction).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

struct ReplaceTextTool;
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
        serde_json::to_value(schemars::schema_for!(dtos::ReplaceTextInput)).unwrap()
    }
    fn is_enabled(&self, _: &AppConfig, _: &str) -> bool {
        true
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::ReplaceTextInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let (path, readonly) = ctx
            .resolve_virtual_path(&input.path, true)?
            .ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
        if readonly {
            return Err("Cannot perform this operation on a read-only library".to_string());
        }
        let producer = ctx.file_event_producer();
        crate::tools::filesystem::tool_replace_text(
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

struct GrepTool;
impl Tool for GrepTool {
    fn name(&self) -> &'static str {
        "grep"
    }
    fn description(&self) -> &'static str {
        "Search for a query string case-insensitively across all Markdown files in the workspace."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::GrepInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(dtos::GrepInput)).unwrap()
    }
    fn is_enabled(&self, _: &AppConfig, _: &str) -> bool {
        true
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::GrepInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let mut all_results = Vec::new();
        let mut libs: Vec<_> = ctx.config.content_libraries.iter().collect();
        libs.sort_by(|a, b| b.priority.cmp(&a.priority));
        for lib in libs {
            if let Ok(res) =
                crate::tools::filesystem::tool_grep(&lib.root_path(), &lib.name, &input.query)
            {
                if res.matches != "No matches found." {
                    all_results.push(res.matches);
                }
            }
        }
        if all_results.is_empty() {
            Ok(serde_json::to_value(dtos::GrepResponse {
                matches: "No matches found.".to_string(),
            })
            .unwrap())
        } else {
            Ok(serde_json::to_value(dtos::GrepResponse {
                matches: all_results.join("\n"),
            })
            .unwrap())
        }
    }
}

struct ReadTagsTool;
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
        serde_json::to_value(schemars::schema_for!(dtos::ReadTagsInput)).unwrap()
    }
    fn is_enabled(&self, _: &AppConfig, _: &str) -> bool {
        true
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let _: dtos::ReadTagsInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let mut all_tags = std::collections::BTreeSet::new();
        for lib in &ctx.config.content_libraries {
            if let Ok(res) = crate::tools::filesystem::tool_read_tags(&lib.root_path()) {
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

struct ListFilesByTagTool;
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
        serde_json::to_value(schemars::schema_for!(dtos::ListFilesByTagInput)).unwrap()
    }
    fn is_enabled(&self, _: &AppConfig, _: &str) -> bool {
        true
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::ListFilesByTagInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let page = input.page.unwrap_or(1).max(1);
        let page_size = input
            .page_size
            .unwrap_or(crate::tools::filesystem::DEFAULT_LIST_FILES_BY_TAG_PAGE_SIZE)
            .max(1);
        let mut all_matches: Vec<String> = Vec::new();
        for lib in &ctx.config.content_libraries {
            match crate::tools::filesystem::tool_list_files_by_tag(
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

struct ListFilesTool;
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
        serde_json::to_value(schemars::schema_for!(dtos::ListFilesInput)).unwrap()
    }
    fn is_enabled(&self, _: &AppConfig, _: &str) -> bool {
        true
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::ListFilesInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let page = input.page.unwrap_or(1).max(1);
        let page_size = input
            .page_size
            .unwrap_or(crate::tools::filesystem::DEFAULT_LIST_FILES_BY_TAG_PAGE_SIZE)
            .max(1);
        let all_matches: Vec<String> = match ctx.resolve_virtual_path(&input.path, false)? {
            Some((path, _)) => crate::tools::filesystem::tool_list_files(&path, &input.path)?,
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

struct ReadFileTool;
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
        serde_json::to_value(schemars::schema_for!(dtos::ReadFileInput)).unwrap()
    }
    fn is_enabled(&self, _: &AppConfig, _: &str) -> bool {
        true
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::ReadFileInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let (path, _) = ctx
            .resolve_virtual_path(&input.path, false)?
            .ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
        crate::tools::filesystem::tool_read_file(&path.to_string_lossy()).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

struct ReadFileLinesTool;
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
        serde_json::to_value(schemars::schema_for!(dtos::ReadFileLinesInput)).unwrap()
    }
    fn is_enabled(&self, _: &AppConfig, _: &str) -> bool {
        true
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::ReadFileLinesInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let (path, _) = ctx
            .resolve_virtual_path(&input.path, false)?
            .ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
        crate::tools::filesystem::tool_read_file_lines(
            &path.to_string_lossy(),
            input.start_line,
            input.end_line,
        )
        .map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

struct CreateFileTool;
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
        serde_json::to_value(schemars::schema_for!(dtos::CreateFileInput)).unwrap()
    }
    fn is_enabled(&self, _: &AppConfig, _: &str) -> bool {
        true
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::CreateFileInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let (path, readonly) = ctx
            .resolve_virtual_path(&input.path, true)?
            .ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
        if readonly {
            return Err("Cannot perform this operation on a read-only library".to_string());
        }
        let producer = ctx.file_event_producer();
        crate::tools::filesystem::tool_create_file(
            &path.to_string_lossy(),
            &input.content,
            &producer,
        )
        .map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

struct InsertLinesTool;
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
        serde_json::to_value(schemars::schema_for!(dtos::InsertLinesInput)).unwrap()
    }
    fn is_enabled(&self, _: &AppConfig, _: &str) -> bool {
        true
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::InsertLinesInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let (path, readonly) = ctx
            .resolve_virtual_path(&input.path, true)?
            .ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
        if readonly {
            return Err("Cannot perform this operation on a read-only library".to_string());
        }
        let producer = ctx.file_event_producer();
        crate::tools::filesystem::tool_insert_lines(
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

struct DeleteLinesTool;
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
        serde_json::to_value(schemars::schema_for!(dtos::DeleteLinesInput)).unwrap()
    }
    fn is_enabled(&self, _: &AppConfig, _: &str) -> bool {
        true
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::DeleteLinesInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let (path, readonly) = ctx
            .resolve_virtual_path(&input.path, true)?
            .ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
        if readonly {
            return Err("Cannot perform this operation on a read-only library".to_string());
        }
        let producer = ctx.file_event_producer();
        crate::tools::filesystem::tool_delete_lines(
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

struct WebFetchTool;
impl Tool for WebFetchTool {
    fn name(&self) -> &'static str {
        "web_fetch"
    }
    fn description(&self) -> &'static str {
        "Fetch content from a URL."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::WebFetchInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(dtos::WebFetchInput)).unwrap()
    }
    fn is_enabled(&self, _: &AppConfig, _: &str) -> bool {
        true
    }
    fn execute(&self, _ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::WebFetchInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::tools::web::tool_web_fetch(&input.url).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

struct ReadYamlHeaderTool;
impl Tool for ReadYamlHeaderTool {
    fn name(&self) -> &'static str {
        "read_yaml_header"
    }
    fn description(&self) -> &'static str {
        "Parse a YAML header from a markdown file and return its content representation. Tip: Use this to read a document's summary before reading the full file if you are not sure the full contents are needed, to protect context."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::ReadYamlHeaderInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(dtos::ReadYamlHeaderInput)).unwrap()
    }
    fn is_enabled(&self, _: &AppConfig, _: &str) -> bool {
        true
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::ReadYamlHeaderInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let (path, _) = ctx
            .resolve_virtual_path(&input.path, false)?
            .ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
        crate::tools::yaml_header::tool_read_yaml_header(&path.to_string_lossy()).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

struct WriteYamlHeaderTool;
impl Tool for WriteYamlHeaderTool {
    fn name(&self) -> &'static str {
        "write_yaml_header"
    }
    fn description(&self) -> &'static str {
        "Write or update data in a YAML header to a markdown file."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::WriteYamlHeaderInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(dtos::WriteYamlHeaderInput)).unwrap()
    }
    fn is_enabled(&self, _: &AppConfig, _: &str) -> bool {
        true
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::WriteYamlHeaderInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let (path, readonly) = ctx
            .resolve_virtual_path(&input.path, true)?
            .ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
        if readonly {
            return Err("Cannot perform this operation on a read-only library".to_string());
        }
        let producer = ctx.file_event_producer();
        crate::tools::yaml_header::tool_write_yaml_header(
            &path.to_string_lossy(),
            input.title.as_deref(),
            input.summary.as_deref(),
            input.tags,
            input.header_date.as_deref(),
            &producer,
        )
        .map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

struct WebSearchTool;
impl Tool for WebSearchTool {
    fn name(&self) -> &'static str {
        "web_search"
    }
    fn description(&self) -> &'static str {
        "Search the web using SearXNG."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::WebSearchInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(dtos::WebSearchInput)).unwrap()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        config.searxng_url.is_some()
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::WebSearchInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        if let Some(url) = &ctx.config.searxng_url {
            crate::tools::web::tool_web_search(url, &input.query).map(|r| {
                serde_json::to_value(r)
                    .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
            })
        } else {
            Err("web_search tool is disabled (no SearXNG URL configured).".to_string())
        }
    }
}

struct SearchCalendarTool;
impl Tool for SearchCalendarTool {
    fn name(&self) -> &'static str {
        "search_calendar"
    }
    fn description(&self) -> &'static str {
        "Search the calendar by keyword."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::SearchCalendarInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(dtos::SearchCalendarInput)).unwrap()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        !config.caldav_clients.is_empty()
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::SearchCalendarInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::tools::caldav::tool_search_calendar(ctx.config, &input.keyword).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

struct GetCalendarTool;
impl Tool for GetCalendarTool {
    fn name(&self) -> &'static str {
        "get_calendar"
    }
    fn description(&self) -> &'static str {
        "Get calendar items by date range."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::GetCalendarInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(dtos::GetCalendarInput)).unwrap()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        !config.caldav_clients.is_empty()
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::GetCalendarInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::tools::caldav::tool_get_calendar(ctx.config, &input.start_date, &input.end_date).map(
            |r| {
                serde_json::to_value(r)
                    .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
            },
        )
    }
}

struct GetCalendarItemTool;
impl Tool for GetCalendarItemTool {
    fn name(&self) -> &'static str {
        "get_calendar_item"
    }
    fn description(&self) -> &'static str {
        "Get a specific calendar item by its full href. IMPORTANT: Use the exact, full 'href' value returned by search or get tools (e.g., '/dav/calendars/user/.../item.ics'). Do not use just the UUID."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::GetCalendarItemInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(dtos::GetCalendarItemInput)).unwrap()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        !config.caldav_clients.is_empty()
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::GetCalendarItemInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::tools::caldav::tool_get_calendar_item(ctx.config, &input.href).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

struct AddCalendarItemTool;
impl Tool for AddCalendarItemTool {
    fn name(&self) -> &'static str {
        "add_calendar_item"
    }
    fn description(&self) -> &'static str {
        "Add a new calendar item."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::AddCalendarItemInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(dtos::AddCalendarItemInput)).unwrap()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        !config.caldav_clients.is_empty()
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::AddCalendarItemInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::tools::caldav::tool_add_calendar_item(ctx.config, &input.item_json).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

struct UpdateCalendarItemTool;
impl Tool for UpdateCalendarItemTool {
    fn name(&self) -> &'static str {
        "update_calendar_item"
    }
    fn description(&self) -> &'static str {
        "Update a calendar item."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::UpdateCalendarItemInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(dtos::UpdateCalendarItemInput)).unwrap()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        !config.caldav_clients.is_empty()
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::UpdateCalendarItemInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::tools::caldav::tool_update_calendar_item(ctx.config, &input.id, &input.update_json)
            .map(|r| {
                serde_json::to_value(r)
                    .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
            })
    }
}

struct DeleteCalendarItemTool;
impl Tool for DeleteCalendarItemTool {
    fn name(&self) -> &'static str {
        "delete_calendar_item"
    }
    fn description(&self) -> &'static str {
        "Delete a calendar item."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::DeleteCalendarItemInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(dtos::DeleteCalendarItemInput)).unwrap()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        !config.caldav_clients.is_empty()
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::DeleteCalendarItemInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::tools::caldav::tool_delete_calendar_item(ctx.config, &input.id).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

struct SearchEmailTool;
impl Tool for SearchEmailTool {
    fn name(&self) -> &'static str {
        "search_email"
    }
    fn description(&self) -> &'static str {
        "Search email by any combination of keyword, folder (mailbox), date range, sender, recipient, unread status, or flagged status. All filters are combined with AND. At least one filter must be provided. Results are paginated (default page size 10); every response includes the total number of matching emails so the caller can drive follow-up page requests."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::SearchEmailInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(dtos::SearchEmailInput)).unwrap()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        !config.jmap_clients.is_empty()
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::SearchEmailInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        let page = input.page.unwrap_or(1).max(1);
        let page_size = input.page_size.unwrap_or(10).max(1);
        crate::tools::jmap::tool_search_email(
            ctx.config,
            input.keyword.as_deref(),
            input.folder.as_deref(),
            input.start_date.as_deref(),
            input.end_date.as_deref(),
            input.from.as_deref(),
            input.to.as_deref(),
            input.is_unread,
            input.is_flagged,
            page,
            page_size,
        )
        .map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

struct GetEmailByIdTool;
impl Tool for GetEmailByIdTool {
    fn name(&self) -> &'static str {
        "get_email_by_id"
    }
    fn description(&self) -> &'static str {
        "Get email by id."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::GetEmailByIdInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(dtos::GetEmailByIdInput)).unwrap()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        !config.jmap_clients.is_empty()
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::GetEmailByIdInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::tools::jmap::tool_get_email_by_id(ctx.config, &input.id).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

struct SendEmailTool;
impl Tool for SendEmailTool {
    fn name(&self) -> &'static str {
        "send_email"
    }
    fn description(&self) -> &'static str {
        "Send an email."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::SendEmailInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(dtos::SendEmailInput)).unwrap()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        !config.jmap_clients.is_empty()
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::SendEmailInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::tools::jmap::tool_send_email(ctx.config, &input.to, &input.subject, &input.body).map(
            |r| {
                serde_json::to_value(r)
                    .unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
            },
        )
    }
}

struct SearchContactTool;
impl Tool for SearchContactTool {
    fn name(&self) -> &'static str {
        "search_contact"
    }
    fn description(&self) -> &'static str {
        "Search contacts by keyword."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::SearchContactInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(dtos::SearchContactInput)).unwrap()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        if config
            .feature_flags
            .get("useDAVForContacts")
            .copied()
            .unwrap_or(false)
        {
            !config.caldav_clients.is_empty()
        } else {
            !config.jmap_clients.is_empty()
        }
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::SearchContactInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        if ctx
            .config
            .feature_flags
            .get("useDAVForContacts")
            .copied()
            .unwrap_or(false)
        {
            crate::tools::carddav::tool_search_contact(ctx.config, &input.keyword)
        } else {
            crate::tools::jmap::tool_search_contact(ctx.config, &input.keyword)
        }
        .map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

struct AddContactTool;
impl Tool for AddContactTool {
    fn name(&self) -> &'static str {
        "add_contact"
    }
    fn description(&self) -> &'static str {
        "Add a new contact."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::AddContactInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(dtos::AddContactInput)).unwrap()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        if config
            .feature_flags
            .get("useDAVForContacts")
            .copied()
            .unwrap_or(false)
        {
            !config.caldav_clients.is_empty()
        } else {
            !config.jmap_clients.is_empty()
        }
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::AddContactInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        if ctx
            .config
            .feature_flags
            .get("useDAVForContacts")
            .copied()
            .unwrap_or(false)
        {
            crate::tools::carddav::tool_add_contact(ctx.config, &input.contact_json)
        } else {
            crate::tools::jmap::tool_add_contact(ctx.config, &input.contact_json)
        }
        .map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

struct GetContactTool;
impl Tool for GetContactTool {
    fn name(&self) -> &'static str {
        "get_contact"
    }
    fn description(&self) -> &'static str {
        "Get contact by id."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<dtos::GetContactInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(dtos::GetContactInput)).unwrap()
    }
    fn is_enabled(&self, config: &AppConfig, _: &str) -> bool {
        if config
            .feature_flags
            .get("useDAVForContacts")
            .copied()
            .unwrap_or(false)
        {
            !config.caldav_clients.is_empty()
        } else {
            !config.jmap_clients.is_empty()
        }
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: dtos::GetContactInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        if ctx
            .config
            .feature_flags
            .get("useDAVForContacts")
            .copied()
            .unwrap_or(false)
        {
            crate::tools::carddav::tool_get_contact(ctx.config, &input.id)
        } else {
            crate::tools::jmap::tool_get_contact(ctx.config, &input.id)
        }
        .map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

// --- CSV Tools ---

fn csv_tools_enabled(prompt: &str) -> bool {
    let p = prompt.to_lowercase();
    p.contains("table")
        || p.contains("csv")
        || p.contains("database")
        || p.contains("add_rows")
        || p.contains("delete_rows")
        || p.contains("create_csv")
        || p.contains("list_csv")
        || p.contains("query")
}

struct CsvCreateTool;
impl Tool for CsvCreateTool {
    fn name(&self) -> &'static str {
        "create_csv"
    }
    fn description(&self) -> &'static str {
        "Create a new CSV file database with specified headers."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<crate::tools::csv_db::schema::CreateCsvInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(
            crate::tools::csv_db::schema::CreateCsvInput
        ))
        .unwrap()
    }
    fn is_enabled(&self, _: &AppConfig, prompt: &str) -> bool {
        csv_tools_enabled(prompt)
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: crate::tools::csv_db::schema::CreateCsvInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::tools::csv_db::operations::create_csv(ctx.config, input).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

struct CsvListTool;
impl Tool for CsvListTool {
    fn name(&self) -> &'static str {
        "list_csv"
    }
    fn description(&self) -> &'static str {
        "List all CSV file databases."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<crate::tools::csv_db::schema::ListCsvInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(
            crate::tools::csv_db::schema::ListCsvInput
        ))
        .unwrap()
    }
    fn is_enabled(&self, _: &AppConfig, prompt: &str) -> bool {
        csv_tools_enabled(prompt)
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: crate::tools::csv_db::schema::ListCsvInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::tools::csv_db::operations::list_csv(ctx.config, input).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

struct CsvAddRowsTool;
impl Tool for CsvAddRowsTool {
    fn name(&self) -> &'static str {
        "add_rows"
    }
    fn description(&self) -> &'static str {
        "Add rows to a CSV file database."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<crate::tools::csv_db::schema::AddRowsInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(
            crate::tools::csv_db::schema::AddRowsInput
        ))
        .unwrap()
    }
    fn is_enabled(&self, _: &AppConfig, prompt: &str) -> bool {
        csv_tools_enabled(prompt)
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: crate::tools::csv_db::schema::AddRowsInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::tools::csv_db::operations::add_rows(ctx.config, input).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

struct CsvDeleteRowsTool;
impl Tool for CsvDeleteRowsTool {
    fn name(&self) -> &'static str {
        "delete_rows"
    }
    fn description(&self) -> &'static str {
        "Delete rows from a CSV file database based on a predicate."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<crate::tools::csv_db::schema::DeleteRowsInput>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(
            crate::tools::csv_db::schema::DeleteRowsInput
        ))
        .unwrap()
    }
    fn is_enabled(&self, _: &AppConfig, prompt: &str) -> bool {
        csv_tools_enabled(prompt)
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: crate::tools::csv_db::schema::DeleteRowsInput =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::tools::csv_db::query::delete_rows(ctx.config, input).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

struct CsvQueryTool;
impl Tool for CsvQueryTool {
    fn name(&self) -> &'static str {
        "query"
    }
    fn description(&self) -> &'static str {
        "Query a CSV file database using an evalexpr predicate, supporting sum and average aggregates."
    }
    fn input_type(&self) -> TypeId {
        TypeId::of::<crate::tools::csv_db::schema::QueryRequest>()
    }
    fn parameters_schema(&self) -> serde_json::Value {
        serde_json::to_value(schemars::schema_for!(
            crate::tools::csv_db::schema::QueryRequest
        ))
        .unwrap()
    }
    fn is_enabled(&self, _: &AppConfig, prompt: &str) -> bool {
        csv_tools_enabled(prompt)
    }
    fn execute(&self, ctx: &ToolContext, args: &str) -> Result<serde_json::Value, String> {
        let input: crate::tools::csv_db::schema::QueryRequest =
            serde_json::from_str(args).map_err(|e| format!("Invalid args: {}", e))?;
        crate::tools::csv_db::query::query_csv(ctx.config, input).map(|r| {
            serde_json::to_value(r).unwrap_or_else(|e| serde_json::json!({"error": e.to_string()}))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_events::Bus;

    fn test_bus() -> &'static Bus<crate::file_events::FileEvent> {
        Box::leak(Box::new(Bus::new()))
    }

    fn test_ctx(config: &AppConfig) -> ToolContext<'static> {
        ToolContext::new(unsafe { &*(config as *const AppConfig) }, test_bus())
    }

    #[test]
    fn test_resolve_virtual_path() {
        let mut config = AppConfig::default();
        config
            .content_libraries
            .push(crate::config::ContentLibrary {
                name: "TestLib".to_string(),
                root_folder: "C:\\TestRoot".to_string(),
                kind: "text".to_string(),
                readonly: false,
                priority: 0,
            });
        let ctx = test_ctx(&config);

        let res1 = execute_tool(&ctx, "read_file", r#"{"path": "TestLib\\sub\\file.md"}"#);
        assert!(!res1.contains("Invalid virtual path"));

        let res3 = execute_tool(
            &ctx,
            "read_file",
            r#"{"path": "TestLib\\..\\Windows\\System32\\cmd.exe"}"#,
        );
        assert!(res3.contains("path traversal"));

        let res4 = execute_tool(&ctx, "read_file", r#"{"path": "UnknownLib\\file.md"}"#);
        assert!(res4.contains("Content library 'UnknownLib' not found"));

        let res5 = execute_tool(&ctx, "list_files", r#"{"path": "."}"#);
        assert!(!res5.contains("Invalid virtual path") && !res5.contains("error"));
        assert!(res5.contains("TestLib"));

        let res6 = execute_tool(&ctx, "list_files", r#"{"path": "/"}"#);
        assert!(!res6.contains("Invalid virtual path") && !res6.contains("error"));
        assert!(res6.contains("TestLib"));
    }

    #[test]
    fn test_grep_priority_ordering() {
        let mut config = AppConfig::default();
        config
            .content_libraries
            .push(crate::config::ContentLibrary {
                name: "Low".to_string(),
                root_folder: "C:\\LowRoot".to_string(),
                kind: "text".to_string(),
                readonly: false,
                priority: 0,
            });
        config
            .content_libraries
            .push(crate::config::ContentLibrary {
                name: "High".to_string(),
                root_folder: "C:\\HighRoot".to_string(),
                kind: "text".to_string(),
                readonly: false,
                priority: 100,
            });
        let mut libs: Vec<_> = config.content_libraries.iter().collect();
        libs.sort_by(|a, b| b.priority.cmp(&a.priority));
        assert_eq!(libs[0].name, "High");
        assert_eq!(libs[1].name, "Low");
    }

    #[test]
    fn test_path_traversal_dotdot_rejected() {
        let mut config = AppConfig::default();
        config
            .content_libraries
            .push(crate::config::ContentLibrary {
                name: "Lib".to_string(),
                root_folder: "C:\\Root".to_string(),
                kind: "text".to_string(),
                readonly: false,
                priority: 0,
            });
        let ctx = test_ctx(&config);

        let res = execute_tool(&ctx, "read_file", r#"{"path": "Lib/../../etc/passwd"}"#);
        assert!(res.contains("path traversal"));

        let res2 = execute_tool(&ctx, "read_file", r#"{"path": "Lib/.."}"#);
        assert!(res2.contains("path traversal"));
    }

    #[test]
    fn test_resolve_path_with_library_missing() {
        let config = AppConfig::default();
        let ctx = test_ctx(&config);
        let res = execute_tool(&ctx, "list_files", r#"{"path": "NonExistentLib/file.md"}"#);
        assert!(res.contains("Content library 'NonExistentLib' not found"));
    }

    #[test]
    fn test_unknown_tool_returns_error() {
        let config = AppConfig::default();
        let ctx = test_ctx(&config);
        let res = execute_tool(&ctx, "nonexistent_tool", "{}");
        assert!(res.contains("Tool nonexistent_tool not found"));
    }

    #[test]
    fn test_tool_invalid_args_returns_error() {
        let config = AppConfig::default();
        let ctx = test_ctx(&config);
        let res = execute_tool(&ctx, "list_files", "not valid json");
        assert!(res.contains("Invalid args") || res.contains("error"));
    }

    #[test]
    fn test_tool_call_debug_mode_feature_flag() {
        let mut config = AppConfig::default();
        assert_eq!(
            config
                .feature_flags
                .get("toolCallDebugMode")
                .copied()
                .unwrap_or(false),
            false
        );
        config
            .feature_flags
            .insert("toolCallDebugMode".to_string(), true);
        assert_eq!(
            config
                .feature_flags
                .get("toolCallDebugMode")
                .copied()
                .unwrap_or(false),
            true
        );
        let ctx = test_ctx(&config);
        let res = execute_tool(&ctx, "unknown_tool", "{}");
        assert!(res.contains("not found") || res.contains("error"));
    }

    use serde_json::Value;
    use std::fs;
    use tempfile::TempDir;

    struct LibFixture {
        _a: TempDir,
        _b: Option<TempDir>,
    }

    fn single_lib_with_n_tagged_files(n: usize) -> (AppConfig, LibFixture) {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..n {
            let name = format!("file_{:03}.md", i);
            let body = format!("---\ntags: [meeting]\n---\n# Doc {}\n", i);
            fs::write(dir.path().join(name), body).unwrap();
        }
        let mut config = AppConfig::default();
        config
            .content_libraries
            .push(crate::config::ContentLibrary {
                name: "Lib".to_string(),
                root_folder: dir.path().to_string_lossy().into_owned(),
                kind: "text".to_string(),
                readonly: false,
                priority: 0,
            });
        (config, LibFixture { _a: dir, _b: None })
    }

    fn two_libs_with_n_tagged_files_each(n: usize) -> (AppConfig, LibFixture) {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        for i in 0..n {
            let body = format!("---\ntags: [meeting]\n---\n# Doc {}\n", i);
            fs::write(a.path().join(format!("a_{:03}.md", i)), &body).unwrap();
            fs::write(b.path().join(format!("b_{:03}.md", i)), &body).unwrap();
        }
        let mut config = AppConfig::default();
        config
            .content_libraries
            .push(crate::config::ContentLibrary {
                name: "LibA".to_string(),
                root_folder: a.path().to_string_lossy().into_owned(),
                kind: "text".to_string(),
                readonly: false,
                priority: 0,
            });
        config
            .content_libraries
            .push(crate::config::ContentLibrary {
                name: "LibB".to_string(),
                root_folder: b.path().to_string_lossy().into_owned(),
                kind: "text".to_string(),
                readonly: false,
                priority: 0,
            });
        (config, LibFixture { _a: a, _b: Some(b) })
    }

    fn run_list_by_tag(config: &AppConfig, args: &str) -> Value {
        let ctx = test_ctx(config);
        let raw = execute_tool(&ctx, "list_files_by_tag", args);
        serde_json::from_str(&raw)
            .unwrap_or_else(|e| panic!("could not parse tool response `{}`: {}", raw, e))
    }

    fn files_array(data: &Value) -> Vec<String> {
        data["files"]
            .as_array()
            .unwrap_or_else(|| panic!("files is not a JSON array: {data}"))
            .iter()
            .map(|v| {
                v.as_str()
                    .unwrap_or_else(|| panic!("non-string element in files array: {v}"))
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn test_list_by_tag_default_page_size_is_20() {
        let (config, _dir) = single_lib_with_n_tagged_files(5);
        let envelope = run_list_by_tag(&config, r#"{"tag":"meeting"}"#);
        assert_eq!(envelope["status"], "success");
        let data = &envelope["data"];
        assert_eq!(data["total"], 5);
        let files = files_array(data);
        assert_eq!(files.len(), 5);
        assert!(files.iter().any(|p| p.ends_with("file_000.md")));
        assert!(files.iter().any(|p| p.ends_with("file_004.md")));
        assert!(data.get("hint").is_none() || data["hint"].is_null());
    }

    #[test]
    fn test_list_by_tag_pagination_first_page() {
        let (config, _dir) = single_lib_with_n_tagged_files(50);
        let envelope = run_list_by_tag(&config, r#"{"tag":"meeting","page":1,"page_size":20}"#);
        let data = &envelope["data"];
        assert_eq!(data["total"], 50);
        let files = files_array(data);
        assert_eq!(files.len(), 20);
        assert!(files[0].ends_with("file_000.md"));
        assert!(files[19].ends_with("file_019.md"));
    }

    #[test]
    fn test_list_by_tag_pagination_second_page() {
        let (config, _dir) = single_lib_with_n_tagged_files(50);
        let envelope = run_list_by_tag(&config, r#"{"tag":"meeting","page":2,"page_size":20}"#);
        let data = &envelope["data"];
        assert_eq!(data["total"], 50);
        let files = files_array(data);
        assert_eq!(files.len(), 20);
        assert!(files[0].ends_with("file_020.md"));
        assert!(files[19].ends_with("file_039.md"));
    }

    #[test]
    fn test_list_by_tag_pagination_last_partial_page() {
        let (config, _dir) = single_lib_with_n_tagged_files(50);
        let envelope = run_list_by_tag(&config, r#"{"tag":"meeting","page":3,"page_size":20}"#);
        let data = &envelope["data"];
        assert_eq!(data["total"], 50);
        let files = files_array(data);
        assert_eq!(files.len(), 10);
        assert!(files[0].ends_with("file_040.md"));
        assert!(files[9].ends_with("file_049.md"));
    }

    #[test]
    fn test_list_by_tag_page_past_end_returns_hint() {
        let (config, _dir) = single_lib_with_n_tagged_files(5);
        let envelope = run_list_by_tag(&config, r#"{"tag":"meeting","page":99,"page_size":20}"#);
        let data = &envelope["data"];
        assert_eq!(data["total"], 5);
        assert!(files_array(data).is_empty());
        let hint = data["hint"]
            .as_str()
            .expect("hint should be set on past-end");
        assert!(hint.starts_with("No tagged files on page 99"));
        assert!(hint.contains("5 total"));
    }

    #[test]
    fn test_list_by_tag_page_size_one() {
        let (config, _dir) = single_lib_with_n_tagged_files(3);
        let envelope = run_list_by_tag(&config, r#"{"tag":"meeting","page":2,"page_size":1}"#);
        let data = &envelope["data"];
        assert_eq!(data["total"], 3);
        let files = files_array(data);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("file_001.md"));
    }

    #[test]
    fn test_list_by_tag_pagination_is_global_across_libraries() {
        let (config, _fixture) = two_libs_with_n_tagged_files_each(25);
        let envelope = run_list_by_tag(&config, r#"{"tag":"meeting","page":1,"page_size":20}"#);
        let data = &envelope["data"];
        assert_eq!(data["total"], 50);
        assert_eq!(files_array(data).len(), 20);
    }

    #[test]
    fn test_list_by_tag_no_matches_reports_zero_total() {
        let _empty = tempfile::tempdir().unwrap();
        let mut config = AppConfig::default();
        config
            .content_libraries
            .push(crate::config::ContentLibrary {
                name: "Lib".to_string(),
                root_folder: _empty.path().to_string_lossy().into_owned(),
                kind: "text".to_string(),
                readonly: false,
                priority: 0,
            });
        let envelope = run_list_by_tag(&config, r#"{"tag":"meeting"}"#);
        let data = &envelope["data"];
        assert_eq!(data["total"], 0);
        assert!(files_array(data).is_empty());
        let hint = data["hint"]
            .as_str()
            .expect("hint should be set on no-match");
        assert_eq!(hint, "No matching tagged files found.");
    }

    #[test]
    fn test_list_by_tag_page_zero_is_normalised_to_page_one() {
        let (config, _dir) = single_lib_with_n_tagged_files(5);
        let envelope = run_list_by_tag(&config, r#"{"tag":"meeting","page":0,"page_size":3}"#);
        let data = &envelope["data"];
        assert_eq!(data["total"], 5);
        let files = files_array(data);
        assert_eq!(files.len(), 3);
        assert!(files.iter().any(|p| p.ends_with("file_000.md")));
        assert!(files.iter().any(|p| p.ends_with("file_002.md")));
    }

    fn run_list_files(config: &AppConfig, args: &str) -> Value {
        let ctx = test_ctx(config);
        let raw = execute_tool(&ctx, "list_files", args);
        serde_json::from_str(&raw)
            .unwrap_or_else(|e| panic!("could not parse tool response `{}`: {}", raw, e))
    }

    fn single_lib_with_n_md_files(n: usize) -> (AppConfig, LibFixture) {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..n {
            let name = format!("note_{:03}.md", i);
            fs::write(dir.path().join(name), "# Just a doc").unwrap();
        }
        let mut config = AppConfig::default();
        config
            .content_libraries
            .push(crate::config::ContentLibrary {
                name: "Lib".to_string(),
                root_folder: dir.path().to_string_lossy().into_owned(),
                kind: "text".to_string(),
                readonly: false,
                priority: 0,
            });
        (config, LibFixture { _a: dir, _b: None })
    }

    #[test]
    fn test_list_files_default_page_size_is_20() {
        let (config, _fix) = single_lib_with_n_md_files(5);
        let envelope = run_list_files(&config, r#"{"path":"Lib"}"#);
        let data = &envelope["data"];
        assert_eq!(data["total"], 5);
        let files = files_array(data);
        assert_eq!(files.len(), 5);
        assert!(files
            .iter()
            .all(|p| p.starts_with("Lib") && p.contains("note_")));
    }

    #[test]
    fn test_list_files_pagination_first_page() {
        let (config, _fix) = single_lib_with_n_md_files(50);
        let envelope = run_list_files(&config, r#"{"path":"Lib","page":1,"page_size":20}"#);
        let data = &envelope["data"];
        assert_eq!(data["total"], 50);
        let files = files_array(data);
        assert_eq!(files.len(), 20);
        assert!(files[0].ends_with("note_000.md"));
        assert!(files[19].ends_with("note_019.md"));
    }

    #[test]
    fn test_list_files_pagination_last_partial_page() {
        let (config, _fix) = single_lib_with_n_md_files(50);
        let envelope = run_list_files(&config, r#"{"path":"Lib","page":3,"page_size":20}"#);
        let data = &envelope["data"];
        assert_eq!(data["total"], 50);
        let files = files_array(data);
        assert_eq!(files.len(), 10);
        assert!(files[0].ends_with("note_040.md"));
        assert!(files[9].ends_with("note_049.md"));
    }

    #[test]
    fn test_list_files_page_past_end_returns_hint() {
        let (config, _fix) = single_lib_with_n_md_files(5);
        let envelope = run_list_files(&config, r#"{"path":"Lib","page":99,"page_size":20}"#);
        let data = &envelope["data"];
        assert_eq!(data["total"], 5);
        assert!(files_array(data).is_empty());
        let hint = data["hint"]
            .as_str()
            .expect("hint should be set on past-end");
        assert!(hint.contains("page 99"));
        assert!(hint.contains("5 total"));
    }

    #[test]
    fn test_list_files_root_path_returns_libraries() {
        let (config, _fix) = single_lib_with_n_md_files(0);
        let envelope = run_list_files(&config, r#"{"path":"/"}"#);
        let data = &envelope["data"];
        assert_eq!(data["total"], 1);
        let files = files_array(data);
        assert_eq!(files, vec!["Lib".to_string()]);
    }

    #[test]
    fn test_list_files_multiple_libraries_paginated_globally() {
        let dir_a = tempfile::tempdir().unwrap();
        let dir_b = tempfile::tempdir().unwrap();
        for i in 0..30 {
            fs::write(dir_a.path().join(format!("a_{:03}.md", i)), "x").unwrap();
            fs::write(dir_b.path().join(format!("b_{:03}.md", i)), "x").unwrap();
        }
        let mut config = AppConfig::default();
        config
            .content_libraries
            .push(crate::config::ContentLibrary {
                name: "LibA".to_string(),
                root_folder: dir_a.path().to_string_lossy().into_owned(),
                kind: "text".to_string(),
                readonly: false,
                priority: 0,
            });
        config
            .content_libraries
            .push(crate::config::ContentLibrary {
                name: "LibB".to_string(),
                root_folder: dir_b.path().to_string_lossy().into_owned(),
                kind: "text".to_string(),
                readonly: false,
                priority: 0,
            });
        let _fix = LibFixture {
            _a: dir_a,
            _b: Some(dir_b),
        };
        let envelope = run_list_files(&config, r#"{"path":"LibA","page":1,"page_size":20}"#);
        let data = &envelope["data"];
        assert_eq!(data["total"], 30);
        assert_eq!(files_array(data).len(), 20);
    }

    #[test]
    fn test_list_files_returns_json_array_not_string() {
        let (config, _fix) = single_lib_with_n_md_files(3);
        let ctx = test_ctx(&config);
        let raw = execute_tool(&ctx, "list_files", r#"{"path":"Lib"}"#);
        let parsed: Value = serde_json::from_str(&raw).unwrap();
        assert!(parsed["data"]["files"].is_array());
    }

    #[test]
    fn test_csv_tools_in_schema() {
        let config = AppConfig::default();
        let schema = get_tools_schema(&config, "create a csv database");
        let tools = schema.as_array().unwrap();
        let names: Vec<&str> = tools
            .iter()
            .filter_map(|t| t["function"]["name"].as_str())
            .collect();
        assert!(names.contains(&"create_csv"));
        assert!(names.contains(&"list_csv"));
        assert!(names.contains(&"add_rows"));
        assert!(names.contains(&"delete_rows"));
        assert!(names.contains(&"query"));
    }

    #[test]
    fn test_csv_tools_excluded() {
        let config = AppConfig::default();
        let schema = get_tools_schema(&config, "just a normal message");
        let tools = schema.as_array().unwrap();
        let names: Vec<&str> = tools
            .iter()
            .filter_map(|t| t["function"]["name"].as_str())
            .collect();
        assert!(!names.contains(&"create_csv"));
        assert!(!names.contains(&"list_csv"));
    }
}
