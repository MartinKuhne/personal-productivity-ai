//! Unit tests for ToolRegistry.

use super::*;
use crate::app::watcher::events::Bus;
use crate::config::AppConfig;
use crate::tools::context::ToolContext;
use serde_json::Value;
use std::fs;
use tempfile::TempDir;

fn test_bus() -> &'static Bus<crate::app::watcher::events::FileEvent> {
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
    libs.sort_by_key(|b| std::cmp::Reverse(b.priority));
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
    assert!(
        !config
            .feature_flags
            .get("toolCallDebugMode")
            .copied()
            .unwrap_or(false)
    );
    config
        .feature_flags
        .insert("toolCallDebugMode".to_string(), true);
    assert!(
        config
            .feature_flags
            .get("toolCallDebugMode")
            .copied()
            .unwrap_or(false)
    );
    let ctx = test_ctx(&config);
    let res = execute_tool(&ctx, "unknown_tool", "{}");
    assert!(res.contains("not found") || res.contains("error"));
}

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
