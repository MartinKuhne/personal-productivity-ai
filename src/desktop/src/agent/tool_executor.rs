//! Tool-call dispatcher — receives tool-call JSON from the LLM, dispatches through the registry, and feeds results back.

use crate::config::AppConfig;
use crate::file_events::{Bus, FileEvent};
use crate::messages::BackgroundMessage;
use crate::tools::context::ToolContext;
use crate::tools::execute_tool;
use std::path::Path;
use std::sync::mpsc::Sender;

pub struct ToolExecutor {
    config: AppConfig,
    file_event_bus: Bus<FileEvent>,
}

impl ToolExecutor {
    pub fn new(config: AppConfig, file_event_bus: Bus<FileEvent>) -> Self {
        Self {
            config,
            file_event_bus,
        }
    }

    pub fn execute_all(
        &self,
        tool_calls: &[serde_json::Value],
        tx_gui: &Sender<BackgroundMessage>,
    ) -> Vec<(String, String, String, String)> {
        let mut safe_calls = Vec::new();
        let mut unsafe_calls = Vec::new();
        for tc in tool_calls {
            let func_name = tc
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
                .unwrap_or("");
            if is_safe_tool(func_name) {
                safe_calls.push(tc.clone());
            } else {
                unsafe_calls.push(tc.clone());
            }
        }
        let mut results = self.execute_parallel(&safe_calls);
        results.extend(self.execute_sequential(&unsafe_calls));
        self.notify_file_creations(&results, tx_gui);
        results
    }

    fn execute_parallel(
        &self,
        calls: &[serde_json::Value],
    ) -> Vec<(String, String, String, String)> {
        let rt = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                tracing::error!(name = "agent.runtime.build_failed", error = %e);
                return Vec::new();
            }
        };
        let config_arc = std::sync::Arc::new(self.config.clone());
        let mut completed = Vec::new();
        rt.block_on(async {
            let mut join_set = tokio::task::JoinSet::new();
            for tc in calls {
                let call_id = extract_str(tc, &["id"]).to_string();
                let func_name = extract_str(tc, &["function", "name"]).to_string();
                let func_args = extract_str(tc, &["function", "arguments"]).to_string();
                let cfg = config_arc.clone();
                let bus = self.file_event_bus.clone();
                join_set.spawn_blocking(move || {
                    let ctx = ToolContext::new(&cfg, &bus);
                    let result = execute_tool(&ctx, &func_name, &func_args);
                    (call_id, func_name, func_args, result)
                });
            }
            while let Some(res) = join_set.join_next().await {
                if let Ok(data) = res {
                    completed.push(data);
                }
            }
        });
        completed
    }

    fn execute_sequential(
        &self,
        calls: &[serde_json::Value],
    ) -> Vec<(String, String, String, String)> {
        let mut results = Vec::new();
        for tc in calls {
            let call_id = extract_str(tc, &["id"]).to_string();
            let func_name = extract_str(tc, &["function", "name"]).to_string();
            let func_args = extract_str(tc, &["function", "arguments"]).to_string();
            let ctx = ToolContext::new(&self.config, &self.file_event_bus);
            let result = execute_tool(&ctx, &func_name, &func_args);
            results.push((call_id, func_name, func_args, result));
        }
        results
    }

    fn notify_file_creations(
        &self,
        results: &[(String, String, String, String)],
        tx_gui: &Sender<BackgroundMessage>,
    ) {
        for (_call_id, func_name, func_args_str, result) in results {
            if func_name != "create_file" {
                continue;
            }
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(result) {
                if parsed.get("status").and_then(|s| s.as_str()) != Some("success") {
                    continue;
                }
                let path_owned: String = match serde_json::from_str::<serde_json::Value>(func_args_str) {
                    Ok(v) => match v.get("path").and_then(|p| p.as_str()).map(|s| s.to_string()) {
                        Some(s) => s,
                        None => continue,
                    },
                    Err(_) => continue,
                };
                let vpath = Path::new(&path_owned);
                let mut comps = vpath.components().peekable();
                while let Some(c) = comps.peek() {
                    match c {
                        std::path::Component::RootDir | std::path::Component::CurDir => {
                            comps.next();
                        }
                        _ => break,
                    }
                }
                if let Some(std::path::Component::Normal(first)) = comps.next() {
                    let lib_name = first.to_string_lossy();
                    for lib in &self.config.content_libraries {
                        if lib.name == lib_name {
                            let rest: std::path::PathBuf = comps.collect();
                            let abs_path = Path::new(&lib.root_folder).join(rest);
                            let tags = crate::utils::tags::extract_tags_from_file(&abs_path);
                            let _ = tx_gui.send(BackgroundMessage::FileModified {
                                path: abs_path,
                                tags,
                            });
                            break;
                        }
                    }
                }
            }
        }
    }
}

fn extract_str<'a>(val: &'a serde_json::Value, path: &[&str]) -> &'a str {
    let mut current = val;
    for key in path {
        match current.get(key) {
            Some(v) => current = v,
            None => return "",
        }
    }
    current.as_str().unwrap_or("")
}

fn is_safe_tool(name: &str) -> bool {
    matches!(
        name,
        "grep"
            | "read_tags"
            | "list_files_by_tag"
            | "list_files"
            | "read_file"
            | "read_file_lines"
            | "web_fetch"
            | "read_yaml_header"
            | "web_search"
            | "search_calendar"
            | "get_calendar"
            | "get_calendar_item"
            | "search_email"
            | "get_email_by_id"
            | "search_contact"
            | "get_contact"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_safe_tool_read() {
        assert!(is_safe_tool("read_file"));
        assert!(is_safe_tool("grep"));
        assert!(is_safe_tool("list_files"));
    }

    #[test]
    fn test_is_safe_tool_write() {
        assert!(!is_safe_tool("create_file"));
        assert!(!is_safe_tool("delete_file"));
        assert!(!is_safe_tool("edit_file"));
    }

    #[test]
    fn test_extract_str_nested() {
        let val = serde_json::json!({
            "function": { "name": "test", "arguments": "{}" },
            "id": "call_1"
        });
        assert_eq!(extract_str(&val, &["id"]), "call_1");
        assert_eq!(extract_str(&val, &["function", "name"]), "test");
        assert_eq!(extract_str(&val, &["missing"]), "");
    }

    #[test]
    fn test_tool_executor_new() {
        let config = AppConfig::default();
        let bus = Bus::new();
        let executor = ToolExecutor::new(config, bus);
        assert!(executor.config.models.is_empty());
    }
}
