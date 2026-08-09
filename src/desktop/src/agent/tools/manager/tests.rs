//! Unit tests for ToolRegistry.

use super::*;
use crate::agent::tools::context::ToolContext;
use crate::app::session::BrowserSession;
use crate::bus::core::Bus;
use crate::bus::events::file::FileEvent;
use crate::config::AppConfig;
use serde_json::Value;
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;

fn test_bus() -> &'static Bus<FileEvent> {
    Box::leak(Box::new(Bus::new()))
}

fn test_browser_session() -> Arc<BrowserSession> {
    Arc::new(BrowserSession::new(&AppConfig::default()))
}

fn test_ctx(config: &AppConfig) -> ToolContext<'static> {
    ToolContext::new(
        unsafe { &*(config as *const AppConfig) },
        test_bus(),
        test_browser_session(),
        Arc::new(crate::app::session::PdfBackingTracker::new()),
        crate::agent::tools::manager::cache::cache(),
        Arc::new(std::sync::RwLock::new(ToolManager::new())),
        Arc::new(crate::utils::uuid::SystemUuidGenerator),
    )
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
fn test_list_by_tag_default_limit_is_100() {
    // Create 150 files so the default limit of 100 actually clips
    // the response.
    let (config, _dir) = single_lib_with_n_tagged_files(150);
    let envelope = run_list_by_tag(&config, r#"{"tag":"meeting"}"#);
    assert_eq!(envelope["status"], "success");
    let data = &envelope["data"];
    assert_eq!(data["total"], 150);
    let files = files_array(data);
    assert_eq!(files.len(), 100);
    assert!(files.iter().any(|p| p.ends_with("file_000.md")));
    assert!(files.iter().any(|p| p.ends_with("file_099.md")));
    assert!(data.get("hint").is_none() || data["hint"].is_null());
}

#[test]
fn test_list_by_tag_paging_dispatch() {
    // Each case: (label, n_files, offset, limit, expected_len, expected_first_idx, expected_last_idx)
    // `expected_len == 0` means the response should be empty (past-end).
    struct Case {
        label: &'static str,
        n_files: usize,
        offset: u32,
        limit: u32,
        expected_len: usize,
        expected_first_idx: Option<usize>,
        expected_last_idx: Option<usize>,
    }
    let cases: &[Case] = &[
        Case {
            label: "first slice (50 files, offset 0, limit 20)",
            n_files: 50,
            offset: 0,
            limit: 20,
            expected_len: 20,
            expected_first_idx: Some(0),
            expected_last_idx: Some(19),
        },
        Case {
            label: "second slice (50 files, offset 20, limit 20)",
            n_files: 50,
            offset: 20,
            limit: 20,
            expected_len: 20,
            expected_first_idx: Some(20),
            expected_last_idx: Some(39),
        },
        Case {
            label: "last partial slice (50 files, offset 40, limit 20)",
            n_files: 50,
            offset: 40,
            limit: 20,
            expected_len: 10,
            expected_first_idx: Some(40),
            expected_last_idx: Some(49),
        },
        Case {
            label: "offset past end (5 files, offset 999, limit 20)",
            n_files: 5,
            offset: 999,
            limit: 20,
            expected_len: 0,
            expected_first_idx: None,
            expected_last_idx: None,
        },
        Case {
            label: "limit one (3 files, offset 1, limit 1)",
            n_files: 3,
            offset: 1,
            limit: 1,
            expected_len: 1,
            expected_first_idx: Some(1),
            expected_last_idx: Some(1),
        },
    ];

    for case in cases {
        let (config, _dir) = single_lib_with_n_tagged_files(case.n_files);
        let envelope = run_list_by_tag(
            &config,
            &format!(
                r#"{{"tag":"meeting","offset":{},"limit":{}}}"#,
                case.offset, case.limit
            ),
        );
        let data = &envelope["data"];
        let files = files_array(data);
        assert_eq!(
            files.len(),
            case.expected_len,
            "[{}] page count mismatch (total={}, got {} files)",
            case.label,
            data["total"],
            files.len()
        );
        if let (Some(first), Some(last)) = (case.expected_first_idx, case.expected_last_idx) {
            assert!(
                files
                    .first()
                    .is_some_and(|p| p.ends_with(&format!("file_{first:03}.md"))),
                "[{}] first file mismatch; got {:?}",
                case.label,
                files.first()
            );
            assert!(
                files
                    .last()
                    .is_some_and(|p| p.ends_with(&format!("file_{last:03}.md"))),
                "[{}] last file mismatch; got {:?}",
                case.label,
                files.last()
            );
        }
        if case.expected_len == 0 {
            // Past-end case: a `hint` field with a useful message.
            let hint = data["hint"]
                .as_str()
                .unwrap_or_else(|| panic!("[{}] expected hint on past-end page", case.label));
            assert!(
                hint.contains(&format!("offset {}", case.offset))
                    && hint.contains(&format!("{} total", case.n_files)),
                "[{}] hint text mismatch: {hint}",
                case.label
            );
        }
    }
}

#[test]
fn test_list_by_tag_paging_is_global_across_libraries() {
    let (config, _fixture) = two_libs_with_n_tagged_files_each(25);
    let envelope = run_list_by_tag(&config, r#"{"tag":"meeting","offset":0,"limit":20}"#);
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
fn test_list_by_tag_offset_zero_returns_first_slice() {
    // With the new offset model, offset 0 is a literal "first slice"
    // request; no normalisation is needed. The response is the same
    // as omitting the offset entirely.
    let (config, _dir) = single_lib_with_n_tagged_files(5);
    let envelope = run_list_by_tag(&config, r#"{"tag":"meeting","offset":0,"limit":3}"#);
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
fn test_list_files_default_limit_is_100() {
    // Create 150 files so the default limit of 100 actually clips
    // the response.
    let (config, _fix) = single_lib_with_n_md_files(150);
    let envelope = run_list_files(&config, r#"{"path":"Lib"}"#);
    let data = &envelope["data"];
    assert_eq!(data["total"], 150);
    let files = files_array(data);
    assert_eq!(files.len(), 100);
    assert!(
        files
            .iter()
            .all(|p| p.starts_with("Lib") && p.contains("note_"))
    );
}

#[test]
fn test_list_files_paging_dispatch() {
    // Each case: (label, n_files, offset, limit, expected_len, expected_first_idx, expected_last_idx)
    struct Case {
        label: &'static str,
        n_files: usize,
        offset: u32,
        limit: u32,
        expected_len: usize,
        expected_first_idx: Option<usize>,
        expected_last_idx: Option<usize>,
    }
    let cases: &[Case] = &[
        Case {
            label: "first slice (50 files, offset 0, limit 20)",
            n_files: 50,
            offset: 0,
            limit: 20,
            expected_len: 20,
            expected_first_idx: Some(0),
            expected_last_idx: Some(19),
        },
        Case {
            label: "last partial slice (50 files, offset 40, limit 20)",
            n_files: 50,
            offset: 40,
            limit: 20,
            expected_len: 10,
            expected_first_idx: Some(40),
            expected_last_idx: Some(49),
        },
        Case {
            label: "offset past end (5 files, offset 999, limit 20)",
            n_files: 5,
            offset: 999,
            limit: 20,
            expected_len: 0,
            expected_first_idx: None,
            expected_last_idx: None,
        },
    ];

    for case in cases {
        let (config, _fix) = single_lib_with_n_md_files(case.n_files);
        let envelope = run_list_files(
            &config,
            &format!(
                r#"{{"path":"Lib","offset":{},"limit":{}}}"#,
                case.offset, case.limit
            ),
        );
        let data = &envelope["data"];
        let files = files_array(data);
        assert_eq!(
            files.len(),
            case.expected_len,
            "[{}] page count mismatch (total={}, got {} files)",
            case.label,
            data["total"],
            files.len()
        );
        if let (Some(first), Some(last)) = (case.expected_first_idx, case.expected_last_idx) {
            assert!(
                files
                    .first()
                    .is_some_and(|p| p.ends_with(&format!("note_{first:03}.md"))),
                "[{}] first file mismatch; got {:?}",
                case.label,
                files.first()
            );
            assert!(
                files
                    .last()
                    .is_some_and(|p| p.ends_with(&format!("note_{last:03}.md"))),
                "[{}] last file mismatch; got {:?}",
                case.label,
                files.last()
            );
        }
        if case.expected_len == 0 {
            let hint = data["hint"]
                .as_str()
                .unwrap_or_else(|| panic!("[{}] expected hint on past-end page", case.label));
            assert!(
                hint.contains(&format!("offset {}", case.offset))
                    && hint.contains(&format!("{} total", case.n_files)),
                "[{}] hint text mismatch: {hint}",
                case.label
            );
        }
    }
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
fn test_list_files_multiple_libraries_paging_global() {
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
    let envelope = run_list_files(&config, r#"{"path":"LibA","offset":0,"limit":20}"#);
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

fn run_grep(config: &AppConfig, args: &str) -> Value {
    let ctx = test_ctx(config);
    let raw = execute_tool(&ctx, "grep", args);
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("could not parse tool response `{}`: {}", raw, e))
}

fn single_lib_with_files(files: &[(&str, &str)]) -> (AppConfig, TempDir) {
    let dir = tempfile::tempdir().unwrap();
    for (name, content) in files {
        fs::write(dir.path().join(name), content).unwrap();
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
    (config, dir)
}

#[test]
fn test_grep_no_matches_keeps_sentinel() {
    let (config, _dir) = single_lib_with_files(&[("note.md", "# Doc")]);
    let envelope = run_grep(&config, r#"{"query":"needle"}"#);
    assert_eq!(envelope["status"], "success");
    let data = &envelope["data"];
    assert_eq!(data["matches"], "No matches found.");
    assert_eq!(data["total"], 0);
    assert_eq!(data["truncated"], false);
}

#[test]
fn test_grep_returns_matches_within_limit() {
    let (config, _dir) = single_lib_with_files(&[("note.md", "needle one\nother\nneedle two")]);
    let envelope = run_grep(&config, r#"{"query":"needle"}"#);
    assert_eq!(envelope["status"], "success");
    let data = &envelope["data"];
    assert_eq!(data["total"], 2);
    assert_eq!(data["truncated"], false);
    let matches = data["matches"].as_str().unwrap();
    assert!(matches.starts_with("Lib"));
    assert!(matches.contains("note.md"));
    assert!(!matches.contains("truncated"));
}

#[test]
fn test_grep_truncates_at_default_max_results() {
    let content = (0..250)
        .map(|i| format!("needle line {}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let (config, _dir) = single_lib_with_files(&[("big.md", &content)]);
    let envelope = run_grep(&config, r#"{"query":"needle"}"#);
    assert_eq!(envelope["status"], "success");
    let data = &envelope["data"];
    assert_eq!(data["total"], 250);
    assert_eq!(data["truncated"], true);
    let matches = data["matches"].as_str().unwrap();
    // 200 matching lines plus the truncation notice.
    assert_eq!(matches.lines().count(), 201);
    assert!(matches.contains("results truncated at 200 matches"));
    assert!(matches.contains("narrower terms"));
}

#[test]
fn test_grep_only_matches_md_files() {
    let (config, _dir) = single_lib_with_files(&[
        ("note.md", "needle in md"),
        ("note.markdown", "needle in markdown"),
        ("note.txt", "needle in txt"),
    ]);
    let envelope = run_grep(&config, r#"{"query":"needle"}"#);
    let data = &envelope["data"];
    assert_eq!(data["total"], 1);
    let matches = data["matches"].as_str().unwrap();
    assert!(matches.contains("note.md"));
    assert!(!matches.contains("note.markdown"));
    assert!(!matches.contains("note.txt"));
}

#[test]
fn test_grep_truncation_does_not_count_non_markdown_matches() {
    // Non-markdown files must never consume the 200-match cap: a large
    // `.txt` with many matches must not appear in, or crowd out, the
    // Markdown results.
    let md_content = (0..250)
        .map(|i| format!("needle line {}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let txt_content = (0..1000)
        .map(|i| format!("needle txt {}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let (config, _dir) =
        single_lib_with_files(&[("big.md", &md_content), ("noise.txt", &txt_content)]);
    let envelope = run_grep(&config, r#"{"query":"needle"}"#);
    let data = &envelope["data"];
    assert_eq!(data["total"], 250);
    assert_eq!(data["truncated"], true);
    let matches = data["matches"].as_str().unwrap();
    assert_eq!(matches.lines().count(), 201);
    assert!(matches.contains("results truncated at 200 matches"));
    assert!(!matches.contains("needle txt"));
}

#[test]
fn test_csv_tools_in_schema() {
    let config = AppConfig::default();
    let mut mgr = ToolManager::new();
    let schema = mgr.get_tools_schema(&config, "create a csv database");
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
    let mut mgr = ToolManager::new();
    let schema = mgr.get_tools_schema(&config, "just a normal message");
    let tools = schema.as_array().unwrap();
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t["function"]["name"].as_str())
        .collect();
    assert!(!names.contains(&"create_csv"));
    assert!(!names.contains(&"list_csv"));
}

#[test]
fn test_get_weather_tool_in_schema() {
    let config = AppConfig::default();
    let mut mgr = ToolManager::new();
    let schema = mgr.get_tools_schema(&config, "what is the weather today");
    let tools = schema.as_array().unwrap();
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t["function"]["name"].as_str())
        .collect();
    assert!(names.contains(&"get_weather"));
}

#[test]
fn test_get_weather_tool_excluded_when_disabled() {
    let mut config = AppConfig::default();
    config.tool_groups.weather = false;
    let mut mgr = ToolManager::new();
    let schema = mgr.get_tools_schema(&config, "what is the weather today");
    let tools = schema.as_array().unwrap();
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t["function"]["name"].as_str())
        .collect();
    assert!(!names.contains(&"get_weather"));
}

/// The tools-manager config-bus subscription must run the MCP
/// startup init on a background thread (never the caller's thread)
/// and post a completion log entry. The default config has no MCP
/// servers, so the init is a fast, offline no-op and the test is
/// deterministic.
#[test]
fn test_spawn_config_subscription_runs_init_in_background() {
    use crate::bus::events::config::ConfigArrived;
    use crate::bus::events::typed::{BackgroundEvent, ProcessEvent};

    let bus = crate::bus::config::config_bus();
    let (tx, rx) = std::sync::mpsc::channel::<BackgroundEvent>();

    let tm = Arc::new(std::sync::RwLock::new(ToolManager::new()));
    spawn_config_subscription(tm, bus.clone(), tx);

    bus.publish(ConfigArrived::new(AppConfig::default()));

    let start = std::time::Instant::now();
    while start.elapsed().as_secs() < 5 {
        match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(BackgroundEvent::Process(ProcessEvent::LogEntry(entry))) => {
                assert!(
                    entry
                        .message
                        .starts_with("MCP startup initialization complete"),
                    "unexpected log entry: {}",
                    entry.message
                );
                return;
            }
            Ok(_) | Err(_) => continue,
        }
    }
    panic!("no MCP startup log entry observed within timeout");
}

#[test]
fn test_mcp_char_count_bug() {
    let mut config = AppConfig::default();
    config.mcp_servers.insert("test_mcp".into(), crate::config::McpServerConfig::Stdio { command: "".into(), args: vec![], env: Default::default() }.into());
    let mut mgr = ToolManager::new();
    mgr.register_mcp_tool("test_mcp", "mcp_test_mcp_test_tool", "desc", serde_json::json!({}));
    
    // Simulate what the ui does:
    let count = mgr.tool_char_count("mcp_test_mcp_test_tool", &config, "");
    
    // Also, what happens when mcp tool doesn't have mcp_ prefixed name?
    mgr.register_mcp_tool("test_mcp", "test_tool", "desc", serde_json::json!({}));
    let count2 = mgr.tool_char_count("test_tool", &config, "");
    assert_eq!(count, Some(116));
    assert_eq!(count2, Some(103));
}
