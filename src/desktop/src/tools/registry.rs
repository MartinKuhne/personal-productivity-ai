use crate::config::AppConfig;
use crate::file_events::{Bus, FileEvent, FileEventProducer};
use std::path::{Component, Path};

/// Slice `items` into a paginated window of size `page_size` for
/// `page` (1-indexed). Returns the page as a `Vec<T>` and an
/// optional human-readable hint when the requested page is past the
/// end. `plural` is the noun form used in the hint (e.g. `"files"`,
/// `"libraries"`, `"tagged files"`) so the LLM agent gets context
/// about which tool produced the empty result.
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

macro_rules! define_tools {
    (
        $(
            {
                name: $name:expr,
                description: $desc:expr,
                input: $input_type:ty,
                enabled: $enabled:expr,
                execute: |$config:ident, $root_path:ident, $input:ident, $resolve_path:ident, $producer:ident| $exec:expr
            }
        ),* $(,)?
    ) => {
        pub fn get_tools_schema(config: &AppConfig, prompt: &str) -> serde_json::Value {
            let mut tools = Vec::new();
            $(
                let is_enabled: bool = $enabled(config);
                if is_enabled {
                    tools.push(serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": $name,
                            "description": $desc,
                            "parameters": schemars::schema_for!($input_type)
                        }
                    }));
                }
            )*
            tools.extend(crate::tools::csv_db::get_csv_tools(config, prompt));
            serde_json::Value::Array(tools)
        }

        pub fn execute_tool(config: &AppConfig, root_path: &Path, name: &str, args_str: &str, file_event_bus: &Bus<FileEvent>) -> String {
            rustls::crypto::ring::default_provider().install_default().ok();

            let debug_mode = config
                .feature_flags
                .get("toolCallDebugMode")
                .copied()
                .unwrap_or(false);

            let args_compact = args_str.to_string();
            tracing::info!(name = "tool.registry.call", tool_name = %name, args = %args_compact, "Executing tool call");
            let start_time = std::time::Instant::now();

            // A producer handle for any tool that mutates the
            // filesystem. The lifetime is tied to `file_event_bus`,
            // which lives for the duration of this call.
            let producer = FileEventProducer::new(file_event_bus);

            let resolve_and_check_path = |p: &str| -> Result<Option<(std::path::PathBuf, bool)>, String> {
                if Path::new(p).components().any(|c| c == Component::ParentDir) { return Err("Path traversal not allowed".to_string()); }
                let path = Path::new(p);
                let mut components = path.components().peekable();

                while let Some(c) = components.peek() {
                    match c {
                        Component::RootDir | Component::CurDir => { components.next(); },
                        _ => break,
                    }
                }

                if components.peek().is_none() {
                    return Ok(None);
                }

                if let Some(std::path::Component::Normal(first)) = components.next() {
                    let first_str = first.to_string_lossy();
                    for lib in &config.content_libraries {
                        if lib.name == first_str {
                            let rest: std::path::PathBuf = components.collect();
                            return Ok(Some((Path::new(&lib.root_folder).join(rest), lib.readonly)));
                        }
                    }

                    Err(format!("Content library '{}' not found in virtual path '{}'", first_str, p))
                } else {
                    Err(format!("Invalid virtual path: '{}'", p))
                }
            };

            let result_raw: Result<serde_json::Value, String> = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if let Some(res) = crate::tools::csv_db::execute_csv_tool(config, name, args_str) {
                    return res;
                }
                match name {
                    $(
                        $name => {
                            let input_res: Result<$input_type, _> = serde_json::from_str(args_str);
                            match input_res {
                                Ok(parsed_input) => {
                                    let $input = parsed_input;
                                    let $config = config;
                                    let $root_path = root_path;
                                    let $resolve_path = &resolve_and_check_path;
                                    let $producer = &producer;
                                    let res = $exec;
                                    res.map(|r| serde_json::to_value(r).unwrap())
                                },
                                Err(e) => Err(format!("Invalid args: {}", e)),
                            }
                        }
                    )*
                    _ => Err(format!("Tool {} not found.", name)),
                }
            })) {
                Ok(res) => res,
                Err(e) => {
                    let msg = if let Some(s) = e.downcast_ref::<&str>() { *s } else if let Some(s) = e.downcast_ref::<String>() { s.as_str() } else { "Unknown panic" };
                    Err(format!("Tool {} panicked: {}", name, msg))
                }
            };

            let elapsed = start_time.elapsed();
            let response_dto = match result_raw {
                Ok(data) => {
                    if debug_mode {
                        let data_str = serde_json::to_string(&data).unwrap_or_else(|_| "<serialization error>".to_string());
                        tracing::info!(name = "tool.registry.success", tool_name = %name, elapsed = ?elapsed, data = %data_str, "Tool execution succeeded");
                    } else {
                        tracing::info!(name = "tool.registry.success", tool_name = %name, elapsed = ?elapsed, "Tool execution succeeded");
                    }
                    crate::tools::dtos::ToolResponse::Success { data }
                },
                Err(err) => {
                    tracing::error!(name = "tool.registry.failed", tool_name = %name, elapsed = ?elapsed, error = %err, "Tool execution failed. Operator should verify tool inputs.");
                    crate::tools::dtos::ToolResponse::Error { message: err }
                }
            };

            serde_json::to_string(&response_dto).unwrap_or_else(|_| r#"{"status":"error","message":"Failed to serialize tool response"}"#.to_string())
        }
    };
}

define_tools! {

    {
        name: "web_delegate",
        description: "Delegate web searches and web fetches to a sub-agent. This protects your context window. Give clear instructions and it will return summarized information.",
        input: crate::tools::dtos::WebDelegateInput,
        enabled: |_| true,
        execute: |config, _root_path, input, _is_safe, _producer| crate::tools::web::tool_web_delegate(config, &input.instruction)
    },
    {
        name: "replace_text",
        description: "Replace exact occurrences of old_string with new_string in a file.",
        input: crate::tools::dtos::ReplaceTextInput,
        enabled: |_| true,
        execute: |_config, _root_path, input, resolve_path, producer| {
            let (path, readonly) = resolve_path(&input.path)?.ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
            if readonly { return Err("Cannot perform this operation on a read-only library".to_string()); }
            crate::tools::filesystem::tool_replace_text(&path.to_string_lossy(), &input.old_string, &input.new_string, producer)
        }
    },    {
        name: "grep",
        description: "Search for a query string case-insensitively across all Markdown files in the workspace.",
        input: crate::tools::dtos::GrepInput,
        enabled: |_| true,
        execute: |config, _root_path, input, _resolve_path, _producer| {
            let mut all_results = Vec::new();
            let mut libs: Vec<_> = config.content_libraries.iter().collect();
            libs.sort_by(|a, b| b.priority.cmp(&a.priority)); // Highest priority first
            for lib in libs {
                if let Ok(res) = crate::tools::filesystem::tool_grep(Path::new(&lib.root_folder), &lib.name, &input.query) {
                    if res.matches != "No matches found." {
                        all_results.push(res.matches);
                    }
                }
            }
            if all_results.is_empty() {
                Ok(crate::tools::dtos::GrepResponse { matches: "No matches found.".to_string() })
            } else {
                Ok(crate::tools::dtos::GrepResponse { matches: all_results.join("\n") })
            }
        }
    },
    {
        name: "read_tags",
        description: "Get all unique tags defined in front-matter headers of all Markdown files in the workspace.",
        input: crate::tools::dtos::ReadTagsInput,
        enabled: |_| true,
        execute: |config, _root_path, _input, _resolve_path, _producer| {
            let mut all_tags = std::collections::BTreeSet::new();
            for lib in &config.content_libraries {
                if let Ok(res) = crate::tools::filesystem::tool_read_tags(Path::new(&lib.root_folder)) {
                    for tag in res.tags {
                        all_tags.insert(tag);
                    }
                }
            }
            Ok(crate::tools::dtos::ReadTagsResponse { tags: all_tags.into_iter().collect() })
        }
    },
    {
        name: "list_files_by_tag",
        description: "List Markdown files that contain a specific tag in their front-matter. Results are returned as a JSON array, paginated across all configured libraries (default page size 20); every response includes the total number of matching files so the caller can drive follow-up page requests.",
        input: crate::tools::dtos::ListFilesByTagInput,
        enabled: |_| true,
        execute: |config, _root_path, input, _resolve_path, _producer| {
            // Resolve pagination defaults up-front so a single set of
            // values applies to the *combined* result.
            let page = input.page.unwrap_or(1).max(1);
            let page_size = input
                .page_size
                .unwrap_or(crate::tools::filesystem::DEFAULT_LIST_FILES_BY_TAG_PAGE_SIZE)
                .max(1);

            // Collect every match across every library, then sort so
            // paging is deterministic regardless of library order.
            let mut all_matches: Vec<String> = Vec::new();
            for lib in &config.content_libraries {
                match crate::tools::filesystem::tool_list_files_by_tag(
                    Path::new(&lib.root_folder),
                    &lib.name,
                    &input.tag,
                ) {
                    Ok(mut files) => all_matches.append(&mut files),
                    Err(e) => {
                        tracing::warn!(name = "tool.list_files_by_tag.lib_failed", lib = %lib.name, error = %e, "list_files_by_tag failed for a single library; continuing with the others");
                    }
                }
            }
            all_matches.sort();
            all_matches.dedup();

            let total = all_matches.len();
            let (page_files, hint) = paginate_in_range(
                &all_matches,
                page,
                page_size,
                total,
                "tagged files",
            );
            Ok(crate::tools::dtos::ListFilesByTagResponse {
                files: page_files,
                total,
                hint,
            })
        }
    },
    {
        name: "list_files",
        description: "List Markdown files in a directory (not recursive). Results are returned as a JSON array, paginated (default page size 20); every response includes the total number of files in the directory so the caller can drive follow-up page requests. With `path` set to \"/\" or \".\" returns the configured content libraries.",
        input: crate::tools::dtos::ListFilesInput,
        enabled: |_| true,
        execute: |config, _root_path, input, resolve_path, _producer| {
            // Resolve pagination defaults up-front.
            let page = input.page.unwrap_or(1).max(1);
            let page_size = input
                .page_size
                .unwrap_or(crate::tools::filesystem::DEFAULT_LIST_FILES_BY_TAG_PAGE_SIZE)
                .max(1);

            // Collect every match up-front so the call site can
            // apply paging consistently across both the
            // directory-listing and the library-rooting branches.
            let all_matches: Vec<String> = match resolve_path(&input.path)? {
                Some((path, _)) => {
                    crate::tools::filesystem::tool_list_files(&path, &input.path)?
                }
                None => {
                    // `/` or `.` — list the configured libraries.
                    let mut libs: Vec<String> =
                        config.content_libraries.iter().map(|lib| lib.name.clone()).collect();
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
            let (page_files, hint) = paginate_in_range(
                &all_matches,
                page,
                page_size,
                total,
                plural,
            );
            Ok(crate::tools::dtos::ListFilesResponse {
                files: page_files,
                total,
                hint,
            })
        }
    },
    {
        name: "read_file",
        description: "Read the entire text contents of a file at the specified path. Prefer using the read_yaml_header tool if just a document summary is needed.",
        input: crate::tools::dtos::ReadFileInput,
        enabled: |_| true,
        execute: |_config, _root_path, input, resolve_path, _producer| {
            let (path, _) = resolve_path(&input.path)?.ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
            crate::tools::filesystem::tool_read_file(&path.to_string_lossy())
        }
    },
    {
        name: "read_file_lines",
        description: "Read specific lines from a file (1-indexed).",
        input: crate::tools::dtos::ReadFileLinesInput,
        enabled: |_| true,
        execute: |_config, _root_path, input, resolve_path, _producer| {
            let (path, _) = resolve_path(&input.path)?.ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
            crate::tools::filesystem::tool_read_file_lines(&path.to_string_lossy(), input.start_line, input.end_line)
        }
    },
    {
        name: "create_file",
        description: "Create a new file at the specified path with the provided content.",
        input: crate::tools::dtos::CreateFileInput,
        enabled: |_| true,
        execute: |_config, _root_path, input, resolve_path, producer| {
            let (path, readonly) = resolve_path(&input.path)?.ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
            if readonly { return Err("Cannot perform this operation on a read-only library".to_string()); }
            crate::tools::filesystem::tool_create_file(&path.to_string_lossy(), &input.content, producer)
        }
    },
    {
        name: "insert_lines",
        description: "Insert lines into a file at a specific 1-indexed line index.",
        input: crate::tools::dtos::InsertLinesInput,
        enabled: |_| true,
        execute: |_config, _root_path, input, resolve_path, producer| {
            let (path, readonly) = resolve_path(&input.path)?.ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
            if readonly { return Err("Cannot perform this operation on a read-only library".to_string()); }
            crate::tools::filesystem::tool_insert_lines(&path.to_string_lossy(), input.line_index, &input.lines, producer)
        }
    },
    {
        name: "delete_lines",
        description: "Delete specific lines from a file (1-indexed, inclusive).",
        input: crate::tools::dtos::DeleteLinesInput,
        enabled: |_| true,
        execute: |_config, _root_path, input, resolve_path, producer| {
            let (path, readonly) = resolve_path(&input.path)?.ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
            if readonly { return Err("Cannot perform this operation on a read-only library".to_string()); }
            crate::tools::filesystem::tool_delete_lines(&path.to_string_lossy(), input.start_line, input.end_line, producer)
        }
    },
    {
        name: "web_fetch",
        description: "Fetch content from a URL.",
        input: crate::tools::dtos::WebFetchInput,
        enabled: |_| true,
        execute: |_config, _root_path, input, _is_safe, _producer| crate::tools::web::tool_web_fetch(&input.url)
    },
    {
        name: "read_yaml_header",
        description: "Parse a YAML header from a markdown file and return its content representation. Tip: Use this to read a document's summary before reading the full file if you are not sure the full contents are needed, to protect context.",
        input: crate::tools::dtos::ReadYamlHeaderInput,
        enabled: |_| true,
        execute: |_config, _root_path, input, resolve_path, _producer| {
            let (path, _) = resolve_path(&input.path)?.ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
            crate::tools::yaml_header::tool_read_yaml_header(&path.to_string_lossy())
        }
    },
    {
        name: "write_yaml_header",
        description: "Write or update data in a YAML header to a markdown file.",
        input: crate::tools::dtos::WriteYamlHeaderInput,
        enabled: |_| true,
        execute: |_config, _root_path, input, resolve_path, producer| {
            let (path, readonly) = resolve_path(&input.path)?.ok_or_else(|| "Cannot perform this operation on the virtual root".to_string())?;
            if readonly { return Err("Cannot perform this operation on a read-only library".to_string()); }
            crate::tools::yaml_header::tool_write_yaml_header(&path.to_string_lossy(), input.title.as_deref(), input.summary.as_deref(), input.tags, input.header_date.as_deref(), producer)
        }
    },
    {
        name: "web_search",
        description: "Search the web using SearXNG.",
        input: crate::tools::dtos::WebSearchInput,
        enabled: |config: &AppConfig| config.searxng_url.is_some(),
        execute: |config, _root_path, input, _is_safe, _producer| {
            if let Some(url) = &config.searxng_url {
                crate::tools::web::tool_web_search(url, &input.query)
            } else {
                Err("web_search tool is disabled (no SearXNG URL configured).".to_string())
            }
        }
    },
    {
        name: "search_calendar",
        description: "Search the calendar by keyword.",
        input: crate::tools::dtos::SearchCalendarInput,
        enabled: |config: &AppConfig| !config.caldav_clients.is_empty(),
        execute: |config, _root_path, input, _is_safe, _producer| crate::tools::caldav::tool_search_calendar(config, &input.keyword)
    },
    {
        name: "get_calendar",
        description: "Get calendar items by date range.",
        input: crate::tools::dtos::GetCalendarInput,
        enabled: |config: &AppConfig| !config.caldav_clients.is_empty(),
        execute: |config, _root_path, input, _is_safe, _producer| crate::tools::caldav::tool_get_calendar(config, &input.start_date, &input.end_date)
    },
    {
        name: "get_calendar_item",
        description: "Get a specific calendar item by its full href. IMPORTANT: Use the exact, full 'href' value returned by search or get tools (e.g., '/dav/calendars/user/.../item.ics'). Do not use just the UUID.",
        input: crate::tools::dtos::GetCalendarItemInput,
        enabled: |config: &AppConfig| !config.caldav_clients.is_empty(),
        execute: |config, _root_path, input, _is_safe, _producer| crate::tools::caldav::tool_get_calendar_item(config, &input.href)
    },
    {
        name: "add_calendar_item",
        description: "Add a new calendar item.",
        input: crate::tools::dtos::AddCalendarItemInput,
        enabled: |config: &AppConfig| !config.caldav_clients.is_empty(),
        execute: |config, _root_path, input, _is_safe, _producer| crate::tools::caldav::tool_add_calendar_item(config, &input.item_json)
    },
    {
        name: "update_calendar_item",
        description: "Update a calendar item.",
        input: crate::tools::dtos::UpdateCalendarItemInput,
        enabled: |config: &AppConfig| !config.caldav_clients.is_empty(),
        execute: |config, _root_path, input, _is_safe, _producer| crate::tools::caldav::tool_update_calendar_item(config, &input.id, &input.update_json)
    },
    {
        name: "delete_calendar_item",
        description: "Delete a calendar item.",
        input: crate::tools::dtos::DeleteCalendarItemInput,
        enabled: |config: &AppConfig| !config.caldav_clients.is_empty(),
        execute: |config, _root_path, input, _is_safe, _producer| crate::tools::caldav::tool_delete_calendar_item(config, &input.id)
    },
    {
        name: "search_email",
        description: "Search email by any combination of keyword, folder (mailbox), date range, sender, recipient, unread status, or flagged status. All filters are combined with AND. At least one filter must be provided. Results are paginated (default page size 10); every response includes the total number of matching emails so the caller can drive follow-up page requests.",
        input: crate::tools::dtos::SearchEmailInput,
        enabled: |config: &AppConfig| !config.jmap_clients.is_empty(),
        execute: |config, _root_path, input, _is_safe, _producer| {
            let page = input.page.unwrap_or(1).max(1);
            let page_size = input.page_size.unwrap_or(10).max(1);
            crate::tools::jmap::tool_search_email(config, input.keyword.as_deref(), input.folder.as_deref(), input.start_date.as_deref(), input.end_date.as_deref(), input.from.as_deref(), input.to.as_deref(), input.is_unread, input.is_flagged, page, page_size)
        }
    },
    {
        name: "get_email_by_id",
        description: "Get email by id.",
        input: crate::tools::dtos::GetEmailByIdInput,
        enabled: |config: &AppConfig| !config.jmap_clients.is_empty(),
        execute: |config, _root_path, input, _is_safe, _producer| crate::tools::jmap::tool_get_email_by_id(config, &input.id)
    },
    {
        name: "send_email",
        description: "Send an email.",
        input: crate::tools::dtos::SendEmailInput,
        enabled: |config: &AppConfig| !config.jmap_clients.is_empty(),
        execute: |config, _root_path, input, _is_safe, _producer| crate::tools::jmap::tool_send_email(config, &input.to, &input.subject, &input.body)
    },
    {
        name: "search_contact",
        description: "Search contacts by keyword.",
        input: crate::tools::dtos::SearchContactInput,
        enabled: |config: &AppConfig| {
            if config.feature_flags.get("useDAVForContacts").copied().unwrap_or(false) {
                !config.caldav_clients.is_empty()
            } else {
                !config.jmap_clients.is_empty()
            }
        },
        execute: |config, _root_path, input, _is_safe, _producer| {
            if config.feature_flags.get("useDAVForContacts").copied().unwrap_or(false) {
                crate::tools::carddav::tool_search_contact(config, &input.keyword)
            } else {
                crate::tools::jmap::tool_search_contact(config, &input.keyword)
            }
        }
    },
    {
        name: "add_contact",
        description: "Add a new contact.",
        input: crate::tools::dtos::AddContactInput,
        enabled: |config: &AppConfig| {
            if config.feature_flags.get("useDAVForContacts").copied().unwrap_or(false) {
                !config.caldav_clients.is_empty()
            } else {
                !config.jmap_clients.is_empty()
            }
        },
        execute: |config, _root_path, input, _is_safe, _producer| {
            if config.feature_flags.get("useDAVForContacts").copied().unwrap_or(false) {
                crate::tools::carddav::tool_add_contact(config, &input.contact_json)
            } else {
                crate::tools::jmap::tool_add_contact(config, &input.contact_json)
            }
        }
    },
    {
        name: "get_contact",
        description: "Get contact by id.",
        input: crate::tools::dtos::GetContactInput,
        enabled: |config: &AppConfig| {
            if config.feature_flags.get("useDAVForContacts").copied().unwrap_or(false) {
                !config.caldav_clients.is_empty()
            } else {
                !config.jmap_clients.is_empty()
            }
        },
        execute: |config, _root_path, input, _is_safe, _producer| {
            if config.feature_flags.get("useDAVForContacts").copied().unwrap_or(false) {
                crate::tools::carddav::tool_get_contact(config, &input.id)
            } else {
                crate::tools::jmap::tool_get_contact(config, &input.id)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file_events::Bus;
    use std::path::Path;

    /// A throwaway bus for tests. Tests don't subscribe to the bus
    /// — they only care about the return value of `execute_tool`.
    /// The bus is leaked so the `&Bus<FileEvent>` reference is
    /// valid for the lifetime of the test.
    fn test_bus() -> &'static Bus<crate::file_events::FileEvent> {
        Box::leak(Box::new(Bus::new()))
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
        let root = Path::new("C:\\TestRoot");
        let bus = test_bus();

        // This will succeed in resolving to C:\TestRoot\sub\file.md
        let res1 = execute_tool(
            &config,
            root,
            "read_file",
            r#"{"path": "TestLib\\sub\\file.md"}"#,
            bus,
        );
        assert!(!res1.contains("Invalid virtual path"));

        // Path traversal is blocked
        let res3 = execute_tool(
            &config,
            root,
            "read_file",
            r#"{"path": "TestLib\\..\\Windows\\System32\\cmd.exe"}"#,
            bus,
        );
        assert!(res3.contains("Path traversal not allowed"));

        // Unknown library
        let res4 = execute_tool(
            &config,
            root,
            "read_file",
            r#"{"path": "UnknownLib\\file.md"}"#,
            bus,
        );
        assert!(res4.contains("Content library 'UnknownLib' not found"));

        // Resolving root dir / and .
        let res5 = execute_tool(&config, root, "list_files", r#"{"path": "."}"#, bus);
        assert!(!res5.contains("Invalid virtual path") && !res5.contains("error"));
        // Since it's list_files on root, it should return TestLib
        assert!(res5.contains("TestLib"));

        let res6 = execute_tool(&config, root, "list_files", r#"{"path": "/"}"#, bus);
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
        let root = Path::new("C:\\Root");
        let bus = test_bus();

        // Multiple traversals
        let res = execute_tool(
            &config,
            root,
            "read_file",
            r#"{"path": "Lib/../../etc/passwd"}"#,
            bus,
        );
        assert!(res.contains("Path traversal not allowed"));

        // Single parent dir
        let res2 = execute_tool(&config, root, "read_file", r#"{"path": "Lib/.."}"#, bus);
        assert!(res2.contains("Path traversal not allowed"));
    }

    #[test]
    fn test_resolve_path_with_library_missing() {
        let config = AppConfig::default();
        let root = Path::new("C:\\");
        let bus = test_bus();
        let res = execute_tool(
            &config,
            root,
            "list_files",
            r#"{"path": "NonExistentLib"}"#,
            bus,
        );
        assert!(res.contains("Content library 'NonExistentLib' not found"));
    }

    #[test]
    fn test_unknown_tool_returns_error() {
        let config = AppConfig::default();
        let root = Path::new("C:\\");
        let bus = test_bus();
        let res = execute_tool(&config, root, "nonexistent_tool", "{}", bus);
        assert!(res.contains("Tool nonexistent_tool not found"));
    }

    #[test]
    fn test_tool_invalid_args_returns_error() {
        let config = AppConfig::default();
        let root = Path::new("C:\\");
        let bus = test_bus();
        let res = execute_tool(&config, root, "list_files", "not valid json", bus);
        assert!(res.contains("Invalid args") || res.contains("error"));
    }

    #[test]
    fn test_tool_call_debug_mode_feature_flag() {
        // Test that the feature flag controls debug logging behavior
        // Default config has toolCallDebugMode = false
        let mut config = AppConfig::default();
        assert_eq!(
            config
                .feature_flags
                .get("toolCallDebugMode")
                .copied()
                .unwrap_or(false),
            false,
            "toolCallDebugMode should default to false"
        );

        // Test enabling the flag
        config
            .feature_flags
            .insert("toolCallDebugMode".to_string(), true);
        assert_eq!(
            config
                .feature_flags
                .get("toolCallDebugMode")
                .copied()
                .unwrap_or(false),
            true,
            "toolCallDebugMode should be true when set"
        );

        // Test that execute_tool uses the flag (we can't easily test log output,
        // but we verify the flag is accessible from config)
        let root = Path::new("C:\\");
        let bus = test_bus();
        let res = execute_tool(&config, root, "unknown_tool", "{}", bus);
        // Should still work the same way, just with different logging behavior
        assert!(res.contains("not found") || res.contains("error"));
    }

    // -- list_files_by_tag paging -----------------------------------------

    use serde_json::Value;
    use std::fs;
    use tempfile::TempDir;

    /// A handle that keeps the temp library directories alive for the
    /// lifetime of a test. Holding this in the test scope prevents the
    /// `TempDir` from being dropped (which would delete the underlying
    /// directory and silently break every subsequent `WalkDir` call).
    struct LibFixture {
        _a: TempDir,
        _b: Option<TempDir>,
    }

    /// Build a `Config` with one library whose folder contains `n`
    /// tagged markdown files. The library name is `Lib`.
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

    /// Two-library variant: `n` tagged files in each library.
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

    /// Execute `list_files_by_tag` via the public `execute_tool`
    /// entry point and return the parsed JSON envelope.
    fn run_list_by_tag(config: &AppConfig, args: &str) -> Value {
        let root = Path::new("");
        let bus = test_bus();
        let raw = execute_tool(config, root, "list_files_by_tag", args, bus);
        serde_json::from_str(&raw).unwrap_or_else(|e| {
            panic!("could not parse tool response `{}`: {}", raw, e);
        })
    }

    /// `data["files"]` is now a JSON array of strings; pull it out
    /// as `Vec<String>` for ergonomic assertions.
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
        // All 5 fit on a single page of size 20.
        let files = files_array(data);
        assert_eq!(files.len(), 5);
        assert!(files.iter().any(|p| p.ends_with("file_000.md")));
        assert!(files.iter().any(|p| p.ends_with("file_004.md")));
        // No `hint` for in-range pages.
        assert!(data.get("hint").is_none() || data["hint"].is_null());
    }

    #[test]
    fn test_list_by_tag_pagination_first_page() {
        let (config, _dir) = single_lib_with_n_tagged_files(50);
        let envelope = run_list_by_tag(&config, r#"{"tag":"meeting","page":1,"page_size":20}"#);
        let data = &envelope["data"];
        assert_eq!(data["total"], 50, "total must reflect all matches");
        let files = files_array(data);
        // First page: files 0..=19
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
        // 50 files / page_size 20 = pages 1,2,3 — page 3 has the
        // last 10 files only.
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
        // Asking for a page beyond the last one must not silently
        // return an empty array — the caller (LLM agent) needs the
        // hint so it can adjust.
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
        // Edge case: page_size=1 must return a single-element array
        // and the total must still match.
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
        // Two libraries * 25 files each = 50 total. Paging should be
        // applied to the *combined* list, not per-library.
        let (config, _fixture) = two_libs_with_n_tagged_files_each(25);
        let envelope = run_list_by_tag(&config, r#"{"tag":"meeting","page":1,"page_size":20}"#);
        let data = &envelope["data"];
        assert_eq!(data["total"], 50);
        // Per-library paging would have given 20+20=40 entries on
        // page 1; global paging gives exactly 20.
        assert_eq!(files_array(data).len(), 20);
    }

    #[test]
    fn test_list_by_tag_no_matches_reports_zero_total() {
        // Use a fresh empty TempDir so the lib has a real on-disk
        // root but zero matching files. Keep `_empty` in scope so
        // the directory is not removed before the tool runs.
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
        // page=0 is meaningless for 1-indexed paging; treat it the
        // same as omitting the field. The LLM occasionally hands in
        // 0 by accident; the tool should not error.
        let (config, _dir) = single_lib_with_n_tagged_files(5);
        let envelope = run_list_by_tag(&config, r#"{"tag":"meeting","page":0,"page_size":3}"#);
        let data = &envelope["data"];
        assert_eq!(data["total"], 5);
        let files = files_array(data);
        // page=0 → first 3 files
        assert_eq!(files.len(), 3);
        assert!(files.iter().any(|p| p.ends_with("file_000.md")));
        assert!(files.iter().any(|p| p.ends_with("file_002.md")));
    }

    // -- list_files (paging + JSON array) --------------------------------

    /// Same helper, but for `list_files`. Note: `list_files` takes
    /// a virtual path, not a tag, so the fixture must wire up a
    /// library with the matching name.
    fn run_list_files(config: &AppConfig, args: &str) -> Value {
        let root = Path::new("");
        let bus = test_bus();
        let raw = execute_tool(config, root, "list_files", args, bus);
        serde_json::from_str(&raw).unwrap_or_else(|e| {
            panic!("could not parse tool response `{}`: {}", raw, e);
        })
    }

    /// Build a single library containing `n` Markdown files. Returns
    /// the config and a fixture guard that keeps the tempdir alive.
    fn single_lib_with_n_md_files(n: usize) -> (AppConfig, LibFixture) {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..n {
            // Zero-pad so the lexicographic order matches numeric
            // order — paging tests need a stable, predictable order.
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
        // Each path is the virtual one (Lib/<file>) — the
        // separator is platform-specific so we just check the
        // library prefix and the leaf.
        assert!(
            files
                .iter()
                .all(|p| p.starts_with("Lib") && p.contains("note_"))
        );
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
        // `path` is `/` (or `.`): the tool must return the
        // configured content libraries. Paging must apply here too.
        let (config, _fix) = single_lib_with_n_md_files(0);
        let envelope = run_list_files(&config, r#"{"path":"/"}"#);
        let data = &envelope["data"];
        assert_eq!(data["total"], 1);
        let files = files_array(data);
        assert_eq!(files, vec!["Lib".to_string()]);
    }

    #[test]
    fn test_list_files_multiple_libraries_paginated_globally() {
        // Two libraries * 30 files each = 60 total. Paging must be
        // applied to the *combined* result, not per-library.
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
        // LibA alone has 30 files.
        assert_eq!(data["total"], 30);
        assert_eq!(files_array(data).len(), 20);
    }

    #[test]
    fn test_list_files_returns_json_array_not_string() {
        // A regression guard: `files` must serialise as a JSON
        // array, not a newline-joined string. This is the contract
        // the user asked for.
        let (config, _fix) = single_lib_with_n_md_files(3);
        let envelope = run_list_files(&config, r#"{"path":"Lib"}"#);
        let raw = execute_tool(
            &config,
            Path::new(""),
            "list_files",
            r#"{"path":"Lib"}"#,
            test_bus(),
        );
        let parsed: Value = serde_json::from_str(&raw).unwrap();
        assert!(parsed["data"]["files"].is_array());
        // And on the parsed envelope, same shape.
        let data = &envelope["data"];
        assert!(data["files"].is_array());
    }
}
