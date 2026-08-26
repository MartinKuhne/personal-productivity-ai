//! Tests for `tree/handlers.rs`.
//!
//! All tests build a [`TreeNodeContext`] directly with owned
//! fields and `..Default::default()`. The previous version
//! borrowed every field from local `let mut` variables and
//! required 18 `Box::leak` calls per test to satisfy a
//! `'static` re-borrow across the harness closure. The
//! lifetime-free rewrite drops that machinery entirely.

use super::*;
use crate::ui::tree::context::TreeNodeContext;
use eframe::egui;
use std::collections::HashSet;
use std::path::PathBuf;

/// Tier 1 test: a file row click with no modifier replaces
/// the single selection with the clicked file and pushes it
/// onto `tabs` if not already there.
#[test]
fn test_apply_file_row_click_no_modifier_replaces_selection_and_opens_tab() {
    let row = FlatRow {
        depth: 0,
        name: "b.md".to_string(),
        path: PathBuf::from("b.md"),
        is_dir: false,
        is_expanded: false,
    };
    let mut ctx = TreeNodeContext {
        selected_file: Some(PathBuf::from("a.md")),
        selected_files: HashSet::from([PathBuf::from("a.md")]),
        tabs: vec![PathBuf::from("a.md")],
        ..Default::default()
    };

    apply_file_row_click(&mut ctx, &row);

    assert_eq!(ctx.selected_file, Some(PathBuf::from("b.md")));
    assert!(ctx.selected_files.contains(&PathBuf::from("b.md")));
    assert_eq!(
        ctx.selected_files.len(),
        1,
        "previous selection must be cleared"
    );
    assert_eq!(
        ctx.tabs,
        vec![PathBuf::from("a.md"), PathBuf::from("b.md")],
        "clicked file must be pushed onto tabs"
    );
}

/// Tier 1 test: a file row click with shift held toggles the
/// row's membership in `selected_files`. Toggling an
/// already-selected file removes it and clears `selected_file`
/// if it pointed at that file.
#[test]
fn test_apply_file_row_click_shift_toggles_off() {
    let row = FlatRow {
        depth: 0,
        name: "b.md".to_string(),
        path: PathBuf::from("b.md"),
        is_dir: false,
        is_expanded: false,
    };
    let mut ctx = TreeNodeContext {
        selected_file: Some(PathBuf::from("b.md")),
        selected_files: HashSet::from([PathBuf::from("b.md")]),
        modifiers: egui::Modifiers {
            shift: true,
            ..Default::default()
        },
        ..Default::default()
    };

    apply_file_row_click(&mut ctx, &row);

    assert!(
        !ctx.selected_files.contains(&PathBuf::from("b.md")),
        "shift-click on a selected file must remove it from selected_files"
    );
    assert!(
        ctx.selected_file.is_none(),
        "selected_file must be cleared when the toggled-off file was the selected one"
    );
}

/// Tier 1 test: shift-clicking a file that is NOT in
/// `selected_files` adds it to the set and makes it the
/// `selected_file` without touching `tabs` (multi-select does
/// not auto-open tabs).
#[test]
fn test_apply_file_row_click_shift_adds_to_selection_without_opening_tab() {
    let row = FlatRow {
        depth: 0,
        name: "b.md".to_string(),
        path: PathBuf::from("b.md"),
        is_dir: false,
        is_expanded: false,
    };
    let mut ctx = TreeNodeContext {
        selected_file: Some(PathBuf::from("a.md")),
        selected_files: HashSet::from([PathBuf::from("a.md")]),
        modifiers: egui::Modifiers {
            shift: true,
            ..Default::default()
        },
        ..Default::default()
    };

    apply_file_row_click(&mut ctx, &row);

    assert!(ctx.selected_files.contains(&PathBuf::from("b.md")));
    assert_eq!(
        ctx.selected_file,
        Some(PathBuf::from("b.md")),
        "shift-click must set selected_file to the clicked file"
    );
    assert!(
        ctx.tabs.is_empty(),
        "shift-click must NOT auto-open the clicked file as a tab (multi-select mode)"
    );
}

/// Tier 1 test: clicking a file that is already open in a tab
/// does NOT push a duplicate. Tab list is the unique set of
/// open paths.
#[test]
fn test_apply_file_row_click_no_duplicate_tab() {
    let row = FlatRow {
        depth: 0,
        name: "a.md".to_string(),
        path: PathBuf::from("a.md"),
        is_dir: false,
        is_expanded: false,
    };
    let mut ctx = TreeNodeContext {
        selected_file: Some(PathBuf::from("a.md")),
        selected_files: HashSet::from([PathBuf::from("a.md")]),
        tabs: vec![PathBuf::from("a.md")],
        ..Default::default()
    };

    apply_file_row_click(&mut ctx, &row);

    assert_eq!(
        ctx.tabs,
        vec![PathBuf::from("a.md")],
        "clicking an already-open tab must not push a duplicate"
    );
}

/// TDD regression: clicking a file row in the left directory
/// tree must update `selected_dir` (the "current directory
/// context" used by the bottom-panel prompt prefix and the
/// agent session) to the file's containing directory.
///
/// Before the fix, `apply_file_row_click` only updated
/// `selected_file` / `selected_files` / `tabs` — `selected_dir`
/// kept whatever value the previous directory click (or app
/// start) had set, so the bottom panel would keep showing a
/// stale directory prefix and the agent would receive the
/// wrong context once the user opened a file.
#[test]
fn test_apply_file_row_click_updates_selected_dir_to_parent() {
    let file_path = PathBuf::from("C:/notes/folder/file.md");
    let expected_parent = Some(PathBuf::from("C:/notes/folder"));
    let row = FlatRow {
        depth: 1,
        name: "file.md".to_string(),
        path: file_path,
        is_dir: false,
        is_expanded: false,
    };
    // Pre-existing stale value to prove the click overwrites it.
    let mut ctx = TreeNodeContext {
        selected_dir: Some(PathBuf::from("C:/old/dir")),
        ..Default::default()
    };

    apply_file_row_click(&mut ctx, &row);

    assert_eq!(
        ctx.selected_dir, expected_parent,
        "clicking a file row must update selected_dir to the file's containing directory"
    );
}

/// TDD regression: even with a multi-select modifier (shift),
/// clicking a file row must still update `selected_dir` to
/// the file's containing directory. The user is operating
/// in that directory and the bottom-panel prefix / agent
/// context should reflect it.
#[test]
fn test_apply_file_row_click_shift_updates_selected_dir_to_parent() {
    let file_path = PathBuf::from("C:/notes/folder/file.md");
    let expected_parent = Some(PathBuf::from("C:/notes/folder"));
    let row = FlatRow {
        depth: 1,
        name: "file.md".to_string(),
        path: file_path,
        is_dir: false,
        is_expanded: false,
    };
    let mut ctx = TreeNodeContext {
        modifiers: egui::Modifiers {
            shift: true,
            ..Default::default()
        },
        ..Default::default()
    };

    apply_file_row_click(&mut ctx, &row);

    assert_eq!(
        ctx.selected_dir, expected_parent,
        "shift-clicking a file row must also update selected_dir to the file's containing directory"
    );
}

/// Edge case: a file with no parent component (a bare
/// filename like `file.md`) must refresh `selected_dir` away
/// from any stale prior value. `Path::parent("file.md")`
/// returns `Some(Path::new(""))` (an empty path), not `None`,
/// because the OS-level "containing directory" of a bare
/// filename is the empty path. The downstream
/// `compute_prompt_prefix` already handles this case — an
/// empty path falls through to its `is_empty()` branch and
/// renders the bare `">"` prefix, matching the `None` case.
#[test]
fn test_apply_file_row_click_bare_filename_sets_empty_parent() {
    let row = FlatRow {
        depth: 0,
        name: "file.md".to_string(),
        path: PathBuf::from("file.md"),
        is_dir: false,
        is_expanded: false,
    };
    let mut ctx = TreeNodeContext {
        selected_dir: Some(PathBuf::from("C:/stale/dir")),
        ..Default::default()
    };

    apply_file_row_click(&mut ctx, &row);

    // `Path::parent("file.md")` is `Some(Path::new(""))`,
    // not `None`. Verify the click refreshes the stale
    // value to that canonical empty-parent form, and that
    // the resulting bottom-panel prefix renders as the bare
    // ">" (same surface as `selected_dir == None`).
    assert_eq!(
        ctx.selected_dir,
        Some(PathBuf::new()),
        "clicking a bare-filename row must set selected_dir to Some(Path::new(\"\"))"
    );
    let prefix = crate::ui::panels::bottom::compute_prompt_prefix(ctx.selected_dir.as_deref(), &[]);
    assert_eq!(
        prefix, ">",
        "an empty-path selected_dir must render as the bare `>` prefix in the bottom panel"
    );
}

/// TDD regression: clicking a directory row in the left
/// directory tree must NOT clear the currently selected file
/// or the multi-selection set.
///
/// **Why this matters.** `render_tabs_and_content` in the
/// center panel guards its body on
/// `if let Some(selected_path) = app.selection().selected_file()`.
/// If a directory click cleared `selected_file`, the body —
/// the file's header, the YAML front-matter table, and the
/// rendered markdown inside its `ScrollArea` — would be
/// skipped on the next frame. The tab strip would still be
/// visible, but the preview area would go blank. The right
/// (TOC) panel would also disappear, because
/// `should_show_panel(has_toc, has_selected_file)` requires
/// a selected file. The user would have to click the file
/// again to restore the preview, even though
/// `tabs.current_markdown` / `current_yaml` /
/// `loaded_path` were never touched.
///
/// **The bug.** The directory-click branch in
/// `render_flat_row` and `draw_tree_node` (legacy) used to
/// unconditionally run `*ctx.selected_file() = None` and
/// `ctx.selected_files().clear()`, conflating "expand this
/// folder" with "deselect the open file." The two helpers
/// now route through `apply_directory_row_click`, which
/// only toggles `expanded_dirs` and refreshes `selected_dir`.
///
/// **The contract pinned by this test.** After
/// `apply_directory_row_click`:
///   * `selected_file` is unchanged.
///   * `selected_file` is cleared.
///   * `selected_files` is cleared.
///   * `tabs` is unchanged.
///   * `expanded_dirs` is toggled for `row.path`.
///   * `selected_dir` is set to `Some(row.path.clone())`
///     (the "current directory context" used by the
///     bottom-panel prompt prefix and the agent session).
#[test]
fn test_apply_directory_row_click_clears_selected_file() {
    let dir_path = PathBuf::from("C:/notes/folder");
    let row = FlatRow {
        depth: 0,
        name: "folder".to_string(),
        path: dir_path.clone(),
        is_dir: true,
        is_expanded: false,
    };
    // Pre-existing stale value to prove the click overwrites it
    // (mirrors the `apply_file_row_click` `selected_dir` test).
    let mut ctx = TreeNodeContext {
        selected_file: Some(PathBuf::from("doc.md")),
        selected_files: HashSet::from([PathBuf::from("doc.md")]),
        tabs: vec![PathBuf::from("doc.md")],
        selected_dir: Some(PathBuf::from("C:/old/dir")),
        ..Default::default()
    };

    apply_directory_row_click(&mut ctx, &row);

    // The contract: file selection is cleared when navigating directories.
    assert!(
        ctx.selected_file.is_none(),
        "directory row click must clear selected_file"
    );
    assert!(
        ctx.selected_files.is_empty(),
        "directory row click must clear selected_files"
    );
    assert_eq!(
        ctx.tabs,
        vec![PathBuf::from("doc.md")],
        "directory row click must NOT touch the open tabs"
    );
    // The actual purpose: expand the folder and refresh the
    // current-directory context.
    assert!(
        ctx.expanded_dirs.contains(&dir_path),
        "directory row click must add the folder to expanded_dirs"
    );
    assert_eq!(
        ctx.selected_dir,
        Some(dir_path),
        "directory row click must update selected_dir to the folder's path"
    );
}

/// TDD regression (companion to
/// `test_apply_directory_row_click_clears_selected_file`):
/// the second click on an already-expanded directory must
/// collapse it. The same invariant holds — the file
/// selection is cleared.
///
/// This is a separate test rather than a follow-up call in
/// the previous test, because the borrow checker treats two
/// sequential `&mut ctx` calls as overlapping re-borrows of
/// `ctx`'s inner fields; splitting the test lets each
/// assertion set live independently of the next call.
#[test]
fn test_apply_directory_row_click_collapses_expanded_folder_clears_selection() {
    let dir_path = PathBuf::from("C:/notes/folder");
    let row = FlatRow {
        depth: 0,
        name: "folder".to_string(),
        path: dir_path.clone(),
        is_dir: true,
        is_expanded: true,
    };
    let mut ctx = TreeNodeContext {
        selected_file: Some(PathBuf::from("doc.md")),
        selected_files: HashSet::from([PathBuf::from("doc.md")]),
        tabs: vec![PathBuf::from("doc.md")],
        expanded_dirs: HashSet::from([dir_path.clone()]),
        selected_dir: Some(dir_path.clone()),
        ..Default::default()
    };

    apply_directory_row_click(&mut ctx, &row);

    // Collapse: the folder is removed from `expanded_dirs`.
    assert!(
        !ctx.expanded_dirs.contains(&dir_path),
        "clicking an already-expanded directory must collapse it"
    );
    // File selection is cleared when navigating directories.
    assert!(
        ctx.selected_file.is_none(),
        "collapsing a directory must clear selected_file"
    );
    assert!(
        ctx.selected_files.is_empty(),
        "collapsing a directory must clear selected_files"
    );
    assert_eq!(
        ctx.tabs,
        vec![PathBuf::from("doc.md")],
        "collapsing a directory must NOT touch the open tabs"
    );
    // `selected_dir` is refreshed to the directory's path
    // regardless of whether the click expanded or collapsed it.
    assert_eq!(
        ctx.selected_dir,
        Some(dir_path),
        "collapsing a directory must still update selected_dir to its path"
    );
}

/// TDD regression (companion to the
/// `test_directory_click_invalidates_tree_cache` integration test
/// in `ui/panels/left.rs`): the directory-row click handler must
/// set the tree-rows cache invalidation flag (`tree_dirty`) so
/// the next `show_left_panel` pass rebuilds the flat row list.
/// Without this, the P0 perf-optimization cache keeps returning
/// the *previous* flat rows and the click looks like a no-op.
///
/// This unit test pins the contract at the handler level; the
/// integration test in `ui/panels/left.rs` pins the
/// user-visible outcome.
#[test]
fn test_apply_directory_row_click_marks_tree_dirty() {
    let dir_path = PathBuf::from("C:/notes/folder");
    let row = FlatRow {
        depth: 0,
        name: "folder".to_string(),
        path: dir_path.clone(),
        is_dir: true,
        is_expanded: false,
    };
    let mut ctx = TreeNodeContext::default();

    // Expanding a directory (the path is not in `expanded_dirs`).
    apply_directory_row_click(&mut ctx, &row);
    assert!(
        *ctx.tree_dirty(),
        "expanding a directory click must mark the tree cache dirty so the next render rebuilds the flat rows"
    );

    // The second click collapses it. The cache must still be
    // invalidated because the visible rows change either way.
    *ctx.tree_dirty() = false;
    let row_expanded = FlatRow {
        depth: 0,
        name: "folder".to_string(),
        path: dir_path,
        is_dir: true,
        is_expanded: true,
    };
    apply_directory_row_click(&mut ctx, &row_expanded);
    assert!(
        *ctx.tree_dirty(),
        "collapsing a directory click must also mark the tree cache dirty"
    );
}

/// TDD regression (companion to
/// `test_apply_directory_row_click_marks_tree_dirty`): the
/// file-row click handler must NOT mark the tree cache dirty.
/// File clicks only change which file is selected; the visible
/// rows are unchanged, so the P0 cache stays valid. Marking
/// the cache dirty here would force an unnecessary
/// `flatten_tree` rebuild on every file click, defeating the
/// perf optimization.
#[test]
fn test_apply_file_row_click_does_not_mark_tree_dirty() {
    let row = FlatRow {
        depth: 0,
        name: "b.md".to_string(),
        path: PathBuf::from("b.md"),
        is_dir: false,
        is_expanded: false,
    };
    let mut ctx = TreeNodeContext::default();

    // No-modifier click: opens the file in a tab. The cache
    // must NOT be invalidated.
    apply_file_row_click(&mut ctx, &row);
    assert!(
        !*ctx.tree_dirty(),
        "a no-modifier file click must NOT mark the tree cache dirty (visible rows are unchanged)"
    );

    // Shift-click: toggles the multi-select. The cache must
    // still NOT be invalidated.
    *ctx.tree_dirty() = false;
    let mut shift_ctx = TreeNodeContext {
        modifiers: egui::Modifiers {
            shift: true,
            ..Default::default()
        },
        ..Default::default()
    };
    apply_file_row_click(&mut shift_ctx, &row);
    assert!(
        !*shift_ctx.tree_dirty(),
        "a shift-click file row click must NOT mark the tree cache dirty (visible rows are unchanged)"
    );
}

#[test]
fn test_merge_prompt_includes_consolidate_instruction_and_files() {
    let libs = vec![crate::config::ContentLibrary {
        root_folder: "C:/notes".to_string(),
        name: "Notes".to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 0,
    }];
    let file1 = PathBuf::from("C:/notes/alpha.md");
    let file2 = PathBuf::from("C:/notes/beta.md");

    let mut selected_files = HashSet::new();
    selected_files.insert(file1.clone());
    selected_files.insert(file2.clone());

    let prompt = build_merge_prompt(&libs, &selected_files);

    assert!(
        prompt.to_lowercase().contains("merge"),
        "prompt should instruct merge: {}",
        prompt
    );
    assert!(
        prompt.to_lowercase().contains("consolidate"),
        "prompt should instruct consolidate: {}",
        prompt
    );
    assert!(prompt.contains("alpha.md"), "prompt should list alpha.md");
    assert!(prompt.contains("beta.md"), "prompt should list beta.md");
}

/// T-02: End-to-end test for the multi-select "Merge" operation.
///
/// When the user selects multiple files in the tree and chooses "Merge",
/// `build_merge_prompt` generates the user prompt listing the selected files,
/// and `build_system_prompts` injects `"selected the following files"` into the
/// dynamic system context.
///
/// Proves that both the merge prompt text *and* the selected-files context
/// reach the LLM via `POST /chat/completions`.
#[test]
fn test_e2e_openai_wiremock_multi_select_merge_prompt_and_context_sent_to_llm() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("build tokio runtime");

    let mock_server = runtime.block_on(MockServer::start());

    let response_body = serde_json::json!({
        "id": "chatcmpl-merge",
        "object": "chat.completion",
        "created": 1677652288,
        "model": "gpt-4o",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "I have merged the selected notes."
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": 50,
            "completion_tokens": 10,
            "total_tokens": 60
        }
    });

    runtime.block_on(
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", "application/json")
                    .set_body_json(response_body),
            )
            .mount(&mock_server),
    );

    let tmp = tempfile::tempdir().unwrap();
    let notes_dir = tmp.path().join("Notes");
    std::fs::create_dir_all(&notes_dir).unwrap();
    let file1 = notes_dir.join("meeting-2026-01.md");
    let file2 = notes_dir.join("meeting-2026-02.md");
    std::fs::write(&file1, "# Meeting Jan 2026").unwrap();
    std::fs::write(&file2, "# Meeting Feb 2026").unwrap();

    let libs = vec![crate::config::ContentLibrary {
        root_folder: notes_dir.to_string_lossy().to_string(),
        name: "Notes".to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 0,
    }];

    // Simulate: user ctrl-clicks two files → right-click → "Merge"
    // render.rs does:
    //   let prompt = build_merge_prompt(ctx.content_libraries(), &files);
    //   *ctx.submit_prompt() = Some(prompt);
    // The orchestrator's start_agent_session gets selected_files via selection.agent_context().
    let mut selected_files = HashSet::new();
    selected_files.insert(file1.clone());
    selected_files.insert(file2.clone());

    let user_prompt = build_merge_prompt(&libs, &selected_files);

    let config = crate::config::AppConfig {
        content_libraries: libs,
        ..crate::config::AppConfig::default()
    };

    // Build system prompts with selected_files — this injects the "selected the following files" line
    let system_prompts = crate::agent::prompts::build_system_prompts(
        &config,
        None, // no single active file — multi-select has no primary file
        None,
        &selected_files,
    );

    // Pre-check: selected files are in the dynamic prompt
    assert!(system_prompts[1].contains("selected the following files"));
    assert!(system_prompts[1].contains("meeting-2026-01.md"));
    assert!(system_prompts[1].contains("meeting-2026-02.md"));

    // Pre-check: merge prompt lists both files
    assert!(user_prompt.contains("merge"));
    assert!(user_prompt.contains("meeting-2026-01.md"));
    assert!(user_prompt.contains("meeting-2026-02.md"));

    let mut models = std::collections::HashMap::new();
    models.insert(
        "default".to_string(),
        fastmd_agent::config::LlmConfig {
            model: "gpt-4o".to_string(),
            api_url: mock_server.uri(),
            api_key: "test-openai-key".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        },
    );

    let agent_config = fastmd_agent::config::AgentConfigBuilder::new()
        .with_models(models)
        .build();

    let session_id = uuid::Uuid::new_v4();
    let observer = std::sync::Arc::new(fastmd_agent::events::RecordingObserver::new());
    let ctx =
        fastmd_agent::context::AgentContextBuilder::new(agent_config, session_id, user_prompt)
            .with_system_prompts(system_prompts)
            .with_observer(observer.clone())
            .build();

    let handle = std::thread::spawn(move || {
        fastmd_agent::run_agent(ctx);
    });
    handle.join().unwrap();

    let received_requests = runtime
        .block_on(mock_server.received_requests())
        .expect("must record requests");
    assert_eq!(
        received_requests.len(),
        1,
        "Expected exactly 1 request to OpenAI mock server"
    );

    let payload: serde_json::Value = serde_json::from_slice(&received_requests[0].body)
        .expect("request body must be valid JSON");
    let messages = payload["messages"]
        .as_array()
        .expect("messages array must be present");

    // 1. System message must contain the selected-files context
    let system_with_files = messages
        .iter()
        .find(|m| {
            m["role"] == "system"
                && m["content"]
                    .as_str()
                    .map(|c| {
                        c.contains("selected the following files")
                            && c.contains("meeting-2026-01.md")
                            && c.contains("meeting-2026-02.md")
                    })
                    .unwrap_or(false)
        })
        .expect("Must find system message listing all selected files");

    assert!(
        system_with_files["content"]
            .as_str()
            .unwrap()
            .contains("selected the following files"),
        "System message must say 'selected the following files'"
    );

    // 2. User message must contain the merge instruction and both file names
    let user_msg = messages
        .iter()
        .find(|m| m["role"] == "user")
        .expect("Must find a user message");

    let user_content = user_msg["content"].as_str().unwrap();
    assert!(
        user_content.to_lowercase().contains("merge"),
        "User message must contain merge instruction"
    );
    assert!(
        user_content.contains("meeting-2026-01.md"),
        "User message must list first selected file"
    );
    assert!(
        user_content.contains("meeting-2026-02.md"),
        "User message must list second selected file"
    );
}
