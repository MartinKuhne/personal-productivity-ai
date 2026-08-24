//! Unit tests for ToolRegistry.

use super::*;
use crate::config::AgentConfig;
use crate::tools::context::ToolContext;
use serde_json::Value;
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;

fn test_ctx(config: &AgentConfig) -> ToolContext {
    crate::tools::context::ToolContextBuilder::new(
        Arc::new(config.clone()),
        std::sync::Arc::new(crate::tools::observer::DefaultFileObserver),
    )
    .with_extension(std::sync::Arc::new(crate::tools::context::ToolCacheExt(
        Arc::new(crate::tools::registry::cache::ToolCache::new()),
    )))
    .with_extension(std::sync::Arc::new(
        crate::tools::context::UuidGeneratorExt(Arc::new(crate::utils::uuid::SystemUuidGenerator)),
    ))
    .with_tool_call_policy(Arc::new(crate::tools::policy::DefaultToolCallPolicy))
    .build()
}

#[test]
fn test_resolve_virtual_path() {
    let mut config = AgentConfig::default();
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

    let res1 = execute_tool(
        &ToolRegistry::new(),
        &ctx,
        "read_note",
        r#"{"path": "TestLib\\sub\\file.md"}"#,
    );
    assert!(!res1.contains("Invalid virtual path"));

    let res3 = execute_tool(
        &ToolRegistry::new(),
        &ctx,
        "read_note",
        r#"{"path": "TestLib\\..\\Windows\\System32\\cmd.exe"}"#,
    );
    assert!(res3.contains("path traversal"));

    let res4 = execute_tool(
        &ToolRegistry::new(),
        &ctx,
        "read_note",
        r#"{"path": "UnknownLib\\file.md"}"#,
    );
    assert!(res4.contains("Content library 'UnknownLib' not found"));

    let res5 = execute_tool(&ToolRegistry::new(), &ctx, "list_notes", r#"{"path": "."}"#);
    assert!(!res5.contains("Invalid virtual path") && !res5.contains("error"));
    assert!(res5.contains("TestLib"));

    let res6 = execute_tool(&ToolRegistry::new(), &ctx, "list_notes", r#"{"path": "/"}"#);
    assert!(!res6.contains("Invalid virtual path") && !res6.contains("error"));
    assert!(res6.contains("TestLib"));
}

#[test]
fn test_grep_priority_ordering() {
    let mut config = AgentConfig::default();
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
    let mut config = AgentConfig::default();
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

    let res = execute_tool(
        &ToolRegistry::new(),
        &ctx,
        "read_note",
        r#"{"path": "Lib/../../etc/passwd"}"#,
    );
    assert!(res.contains("path traversal"));

    let res2 = execute_tool(
        &ToolRegistry::new(),
        &ctx,
        "read_note",
        r#"{"path": "Lib/.."}"#,
    );
    assert!(res2.contains("path traversal"));
}

#[test]
fn test_resolve_path_with_library_missing() {
    let config = AgentConfig::default();
    let ctx = test_ctx(&config);
    let res = execute_tool(
        &ToolRegistry::new(),
        &ctx,
        "list_notes",
        r#"{"path": "NonExistentLib/file.md"}"#,
    );
    assert!(res.contains("Content library 'NonExistentLib' not found"));
}

#[test]
fn test_unknown_tool_returns_error() {
    let config = AgentConfig::default();
    let ctx = test_ctx(&config);
    let res = execute_tool(&ToolRegistry::new(), &ctx, "nonexistent_tool", "{}");
    assert!(res.contains("Tool nonexistent_tool not found"));
}

#[test]
fn test_tool_invalid_args_returns_error() {
    let config = AgentConfig::default();
    let ctx = test_ctx(&config);
    let res = execute_tool(&ToolRegistry::new(), &ctx, "list_notes", "not valid json");
    assert!(res.contains("Invalid args") || res.contains("error"));
}

#[test]
fn test_tool_call_debug_mode_feature_flag() {
    let mut config = AgentConfig::default();
    assert!(!config
        .feature_flags
        .get("toolCallDebugMode")
        .copied()
        .unwrap_or(false));
    config
        .feature_flags
        .insert("toolCallDebugMode".to_string(), true);
    assert!(config
        .feature_flags
        .get("toolCallDebugMode")
        .copied()
        .unwrap_or(false));
    let ctx = test_ctx(&config);
    let res = execute_tool(&ToolRegistry::new(), &ctx, "unknown_tool", "{}");
    assert!(res.contains("not found") || res.contains("error"));
}

struct LibFixture {
    _a: TempDir,
    _b: Option<TempDir>,
}

fn single_lib_with_n_tagged_files(n: usize) -> (AgentConfig, LibFixture) {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..n {
        let name = format!("file_{:03}.md", i);
        let body = format!("---\ntags: [meeting]\n---\n# Doc {}\n", i);
        fs::write(dir.path().join(name), body).unwrap();
    }
    let mut config = AgentConfig::default();
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

fn two_libs_with_n_tagged_files_each(n: usize) -> (AgentConfig, LibFixture) {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    for i in 0..n {
        let body = format!("---\ntags: [meeting]\n---\n# Doc {}\n", i);
        fs::write(a.path().join(format!("a_{:03}.md", i)), &body).unwrap();
        fs::write(b.path().join(format!("b_{:03}.md", i)), &body).unwrap();
    }
    let mut config = AgentConfig::default();
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

fn run_list_by_tag(config: &AgentConfig, args: &str) -> Value {
    let ctx = test_ctx(config);
    run_list_by_tag_with_context(&ctx, args)
}

fn run_list_by_tag_with_context(ctx: &ToolContext, args: &str) -> Value {
    let raw = execute_tool(&ToolRegistry::new(), ctx, "list_notes_by_tag", args);
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
fn test_list_by_tag_cursor_pagination_64_page_size() {
    let (config, _dir) = single_lib_with_n_tagged_files(150);
    let ctx = test_ctx(&config);
    // Page 1
    let envelope1 = run_list_by_tag_with_context(&ctx, r#"{"tag":"meeting"}"#);
    assert_eq!(envelope1["status"], "success");
    let data1 = &envelope1["data"];
    assert_eq!(data1["total"], 150);
    let files1 = files_array(data1);
    assert_eq!(files1.len(), 64);
    assert!(data1["cursor"].is_string());
    assert!(data1["hint"].is_null());
    let cursor1 = data1["cursor"].as_str().unwrap();

    // Page 2
    let envelope2 = run_list_by_tag_with_context(
        &ctx,
        &format!(r#"{{"tag":"meeting","cursor":"{cursor1}"}}"#),
    );
    assert_eq!(envelope2["status"], "success");
    let data2 = &envelope2["data"];
    assert_eq!(data2["total"], 150);
    let files2 = files_array(data2);
    assert_eq!(files2.len(), 64);
    assert!(data2["cursor"].is_string());
    assert!(data2["hint"].is_null());
    let cursor2 = data2["cursor"].as_str().unwrap();

    // Page 3 (final)
    let envelope3 = run_list_by_tag_with_context(
        &ctx,
        &format!(r#"{{"tag":"meeting","cursor":"{cursor2}"}}"#),
    );
    assert_eq!(envelope3["status"], "success");
    let data3 = &envelope3["data"];
    assert_eq!(data3["total"], 150);
    let files3 = files_array(data3);
    assert_eq!(files3.len(), 22);
    assert!(data3["cursor"].is_null());
    assert_eq!(data3["hint"], "Final page.");
}

#[test]
fn test_list_by_tag_single_page_has_hint_and_no_cursor() {
    let (config, _dir) = single_lib_with_n_tagged_files(5);
    let envelope = run_list_by_tag(&config, r#"{"tag":"meeting"}"#);
    assert_eq!(envelope["status"], "success");
    let data = &envelope["data"];
    assert_eq!(data["total"], 5);
    let files = files_array(data);
    assert_eq!(files.len(), 5);
    assert!(data["cursor"].is_null());
    assert_eq!(data["hint"], "Final page.");
}

#[test]
fn test_list_by_tag_paging_is_global_across_libraries() {
    let (config, _fixture) = two_libs_with_n_tagged_files_each(25);
    let envelope = run_list_by_tag(&config, r#"{"tag":"meeting"}"#);
    let data = &envelope["data"];
    assert_eq!(data["total"], 50);
    assert_eq!(files_array(data).len(), 50);
    assert_eq!(data["hint"], "Final page.");
}

#[test]
fn test_list_by_tag_no_matches_reports_zero_total() {
    let _empty = tempfile::tempdir().unwrap();
    let mut config = AgentConfig::default();
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

fn run_list_files(config: &AgentConfig, args: &str) -> Value {
    let ctx = test_ctx(config);
    let raw = execute_tool(&ToolRegistry::new(), &ctx, "list_notes", args);
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("could not parse tool response `{}`: {}", raw, e))
}

fn single_lib_with_n_md_files(n: usize) -> (AgentConfig, LibFixture) {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..n {
        let name = format!("note_{:03}.md", i);
        fs::write(dir.path().join(name), "# Just a doc").unwrap();
    }
    let mut config = AgentConfig::default();
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
    assert!(files
        .iter()
        .all(|p| p.starts_with("Lib") && p.contains("note_")));
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
    let mut config = AgentConfig::default();
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
    let raw = execute_tool(
        &ToolRegistry::new(),
        &ctx,
        "list_notes",
        r#"{"path":"Lib"}"#,
    );
    let parsed: Value = serde_json::from_str(&raw).unwrap();
    assert!(parsed["data"]["files"].is_array());
}

fn run_grep(config: &AgentConfig, args: &str) -> Value {
    let ctx = test_ctx(config);
    run_grep_with_context(&ctx, args)
}

fn run_grep_with_context(ctx: &ToolContext, args: &str) -> Value {
    let raw = execute_tool(&ToolRegistry::new(), ctx, "search_notes", args);
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("could not parse tool response `{}`: {}", raw, e))
}

fn single_lib_with_files(files: &[(&str, &str)]) -> (AgentConfig, TempDir) {
    let dir = tempfile::tempdir().unwrap();
    for (name, content) in files {
        fs::write(dir.path().join(name), content).unwrap();
    }
    let mut config = AgentConfig::default();
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
    assert!(data["cursor"].is_null());
    assert_eq!(data["hint"], "Final page.");
}

#[test]
fn test_grep_returns_matches_within_limit() {
    let (config, _dir) = single_lib_with_files(&[("note.md", "needle one\nother\nneedle two")]);
    let envelope = run_grep(&config, r#"{"query":"needle"}"#);
    assert_eq!(envelope["status"], "success");
    let data = &envelope["data"];
    assert_eq!(data["total"], 2);
    assert!(data["cursor"].is_null());
    assert_eq!(data["hint"], "Final page.");
    let matches = data["matches"].as_str().unwrap();
    assert!(matches.starts_with("Lib"));
    assert!(matches.contains("note.md"));
}

#[test]
fn test_grep_cursor_pagination_64_page_size() {
    let content = (0..150)
        .map(|i| format!("needle line {}", i))
        .collect::<Vec<_>>()
        .join("\n");
    let (config, _dir) = single_lib_with_files(&[("big.md", &content)]);
    let ctx = test_ctx(&config);
    // Page 1
    let envelope1 = run_grep_with_context(&ctx, r#"{"query":"needle"}"#);
    assert_eq!(envelope1["status"], "success");
    let data1 = &envelope1["data"];
    assert_eq!(data1["total"], 150);
    assert!(data1["cursor"].is_string());
    assert!(data1["hint"].is_null());
    let matches1 = data1["matches"].as_str().unwrap();
    assert_eq!(matches1.lines().count(), 64);
    let cursor1 = data1["cursor"].as_str().unwrap();

    // Page 2
    let envelope2 = run_grep_with_context(
        &ctx,
        &format!(r#"{{"query":"needle","cursor":"{cursor1}"}}"#),
    );
    assert_eq!(envelope2["status"], "success");
    let data2 = &envelope2["data"];
    assert_eq!(data2["total"], 150);
    assert!(data2["cursor"].is_string());
    assert!(data2["hint"].is_null());
    let matches2 = data2["matches"].as_str().unwrap();
    assert_eq!(matches2.lines().count(), 64);
    let cursor2 = data2["cursor"].as_str().unwrap();

    // Page 3 (final)
    let envelope3 = run_grep_with_context(
        &ctx,
        &format!(r#"{{"query":"needle","cursor":"{cursor2}"}}"#),
    );
    assert_eq!(envelope3["status"], "success");
    let data3 = &envelope3["data"];
    assert_eq!(data3["total"], 150);
    assert!(data3["cursor"].is_null());
    assert_eq!(data3["hint"], "Final page.");
    let matches3 = data3["matches"].as_str().unwrap();
    assert_eq!(matches3.lines().count(), 22);
}

#[test]
fn test_grep_matches_md_and_markdown_files_but_not_others() {
    // search_notes covers .md and .markdown (consistent with list_notes_by_tag / read_tags)
    // but must NOT scan .txt or other file types.
    let (config, _dir) = single_lib_with_files(&[
        ("note.md", "needle in md"),
        ("note.markdown", "needle in markdown"),
        ("note.txt", "needle in txt"),
    ]);
    let envelope = run_grep(&config, r#"{"query":"needle"}"#);
    let data = &envelope["data"];
    assert_eq!(data["total"], 2);
    let matches = data["matches"].as_str().unwrap();
    assert!(matches.contains("note.md"));
    assert!(matches.contains("note.markdown"));
    assert!(!matches.contains("note.txt"));
}

#[test]
fn test_grep_does_not_count_non_markdown_matches() {
    let md_content = (0..100)
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
    assert_eq!(data["total"], 100);
    let matches = data["matches"].as_str().unwrap();
    assert_eq!(matches.lines().count(), 64);
    assert!(!matches.contains("needle txt"));
}

#[test]
fn test_csv_tools_in_schema() {
    let config = AgentConfig::default();
    let mut mgr = ToolRegistry::new();
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
    let config = AgentConfig::default();
    let mut mgr = ToolRegistry::new();
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
    let config = AgentConfig::default();
    let mut mgr = ToolRegistry::new();
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
    let mut config = AgentConfig::default();
    config.tool_groups.weather = false;
    let mut mgr = ToolRegistry::new();
    let schema = mgr.get_tools_schema(&config, "what is the weather today");
    let tools = schema.as_array().unwrap();
    let names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t["function"]["name"].as_str())
        .collect();
    assert!(!names.contains(&"get_weather"));
}

#[test]
fn test_mcp_char_count_bug() {
    let mut config = AgentConfig::default();
    config.mcp_servers.insert(
        "test_mcp".into(),
        crate::config::McpServerConfig::Stdio {
            command: "".into(),
            args: vec![],
            env: Default::default(),
        }
        .into(),
    );
    let mut mgr = ToolRegistry::new();
    mgr.register_mcp_tool(
        "test_mcp",
        "mcp_test_mcp_test_tool",
        "desc",
        serde_json::json!({}),
    );

    // Simulate what the ui does:
    let count = mgr.tool_char_count("mcp_test_mcp_test_tool", &config, "");

    // Also, what happens when mcp tool doesn't have mcp_ prefixed name?
    mgr.register_mcp_tool("test_mcp", "test_tool", "desc", serde_json::json!({}));
    let count2 = mgr.tool_char_count("test_tool", &config, "");
    assert_eq!(count, Some(116));
    assert_eq!(count2, Some(103));
}

#[test]
fn test_default_providers_register_every_family() {
    use crate::tools::registry::builtin::default_providers;
    let provider_count = default_providers().len();
    assert!(
        provider_count >= 9,
        "default_providers should list every built-in family (got {provider_count})"
    );

    // Each provider must contribute at least one tool, and every
    // tool's descriptor must report the provider's group, so the
    // registry's reverse index is consistent.
    for provider in default_providers() {
        let tools = provider.tools();
        assert!(
            !tools.is_empty(),
            "provider {:?} returned no tools",
            provider.id()
        );
        let expected_group = provider.group();
        for tool in &tools {
            assert_eq!(
                tool.descriptor.group, expected_group,
                "tool {} reports a different group than its provider",
                tool.descriptor.name
            );
        }
    }

    // The ToolRegistry constructor iterates the default provider
    // list, so every family is represented in the live catalog.
    // Spot-check one tool per family.
    let mgr = ToolRegistry::new();
    for expected in [
        "read_note",
        "write_yaml_header",
        "web_fetch",
        "web_search",
        "search_email",
        "search_calendar",
        "search_contact",
        "create_csv",
        "get_weather",
        "trello_get_boards",
    ] {
        assert!(
            mgr.descriptor(expected).is_some(),
            "expected tool {expected} in default registry"
        );
    }
}
