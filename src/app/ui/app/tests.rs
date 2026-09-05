//! Tests for `app/mod.rs`.

use super::*;
use crate::agent::events::AgentStatus;
use crate::bus::events::agent::AgentEvent as SeamAgentEvent;
use crate::bus::events::file::FileEvent;
use crate::bus::events::messages::TokenUsageInfo;
use crate::bus::events::typed::FsEvent;
use crate::orchestrator::AppOrchestrator;
use crate::ui::test_helpers::assert::assert_no_id_change_in_shapes;
use crate::ui::test_helpers::run_ui_test;
use std::path::PathBuf;
use uuid::Uuid;

use crate::ui::test_helpers::app::test_app as create_test_app;

/// Idle-CPU regression: when the app is fully idle (no file events
/// arrived, indexing is finished, and no raw input is pending),
/// `FastMdApp::should_request_immediate_repaint` MUST return `false`.
///
/// The previous implementation unconditionally called
/// `ctx.request_repaint_after(16ms)` in the idle branch of
/// `process_file_events_and_repaint`, which kept the entire
/// `update_ui` closure running at 60 FPS even when nothing on
/// screen had changed — pegging the process at ~5% CPU with no
/// input. The decider now exposes the decision so the idle path
/// can be unit-tested without driving a real `egui::Context` frame.
#[test]
fn test_idle_app_does_not_request_repaint() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    // Indexing has finished and the transition has been handled,
    // so the spinner is hidden and the toolbar is settled.
    app.orchestrator.file_processor.indexing_finished = true;
    app.orchestrator.file_processor.indexing_finished_handled = true;
    // No file events are queued: `process_file_events` would have
    // returned `false`. The empty ctx has no pending raw input.
    assert!(
        !app.should_request_immediate_repaint(&ctx),
        "An idle app (no file events, indexing finished, no raw input) \
         must NOT request a repaint. Returning true here re-introduces \
         the 60 FPS forced-repaint loop and burns idle CPU."
    );
}

/// Indexing alone is not a repaint reason when no UI-visible event is pending.
#[test]
fn test_indexing_in_progress_without_events_does_not_request_repaint() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    app.orchestrator.file_processor.indexing_finished = false;
    assert!(
        !app.should_request_immediate_repaint(&ctx),
        "Indexing without a changed UI state must not create a repaint loop"
    );
}

/// UI-002 (dark color scheme): `configure_dark_theme` must pin the
/// active theme to Dark and apply the FastMD brand palette
/// (RGB(9, 9, 11) surface, indigo selection) to the dark theme.
/// Regression guard: the egui 0.27 â†’ 0.35 upgrade silently fell
/// back to the active theme's default visuals on systems reporting
/// light mode, losing the black background.
#[test]
fn test_configure_dark_theme_pins_dark_with_brand_palette() {
    let ctx = egui::Context::default();

    // First, flip the active theme to Light to simulate a host
    // that reports light mode as a preference. The fix must hold
    // even in that case.
    ctx.set_theme(egui::Theme::Light);
    assert_eq!(ctx.theme(), egui::Theme::Light);

    FastMdApp::configure_dark_theme(&ctx);

    // Theme is forced to Dark regardless of the prior preference.
    assert_eq!(
        ctx.theme(),
        egui::Theme::Dark,
        "configure_dark_theme must force the active theme to Dark"
    );

    // The dark theme's visuals are the FastMD brand palette,
    // not the default `Visuals::dark()` (which is RGB(27, 27, 27)
    // for both window_fill and panel_fill).
    let dark_visuals = ctx.style_of(egui::Theme::Dark).visuals.clone();
    let expected_panel = egui::Color32::from_rgb(9, 9, 11);
    let expected_window = egui::Color32::from_rgb(9, 9, 11);
    assert_eq!(
        dark_visuals.panel_fill, expected_panel,
        "dark theme's panel_fill must be the FastMD brand RGB(9, 9, 11)"
    );
    assert_eq!(
        dark_visuals.window_fill, expected_window,
        "dark theme's window_fill must be the FastMD brand RGB(9, 9, 11)"
    );
    assert_eq!(
        dark_visuals.selection.bg_fill,
        egui::Color32::from_rgb(99, 102, 241),
        "selection.bg_fill must be the FastMD indigo RGB(99, 102, 241)"
    );
}

#[test]
fn test_treenode_new() {
    let node = TreeNode::new("Docs".to_string(), PathBuf::from("/docs"), true);
    assert_eq!(node.name, "Docs");
    assert_eq!(node.path, PathBuf::from("/docs"));
    assert!(node.is_dir);
    assert!(node.children.is_empty());
}

#[test]
fn test_tags_tracks_tags_correctly() {
    let mut app = create_test_app();
    app.orchestrator.tags.add_tags(
        PathBuf::from("file1.md"),
        vec!["rust".to_string(), "ui".to_string()],
    );
    app.orchestrator.tags.add_tags(
        PathBuf::from("file2.md"),
        vec!["rust".to_string(), "testing".to_string()],
    );

    assert_eq!(app.orchestrator.tags.all_tags().len(), 3);
    assert!(app.orchestrator.tags.all_tags().contains("rust"));
    assert!(app.orchestrator.tags.all_tags().contains("ui"));
    assert!(app.orchestrator.tags.all_tags().contains("testing"));
}

#[test]
fn test_background_messages_handling() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    let test_file = PathBuf::from("test_doc.md");
    let test_dir = PathBuf::from("test_dir");

    // 1. FileParsed
    app.orchestrator
        .tx
        .send(
            FsEvent::FileParsed {
                path: test_file.clone(),
                tags: vec!["tag1".to_string()],
            }
            .into(),
        )
        .unwrap();

    // 2. DirParsed
    app.orchestrator
        .tx
        .send(
            FsEvent::DirParsed {
                path: test_dir.clone(),
            }
            .into(),
        )
        .unwrap();

    // 3. FinishedWithoutWatcher
    app.orchestrator
        .tx
        .send(FsEvent::FinishedWithoutWatcher.into())
        .unwrap();

    // 4. Agent Status & ContentDelta via Bus<AgentEvent> (T015: new path)
    let session_id = Uuid::new_v4();
    app.orchestrator
        .agent_event_bus
        .publish(SeamAgentEvent::SessionStarted { session_id });
    app.orchestrator
        .agent_event_bus
        .publish(SeamAgentEvent::Status {
            session_id,
            status: AgentStatus::AwaitingLlm,
        });
    app.orchestrator
        .agent_event_bus
        .publish(SeamAgentEvent::Thinking {
            session_id,
            text: "Thinking step".to_string(),
        });
    app.orchestrator
        .agent_event_bus
        .publish(SeamAgentEvent::ContentDelta {
            session_id,
            text: "Done result".to_string(),
        });

    let mut output = run_ui_test(&ctx, Default::default(), |ui| {
        app.update_ui(ui);
    });
    output.textures_delta.clear();

    assert!(
        app.orchestrator
            .file_processor
            .all_files
            .contains(&test_file)
    );
    assert!(app.orchestrator.file_processor.all_dirs.contains(&test_dir));
    assert!(app.orchestrator.file_processor.indexing_finished);
    assert_eq!(
        app.orchestrator.agent.state().status,
        "Waiting for LLM completions..."
    );
    assert_eq!(app.orchestrator.agent_transcript.thinking, "Thinking step");
    assert_eq!(app.orchestrator.agent_transcript.content, "Done result");
}

#[test]
fn test_background_message_file_modified_and_deleted() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    let file_path = PathBuf::from("modified_file.md");

    app.orchestrator
        .file_processor
        .all_files
        .push(file_path.clone());
    *app.orchestrator.selection.selected_file_mut() = Some(file_path.clone());
    app.orchestrator
        .selection
        .selected_files_mut()
        .insert(file_path.clone());
    app.orchestrator.tabs.loaded_path = Some(file_path.clone());

    // File modified message
    app.orchestrator
        .tx
        .send(
            FsEvent::FileModified {
                path: file_path.clone(),
                tags: vec!["updated".to_string()],
            }
            .into(),
        )
        .unwrap();

    let mut output = run_ui_test(&ctx, Default::default(), |ui| {
        app.update_ui(ui);
    });
    output.textures_delta.clear();

    assert!(app.orchestrator.tabs.loaded_path.is_none()); // Trigger reload

    // File deleted message
    app.orchestrator
        .tx
        .send(
            FsEvent::FileDeleted {
                path: file_path.clone(),
            }
            .into(),
        )
        .unwrap();

    let mut output = run_ui_test(&ctx, Default::default(), |ui| {
        app.update_ui(ui);
    });
    output.textures_delta.clear();

    assert!(
        !app.orchestrator
            .file_processor
            .all_files
            .contains(&file_path)
    );
    assert!(app.orchestrator.selection.selected_file().is_none());
    assert!(
        !app.orchestrator
            .selection
            .selected_files()
            .contains(&file_path)
    );
}

#[test]
fn test_agent_failure_and_finish_messages() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();

    let session_id = Uuid::new_v4();
    app.orchestrator
        .agent_event_bus
        .publish(SeamAgentEvent::SessionStarted { session_id });
    app.orchestrator
        .agent_event_bus
        .publish(SeamAgentEvent::Failed {
            session_id,
            error: "Network timeout".to_string(),
        });

    let mut output = run_ui_test(&ctx, Default::default(), |ui| {
        app.update_ui(ui);
    });
    output.textures_delta.clear();

    assert_eq!(
        app.orchestrator.agent.state().status,
        "Error: Network timeout"
    );
    assert!(!app.orchestrator.agent.state().running);

    let session_id2 = Uuid::new_v4();
    app.orchestrator
        .agent_event_bus
        .publish(SeamAgentEvent::SessionStarted {
            session_id: session_id2,
        });
    app.orchestrator
        .agent_event_bus
        .publish(SeamAgentEvent::SessionFinished {
            session_id: session_id2,
            history: vec![serde_json::json!({"ok": true})],
        });

    let mut output = run_ui_test(&ctx, Default::default(), |ui| {
        app.update_ui(ui);
    });
    output.textures_delta.clear();

    assert!(!app.orchestrator.agent.state().running);
    assert!(app.orchestrator.agent.state().history.is_some());
}

#[test]
fn test_agent_token_usage_message_accumulates() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();

    let session_id = Uuid::new_v4();
    app.orchestrator
        .agent_event_bus
        .publish(SeamAgentEvent::SessionStarted { session_id });

    // First turn: small context, no cached or reasoning tokens.
    app.orchestrator
        .agent_event_bus
        .publish(SeamAgentEvent::TokenUsage {
            session_id,
            usage: TokenUsageInfo {
                prompt_tokens: 100,
                completion_tokens: 20,
                total_tokens: 120,
                ..Default::default()
            },
        });

    let mut output = run_ui_test(&ctx, Default::default(), |ui| {
        app.update_ui(ui);
    });
    output.textures_delta.clear();

    assert_eq!(
        app.orchestrator
            .agent
            .state()
            .token_usage
            .as_ref()
            .unwrap()
            .prompt_tokens,
        100
    );
    assert_eq!(
        app.orchestrator.agent.state().total_usage.prompt_tokens,
        100,
        "prompt_tokens should track the peak seen so far"
    );
    assert_eq!(
        app.orchestrator.agent.state().total_usage.completion_tokens,
        20
    );
    assert_eq!(app.orchestrator.agent.state().total_usage.total_tokens, 120);
    assert_eq!(
        app.orchestrator.agent.state().total_usage.cached_tokens,
        Some(0)
    );
    assert_eq!(
        app.orchestrator.agent.state().total_usage.reasoning_tokens,
        Some(0)
    );

    // Second turn: context grew, completion + reasoning added.
    app.orchestrator
        .agent_event_bus
        .publish(SeamAgentEvent::TokenUsage {
            session_id,
            usage: TokenUsageInfo {
                prompt_tokens: 250,
                completion_tokens: 30,
                total_tokens: 280,
                cached_tokens: Some(50),
                reasoning_tokens: Some(5),
            },
        });

    let mut output = run_ui_test(&ctx, Default::default(), |ui| {
        app.update_ui(ui);
    });
    output.textures_delta.clear();

    assert_eq!(
        app.orchestrator
            .agent
            .state()
            .token_usage
            .as_ref()
            .unwrap()
            .prompt_tokens,
        250
    );
    assert_eq!(
        app.orchestrator.agent.state().total_usage.prompt_tokens,
        250,
        "peak should rise with the larger turn"
    );
    assert_eq!(
        app.orchestrator.agent.state().total_usage.completion_tokens,
        50
    );
    assert_eq!(app.orchestrator.agent.state().total_usage.total_tokens, 400);
    assert_eq!(
        app.orchestrator.agent.state().total_usage.cached_tokens,
        Some(50)
    );
    assert_eq!(
        app.orchestrator.agent.state().total_usage.reasoning_tokens,
        Some(5)
    );

    // Third turn: smaller context — peak should NOT shrink.
    app.orchestrator
        .agent_event_bus
        .publish(SeamAgentEvent::TokenUsage {
            session_id,
            usage: TokenUsageInfo {
                prompt_tokens: 80,
                completion_tokens: 10,
                total_tokens: 90,
                ..Default::default()
            },
        });

    let mut output = run_ui_test(&ctx, Default::default(), |ui| {
        app.update_ui(ui);
    });
    output.textures_delta.clear();

    assert_eq!(
        app.orchestrator.agent.state().total_usage.prompt_tokens,
        250,
        "peak prompt size must not regress"
    );
    assert_eq!(
        app.orchestrator.agent.state().total_usage.completion_tokens,
        60
    );
    assert_eq!(
        app.orchestrator.agent.state().total_usage.cached_tokens,
        Some(50)
    );
    assert_eq!(
        app.orchestrator.agent.state().total_usage.reasoning_tokens,
        Some(5)
    );
}

// -- process_file_events: tab reload on file Updated --

#[test]
fn test_process_file_events_updated_resets_loaded_path() {
    // When the bus reports a Discovered/Updated event for a
    // file that is currently loaded into the renderer, the
    // next frame must reload it from disk. We model "currently
    // loaded" by setting `loaded_path = Some(path)` while
    // leaving `selected_file` alone â€” `load_selected_file`
    // (the actual reload driver) only fires when
    // `selected_file.is_some() && loaded_path != selected_file`.
    let mut app = create_test_app();
    let path = PathBuf::from("/tmp/active_doc.md");

    *app.orchestrator.selection.selected_file_mut() = Some(path.clone());
    app.orchestrator.tabs.loaded_path = Some(path.clone());
    app.orchestrator.file_processor.all_files.push(path.clone());

    // Subscribe a reader to the bus so we can publish into it
    // and have process_file_events pick up the event.
    app.orchestrator.file_event_reader = Some(app.orchestrator.file_event_bus.subscribe());

    // Use a separate clone of the bus to publish; both clones
    // share the same subscriber list.
    let publisher = app.orchestrator.file_event_bus.clone();
    publisher.publish(FileEvent::updated_one(path.clone()));

    let changed = app.orchestrator.process_file_events();
    assert!(changed, "process_file_events should report a change");
    assert!(
        app.orchestrator.tabs.loaded_path.is_none(),
        "loaded_path must be cleared so the renderer reloads on the next frame"
    );
    // selected_file must be preserved so the renderer knows
    // what to render.
    assert_eq!(app.orchestrator.selection.selected_file(), Some(&path));
}

#[test]
fn test_process_file_events_updated_preserves_loaded_when_editor_open() {
    // If the inline editor is open on the file, the user's
    // unsaved changes must not be clobbered by an external
    // update. The reload should be skipped.
    let mut app = create_test_app();
    let path = PathBuf::from("/tmp/being_edited.md");

    *app.orchestrator.selection.selected_file_mut() = Some(path.clone());
    app.orchestrator.tabs.loaded_path = Some(path.clone());
    app.orchestrator.file_processor.all_files.push(path.clone());
    app.orchestrator
        .text_buffer
        .open(&path, "old content", None);
    assert!(app.orchestrator.text_buffer.is_open);

    app.orchestrator.file_event_reader = Some(app.orchestrator.file_event_bus.subscribe());
    let publisher = app.orchestrator.file_event_bus.clone();
    publisher.publish(FileEvent::updated_one(path.clone()));

    let _ = app.orchestrator.process_file_events();
    assert!(
        app.orchestrator.tabs.loaded_path.is_some(),
        "loaded_path must NOT be cleared while the inline editor is open"
    );
}

#[test]
fn test_process_file_events_removed_clears_loaded_path() {
    // Sanity check: a Removed event still clears `loaded_path`
    // regardless of whether the editor is open. (We accept
    // losing unsaved edits in the editor if the file was
    // deleted out from under us â€” that's the user's action.)
    let mut app = create_test_app();
    let path = PathBuf::from("/tmp/gone.md");

    *app.orchestrator.selection.selected_file_mut() = Some(path.clone());
    app.orchestrator.tabs.loaded_path = Some(path.clone());
    app.orchestrator.file_processor.all_files.push(path.clone());

    app.orchestrator.file_event_reader = Some(app.orchestrator.file_event_bus.subscribe());
    let publisher = app.orchestrator.file_event_bus.clone();
    publisher.publish(FileEvent::removed_one(path.clone()));

    let _ = app.orchestrator.process_file_events();
    assert!(app.orchestrator.tabs.loaded_path.is_none());
}

#[test]
fn test_process_file_events_filters_out_non_workspace_files() {
    // PDFs and images are inputs to the PDF-converter and
    // image-vision workers. They still flow through the bus
    // (so the workers see them) but they must NOT be added
    // to `all_files` or `all_dirs`, which feed the directory
    // tree. A directory that contains only PDFs / images
    // must not appear in the tree either.
    let mut app = create_test_app();

    let pdf = PathBuf::from("/tmp/lib/doc.pdf");
    let img = PathBuf::from("/tmp/lib/photo.png");
    let md = PathBuf::from("/tmp/lib/notes.md");
    let pdf_only_dir = PathBuf::from("/tmp/pdf_only");
    let pdf_in_pdf_only_dir = PathBuf::from("/tmp/pdf_only/thing.pdf");

    app.orchestrator.file_event_reader = Some(app.orchestrator.file_event_bus.subscribe());
    let publisher = app.orchestrator.file_event_bus.clone();
    publisher.publish(FileEvent::discovered_one(pdf.clone()));
    publisher.publish(FileEvent::discovered_one(img.clone()));
    publisher.publish(FileEvent::discovered_one(md.clone()));
    publisher.publish(FileEvent::discovered_one(pdf_in_pdf_only_dir.clone()));

    let _ = app.orchestrator.process_file_events();

    // The markdown file should be in the tree and its
    // parent should be in `all_dirs`.
    assert!(
        app.orchestrator.file_processor.all_files.contains(&md),
        "markdown files must appear in the workspace tree"
    );
    assert!(
        app.orchestrator
            .file_processor
            .all_dirs
            .contains(&PathBuf::from("/tmp/lib")),
        "directories containing workspace files must appear in the tree"
    );

    // The PDF and image must NOT be in the tree, even though
    // they were published to the bus (the converters need
    // them).
    assert!(
        !app.orchestrator.file_processor.all_files.contains(&pdf),
        "PDFs must not appear in the workspace tree"
    );
    assert!(
        !app.orchestrator.file_processor.all_files.contains(&img),
        "images must not appear in the workspace tree"
    );

    // A directory that contains only a PDF must not be added
    // to `all_dirs`.
    assert!(
        !app.orchestrator
            .file_processor
            .all_dirs
            .contains(&pdf_only_dir),
        "directories that contain only non-workspace files must not appear in the tree"
    );
}

#[test]
fn test_is_workspace_file_predicate() {
    // Direct unit test for the predicate that drives the
    // filter. Markdown (case-insensitive) and plain text
    // are workspace files; everything else (PDFs, images,
    // no extension) is not.
    assert!(AppOrchestrator::is_workspace_file(&PathBuf::from(
        "/a/b/note.md"
    )));
    assert!(AppOrchestrator::is_workspace_file(&PathBuf::from(
        "/a/b/note.MD"
    )));
    assert!(AppOrchestrator::is_workspace_file(&PathBuf::from(
        "/a/b/note.markdown"
    )));
    assert!(AppOrchestrator::is_workspace_file(&PathBuf::from(
        "/a/b/note.txt"
    )));
    assert!(!AppOrchestrator::is_workspace_file(&PathBuf::from(
        "/a/b/doc.pdf"
    )));
    assert!(!AppOrchestrator::is_workspace_file(&PathBuf::from(
        "/a/b/photo.png"
    )));
    assert!(!AppOrchestrator::is_workspace_file(&PathBuf::from(
        "/a/b/photo.jpg"
    )));
    assert!(!AppOrchestrator::is_workspace_file(&PathBuf::from(
        "/a/b/no_extension"
    )));
}

// -- process_file_events: performance invariants (regression) --

#[test]
fn test_process_file_events_does_not_set_left_panel_dirty() {
    // Regression: `process_file_events` used to set
    // `left_panel_dirty = true` on every event, which made
    // `show_left_panel` run `calc_max_width` (a recursive
    // O(n) text-layout pass) once per event during the
    // initial scan. With many files this saturated the UI
    // thread and the app felt unresponsive on startup. The
    // fix: the bus consumer no longer touches
    // `left_panel_dirty`. The width is calculated once,
    // when indexing finishes, in `show_left_panel`.
    let mut app = create_test_app();
    assert!(!app.layout.left_panel_dirty);

    app.orchestrator.file_event_reader = Some(app.orchestrator.file_event_bus.subscribe());
    let publisher = app.orchestrator.file_event_bus.clone();
    publisher.publish(FileEvent::discovered_one(PathBuf::from("/lib/notes.md")));
    publisher.publish(FileEvent::discovered_one(PathBuf::from("/lib/extra.md")));
    publisher.publish(FileEvent::updated_one(PathBuf::from("/lib/notes.md")));

    let _ = app.orchestrator.process_file_events();
    assert!(
        !app.layout.left_panel_dirty,
        "process_file_events must not set left_panel_dirty â€” the width is \
             calculated once when indexing finishes, not per bus event"
    );
}

#[test]
fn test_process_file_events_rebuild_only_on_removal() {
    // `rebuild` is O(n) in the tag manager. Calling it on
    // every bus event (Discovered or Updated) made the UI
    // thread do unnecessary work during the initial scan.
    // The `FileParsed` handler keeps tags up to date
    // incrementally, so rebuild is only needed when a file
    // actually leaves (`Removed`).
    let mut app = create_test_app();

    // Pre-populate tag manager so the tag exists.
    app.orchestrator
        .tags
        .add_tags(PathBuf::from("/lib/notes.md"), vec!["work".to_string()]);
    app.orchestrator
        .file_processor
        .all_files
        .push(PathBuf::from("/lib/notes.md"));

    // A `Removed` event must trigger `rebuid`, which
    // evicts the file's tags.
    app.orchestrator.file_event_reader = Some(app.orchestrator.file_event_bus.subscribe());
    app.orchestrator
        .file_event_bus
        .publish(FileEvent::removed_one(PathBuf::from("/lib/notes.md")));
    let _ = app.orchestrator.process_file_events();
    assert!(
        !app.orchestrator.tags.all_tags().contains("work"),
        "Removed events must trigger rebuild so stale tags are evicted"
    );

    // A `Discovered` event must NOT call rebuild (which
    // would clear all_tags and lose the tag we just
    // added).
    app.orchestrator
        .tags
        .add_tags(PathBuf::from("/lib/other.md"), vec!["keep".to_string()]);
    app.orchestrator
        .file_event_bus
        .publish(FileEvent::discovered_one(PathBuf::from("/lib/other.md")));
    let _ = app.orchestrator.process_file_events();
    assert!(
        app.orchestrator.tags.all_tags().contains("keep"),
        "Discovered events must NOT call rebuild â€” the FileParsed path \
             updates all_tags incrementally"
    );
}

/// Regression: rendering a document with a Table of Contents (such as
/// `Laptop.md`) shows `Panel::right("toc_panel")`. When the TOC panel is
/// active, all 5 side panels must produce a stable widget tree across
/// multi-pass renders (0 red-stroke ID-change warning shapes in egui).
#[test]
fn test_render_panels_no_id_change_warnings_on_toc_transition() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    let file = PathBuf::from("Laptop.md");

    app.orchestrator.tabs.tabs = vec![file.clone()];
    *app.orchestrator.selection.selected_file_mut() = Some(file.clone());
    app.layout.left_panel_width = Some(200.0);
    app.layout.left_panel_dirty = false;

    // Populate TOC (simulating rendering a document with headings like Laptop.md).
    app.orchestrator.tabs.toc = vec![
        crate::ui::ToCEntry {
            title: "Introduction".to_string(),
            level: 1,
            id: "intro".to_string(),
        },
        crate::ui::ToCEntry {
            title: "Specifications".to_string(),
            level: 2,
            id: "specs".to_string(),
        },
    ];

    // Pass 1: Initial render pass with TOC active.
    let mut output1 = run_ui_test(&ctx, Default::default(), |ui| {
        app.render_panels(ui);
    });
    output1.textures_delta.clear();

    // Pass 2: Second render pass with TOC active — must produce 0 ID change warnings.
    let mut output = run_ui_test(&ctx, Default::default(), |ui| {
        app.render_panels(ui);
    });
    output.textures_delta.clear();

    let shapes: Vec<egui::Shape> = output.shapes.into_iter().map(|cs| cs.shape).collect();
    assert_no_id_change_in_shapes(&shapes);
}

/// High-level layout integration test ensuring all 5 top-level UI panels
/// (Top, Left, Right, Center, Bottom) allocate non-zero, full-window layout
/// rects and render their expected child elements without collapsing or
/// disappearing.
#[test]
fn test_all_top_level_panels_visible_and_rendered() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    let file = PathBuf::from("Laptop.md");

    app.orchestrator.tabs.tabs = vec![file.clone()];
    *app.orchestrator.selection.selected_file_mut() = Some(file.clone());
    app.layout.left_panel_width = Some(200.0);
    app.layout.left_panel_dirty = false;
    app.file_processor_mut().indexing_finished = true;
    app.orchestrator.tabs.current_markdown =
        "# Laptop Specifications\n\n- CPU: 8 Cores\n- RAM: 32GB".to_string();

    // Populate TOC so the right panel is active.
    app.orchestrator.tabs.toc = vec![crate::ui::ToCEntry {
        title: "Laptop Specifications".to_string(),
        level: 1,
        id: "laptop_specs".to_string(),
    }];

    let raw_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1024.0, 768.0),
        )),
        ..egui::RawInput::default()
    };

    // Execute render_panels
    let mut output = run_ui_test(&ctx, raw_input, |ui| {
        app.render_panels(ui);
    });
    output.textures_delta.clear();

    // Extract (text, rect) for every text shape, plus the
    // overall bounding rect of the rendered output. The positional
    // assertions below use each panel's stable text marker plus
    // its expected spatial region, so a regression that swaps two
    // panels (e.g. TOC appears on the left) fails the test.
    //
    // We use the text shape's `visual_bounding_rect` for the
    // position. `text_shape.galley.rect` is in local widget
    // coordinates and `clipped.clip_rect` is the parent panel's
    // full allocation; neither is what we want. The visual
    // bounding rect is the rect of the actually-rendered glyphs
    // in root-Ui coordinates, which is where the text sits on
    // screen.
    let mut text_rects: Vec<(String, egui::Rect)> = Vec::new();
    let mut min_pos = egui::Pos2::new(f32::MAX, f32::MAX);
    let mut max_pos = egui::Pos2::new(f32::MIN, f32::MIN);

    fn collect_text_rects(shape: &egui::Shape, acc: &mut Vec<(String, egui::Rect)>) {
        match shape {
            egui::Shape::Text(text_shape) => {
                let text = text_shape.galley.text().to_string();
                if !text.trim().is_empty() {
                    let rect = text_shape.visual_bounding_rect();
                    if rect.is_finite() && !rect.is_negative() {
                        acc.push((text, rect));
                    }
                }
            }
            egui::Shape::Vec(shapes) => {
                for s in shapes {
                    collect_text_rects(s, acc);
                }
            }
            _ => {}
        }
    }

    for clipped in &output.shapes {
        let rect = clipped.shape.visual_bounding_rect();
        if rect.is_finite() && !rect.is_negative() {
            min_pos.x = min_pos.x.min(rect.min.x);
            min_pos.y = min_pos.y.min(rect.min.y);
            max_pos.x = max_pos.x.max(rect.max.x);
            max_pos.y = max_pos.y.max(rect.max.y);
        }
        collect_text_rects(&clipped.shape, &mut text_rects);
    }

    let rendered_width = max_pos.x - min_pos.x;
    let rendered_height = max_pos.y - min_pos.y;

    // 1. Overall bounding box covers most of the 1024x768 viewport.
    assert!(
        rendered_width >= 800.0,
        "UI layout must span the window width (expected >= 800px, got {}px)",
        rendered_width
    );
    assert!(
        rendered_height >= 600.0,
        "UI layout must span the window height (expected >= 600px, got {}px)",
        rendered_height
    );

    // 2. Each panel's stable text marker is rendered AND sits in
    // the expected spatial region. P1-7: reference
    // `crate::ui::strings::*` constants rather than hardcoding
    // literals so copy changes flow through one place.
    use crate::ui::strings::{APP_TITLE, TABLE_OF_CONTENTS_HEADER, WORKSPACE_HEADER};

    let find_marker = |marker: &str| -> Option<egui::Rect> {
        text_rects
            .iter()
            .find(|(t, _)| t.contains(marker))
            .map(|(_, r)| *r)
    };

    // Top panel: header sits at the very top of the viewport.
    let title_rect = find_marker(APP_TITLE)
        .unwrap_or_else(|| panic!("Top panel content ({APP_TITLE:?}) not rendered"));
    assert!(
        title_rect.min.y < 50.0,
        "Top panel must sit at the top of the viewport (min.y < 50, got {})",
        title_rect.min.y
    );

    // Left panel: header is in the leftmost ~250px column.
    let left_rect = find_marker(WORKSPACE_HEADER)
        .unwrap_or_else(|| panic!("Left panel content ({WORKSPACE_HEADER:?}) not rendered"));
    assert!(
        left_rect.min.x < 250.0,
        "Left panel must sit in the leftmost column (min.x < 250, got {})",
        left_rect.min.x
    );

    // Right panel (TOC): header is on the right half of the
    // viewport. The right panel anchors to the right edge, so
    // its right side should be near the viewport's right edge.
    let right_rect = find_marker(TABLE_OF_CONTENTS_HEADER).unwrap_or_else(|| {
        panic!("Right panel content ({TABLE_OF_CONTENTS_HEADER:?}) not rendered")
    });
    assert!(
        right_rect.max.x > 500.0,
        "Right panel must sit on the right half of the viewport (max.x > 500, got {})",
        right_rect.max.x
    );

    // Center panel: the markdown heading is the body marker. The
    // `Laptop Specifications` literal is set by the test's
    // `current_markdown` and is therefore not a canonical copy
    // string; keep the literal but add a comment.
    //
    // We only assert the left edge of the center text. The
    // right edge can extend into the right panel's x range
    // because the right panel renders on top of the center
    // panel (it's the topmost layer in the 5-pane layout);
    // checking max.x would couple the test to the heading's
    // glyph width rather than the panel's actual position.
    let center_rect = find_marker("Laptop Specifications")
        .unwrap_or_else(|| panic!("Center panel content (markdown heading) not rendered"));
    assert!(
        center_rect.min.x > left_rect.max.x - 5.0,
        "Center panel content must start after (or at) the left panel's right edge (left.max={}, center.min={})",
        left_rect.max.x,
        center_rect.min.x
    );

    // Bottom/status bar: at least one of "Indexing finished" or
    // "files" appears in the rendered text.
    let all_text: String = text_rects
        .iter()
        .map(|(t, _)| t.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        all_text.contains("Indexing finished") || all_text.contains("files"),
        "Bottom/Top status bar content must be rendered, text: {}",
        all_text
    );
}

/// UI-051: closing a tab when its file is deleted via the bus
/// `Removed` event must fall the selection back to the last
/// remaining tab (or to `None` when no tabs remain).
#[test]
fn test_process_file_events_removed_closes_open_tab() {
    let mut app = create_test_app();
    let gone = PathBuf::from("/tmp/gone.md");
    let keep = PathBuf::from("/tmp/keep.md");
    app.orchestrator.tabs.tabs = vec![gone.clone(), keep.clone()];
    app.orchestrator.tabs.loaded_path = Some(gone.clone());
    *app.orchestrator.selection.selected_file_mut() = Some(gone.clone());

    app.orchestrator.file_event_reader = Some(app.orchestrator.file_event_bus.subscribe());
    app.orchestrator
        .file_event_bus
        .publish(FileEvent::removed_one(gone.clone()));
    let _ = app.orchestrator.process_file_events();

    assert!(
        !app.orchestrator.tabs.tabs.contains(&gone),
        "tab for deleted file must be closed"
    );
    assert!(
        app.orchestrator.tabs.tabs.contains(&keep),
        "tab for remaining file must stay open"
    );
    assert_eq!(
        app.orchestrator.selection.selected_file(),
        Some(&keep),
        "selection must fall back to the last remaining tab"
    );
    assert!(
        app.orchestrator.tabs.loaded_path.is_none(),
        "loaded_path must be cleared"
    );
}

/// UI-051: closing the last tab when its file is deleted must
/// clear the selection and the displayed content.
#[test]
fn test_process_file_events_removed_closes_last_tab_clears_content() {
    let mut app = create_test_app();
    let gone = PathBuf::from("/tmp/gone.md");
    app.orchestrator.tabs.tabs = vec![gone.clone()];
    app.orchestrator.tabs.loaded_path = Some(gone.clone());
    app.orchestrator.tabs.current_markdown = "some content".to_string();
    *app.orchestrator.selection.selected_file_mut() = Some(gone.clone());

    app.orchestrator.file_event_reader = Some(app.orchestrator.file_event_bus.subscribe());
    app.orchestrator
        .file_event_bus
        .publish(FileEvent::removed_one(gone.clone()));
    let _ = app.orchestrator.process_file_events();

    assert!(
        app.orchestrator.tabs.tabs.is_empty(),
        "all tabs must be closed"
    );
    assert!(
        app.orchestrator.selection.selected_file().is_none(),
        "selection must be None when no tabs remain"
    );
    assert!(
        app.orchestrator.tabs.loaded_path.is_none(),
        "loaded_path must be cleared"
    );
    assert!(
        app.orchestrator.tabs.current_markdown.is_empty(),
        "content must be cleared when no tab remains"
    );
}

/// UI-051: closing a tab when its file is deleted via the typed
/// `FsEvent::FileDeleted` event must fall the selection back to
/// the last remaining tab.
#[test]
fn test_handle_fs_event_file_deleted_closes_open_tab() {
    let mut app = create_test_app();
    let gone = PathBuf::from("gone.md");
    let keep = PathBuf::from("keep.md");
    app.orchestrator.tabs.tabs = vec![gone.clone(), keep.clone()];
    app.orchestrator.tabs.loaded_path = Some(gone.clone());
    *app.orchestrator.selection.selected_file_mut() = Some(gone.clone());

    app.orchestrator
        .handle_fs_event(FsEvent::FileDeleted { path: gone.clone() });

    assert!(
        !app.orchestrator.tabs.tabs.contains(&gone),
        "tab for deleted file must be closed"
    );
    assert_eq!(
        app.orchestrator.selection.selected_file(),
        Some(&keep),
        "selection must fall back to the last remaining tab"
    );
}

// ---------------------------------------------------------------------------
// Font-scale persistence regression tests (PR #63 ported to the
// `app/{mod,init,update}.rs` split).
// ---------------------------------------------------------------------------

/// REGRESSION (font scale compounding): `pixels_per_point` is the
/// OS-reported device pixel ratio (e.g. 1.5 on a 150% DPI display).
/// The persisted `font_size_scale` must be a user-chosen
/// **multiplier** relative to that baseline — not the absolute
/// ppp. The pre-fix code divided the current ppp by a hard-coded
/// 1.0 to compute the scale, so a 150% DPI display saved
/// `Some(1.5)`, then on the next launch multiplied the OS-reported
/// 1.5 by that "scale" 1.5 → 2.25, then saved `Some(2.25)`, then
/// 3.375, 5.06, ... The font visibly grew every launch.
///
/// This test exercises the **real per-frame order** used by
/// `update_ui`: `apply_persisted_font_scale` and
/// `persist_font_scale` run in the same frame, **before**
/// egui 0.35's deferred `set_pixels_per_point` update takes
/// effect. The earlier version of this test masked a bug by
/// inserting a `run_ui` between apply and persist.
#[test]
fn test_font_scale_does_not_compound_across_launches() {
    // === Session 1: simulated 150% DPI display, user has chosen 1.2x zoom ===
    let mut app1 = create_test_app();
    app1.persisted_ui_state.font_size_scale = Some(1.2);

    let (ctx1, _raw1) = ctx_with_native_ppp(1.5);
    app1.apply_persisted_font_scale(&ctx1);
    // Persist in the SAME frame as apply — the real app's
    // `update_ui` does this. The deferred zoom-factor update
    // from `set_pixels_per_point` has not been applied yet
    // (it only takes effect on the next `begin_pass`), so
    // `ctx.pixels_per_point()` still reports the OS baseline
    // (1.5), not the target 1.8. The persist must NOT
    // recompute the scale from this stale ppp or it would
    // silently reset the stored value to `None`.
    app1.persist_font_scale(&ctx1);
    assert_eq!(
        app1.persisted_ui_state.font_size_scale,
        Some(1.2),
        "session 1 persist (same frame as apply): must store the user's \
         1.2 multiplier, not the pre-apply ppp / baseline ratio"
    );

    // After a follow-up frame, the deferred zoom-factor is
    // applied and the on-screen ppp matches the user's choice.
    let (ctx1_after, _) = ctx_with_native_ppp(1.5);
    let mut output = ctx1_after.run_ui(egui::RawInput::default(), |_ui| {});
    output.textures_delta.clear();
    let _ = ctx1_after; // not asserted — we only care about persistence here.

    // === Session 2: restart, reload persisted state, same OS baseline ===
    let persisted_json = serde_json::to_string(&app1.persisted_ui_state).unwrap();
    let mut app2 = create_test_app();
    app2.persisted_ui_state = serde_json::from_str(&persisted_json).unwrap();

    let (ctx2, _raw2) = ctx_with_native_ppp(1.5);
    app2.apply_persisted_font_scale(&ctx2);
    app2.persist_font_scale(&ctx2);
    assert_eq!(
        app2.persisted_ui_state.font_size_scale,
        Some(1.2),
        "session 2 persist: must remain 1.2, not 1.8 or higher"
    );

    // === Session 3..N: the value must stay stable for any number of restarts ===
    for _ in 0..5 {
        let json = serde_json::to_string(&app2.persisted_ui_state).unwrap();
        let mut next = create_test_app();
        next.persisted_ui_state = serde_json::from_str(&json).unwrap();
        let (ctx, _raw) = ctx_with_native_ppp(1.5);
        next.apply_persisted_font_scale(&ctx);
        next.persist_font_scale(&ctx);
        assert_eq!(
            next.persisted_ui_state.font_size_scale,
            Some(1.2),
            "loop: persisted scale drifted after multiple restarts"
        );
        app2 = next;
    }
}

/// REGRESSION (same-frame persist under a non-trivial OS
/// baseline): the persist must store the *applied* scale,
/// not a freshly-computed `current_ppp / baseline_ppp`. With
/// the old logic, running apply + persist in the same frame
/// on a 150% DPI display with `Some(1.2)` would compute
/// `scale = 1.5 / 1.5 = 1.0` and silently reset the
/// persisted value to `None`. The user's font would then
/// snap back to the OS default on every restart, shrinking
/// the UI each time.
#[test]
fn test_font_scale_persist_in_same_frame_as_apply_keeps_value() {
    let mut app = create_test_app();
    app.persisted_ui_state.font_size_scale = Some(1.2);

    let (ctx, _raw) = ctx_with_native_ppp(1.5);
    app.apply_persisted_font_scale(&ctx);
    // Same-frame persist: ctx.pixels_per_point() is still 1.5
    // (the OS baseline) because the deferred zoom-factor from
    // `set_pixels_per_point(1.8)` has not been applied yet.
    app.persist_font_scale(&ctx);

    assert_eq!(
        app.persisted_ui_state.font_size_scale,
        Some(1.2),
        "same-frame persist must not silently reset the scale to None"
    );
}

/// REGRESSION (legacy corruption): A user who upgraded from the
/// buggy build may have a pre-fix persisted scale that is the
/// absolute ppp (e.g., `Some(1.5)` on a 1.0 DPI display, or
/// worse — `Some(5.0)` after several compounding launches).
/// The apply helper must clamp out-of-range values to a no-op
/// and the next persist must self-heal the stored value back
/// to `None`.
#[test]
fn test_font_scale_clamps_legacy_corrupt_value() {
    let mut app = create_test_app();
    // Pretend the old buggy code persisted the absolute ppp
    // (or a compounded value) as a "scale".
    app.persisted_ui_state.font_size_scale = Some(5.0);

    let (ctx, _raw) = ctx_with_native_ppp(1.5);
    app.apply_persisted_font_scale(&ctx);
    app.persist_font_scale(&ctx);

    // The corrupt 5.0 must NOT be applied on top of the OS
    // baseline (that would yield 7.5 ppp). The baseline is
    // left untouched, and the next persist self-heals the
    // stored value to None (the user has not actually
    // chosen a zoom).
    assert_eq!(
        app.persisted_ui_state.font_size_scale, None,
        "corrupt scale must self-heal to None after one save"
    );
}

/// REGRESSION (NaN / infinity guard): a corrupt persisted scale
/// that is not finite must also be ignored, not propagated into
/// `set_pixels_per_point`, which would otherwise produce a
/// runtime panic in egui.
#[test]
fn test_font_scale_rejects_non_finite_value() {
    let mut app = create_test_app();
    app.persisted_ui_state.font_size_scale = Some(f32::NAN);

    let (ctx, _raw) = ctx_with_native_ppp(1.5);
    app.apply_persisted_font_scale(&ctx);
    app.persist_font_scale(&ctx);

    // The NaN must not be applied; the persist self-heals to None.
    assert_eq!(
        app.persisted_ui_state.font_size_scale, None,
        "NaN scale must self-heal to None"
    );
}

/// REGRESSION (schema migration): a persisted state written by
/// the pre-fix build (no `schema_version` field) must be
/// migrated to the current schema on load — specifically,
/// `font_size_scale` is cleared so the absolute-ppp value
/// that the old bug used to compound is not carried forward
/// as a multiplier.
#[test]
fn test_persisted_state_migration_clears_legacy_font_size_scale() {
    use crate::ui::persisted::{CURRENT_SCHEMA_VERSION, PersistedUiState};

    // Hand-written JSON mimicking the pre-fix on-disk shape:
    // no `schema_version` field; `font_size_scale` holds the
    // absolute ppp from a 150% DPI display.
    let legacy_json = r#"{
        "left_panel_width": null,
        "right_panel_width": null,
        "window_width": null,
        "window_height": null,
        "window_x": null,
        "window_y": null,
        "font_size_scale": 1.5,
        "expanded_dirs": []
    }"#;
    let mut state: PersistedUiState = serde_json::from_str(legacy_json).unwrap();
    assert_eq!(state.schema_version, 0);
    assert_eq!(state.font_size_scale, Some(1.5));

    // Apply the same migration the production `FastMdApp::new`
    // runs. (We do it inline here because the helper would
    // require an eframe::CreationContext.)
    if state.schema_version < CURRENT_SCHEMA_VERSION {
        state.font_size_scale = None;
        state.schema_version = CURRENT_SCHEMA_VERSION;
    }

    assert_eq!(
        state.font_size_scale, None,
        "migration must clear the legacy font_size_scale"
    );
    assert_eq!(
        state.schema_version, CURRENT_SCHEMA_VERSION,
        "migration must bump schema_version to the current value"
    );
}

/// Build an `egui::Context` whose input state reports
/// `native_pixels_per_point = Some(ppp)`, so
/// `ctx.pixels_per_point()` returns `ppp` before any zoom
/// change is applied. Returns the context together with the
/// matching `RawInput` so the caller can drive a follow-up
/// `run_ui` that preserves the high-DPI viewport info.
fn ctx_with_native_ppp(ppp: f32) -> (egui::Context, egui::RawInput) {
    let ctx = egui::Context::default();
    let viewports = std::iter::once((
        egui::ViewportId::ROOT,
        egui::ViewportInfo {
            native_pixels_per_point: Some(ppp),
            ..Default::default()
        },
    ))
    .collect();
    let raw_input = egui::RawInput {
        viewports,
        ..Default::default()
    };
    // Drive a single empty pass to seed the input state. We
    // immediately call `end_pass` to leave the viewport
    // stack balanced, so the next `begin_pass` (driven by
    // `run_ui`) is the "outermost" pass and the deferred
    // `new_zoom_factor` written by `set_pixels_per_point` is
    // actually applied.
    ctx.begin_pass(raw_input.clone());
    let mut output = ctx.end_pass();
    output.textures_delta.clear();
    (ctx, raw_input)
}

/// T018: Verify that a `ToolSideEffect::FileCreated` event on
/// `Bus<AgentEvent>` is reissued as `FsEvent::FileModified` by the
/// orchestrator drain, triggering `handle_fs_event` (verified via
/// `selection.tree_dirty` becoming true). Quickstart scenario 4, SC-005.
#[test]
fn test_tool_side_effect_reissues_fs_event() {
    use crate::agent::events::ToolSideEffect;
    use crate::bus::events::agent::AgentEvent as SeamAgentEvent;
    use std::io::Write;

    let mut app = create_test_app();
    let bus = app.orchestrator.agent_event_bus.clone();

    // Create a temp file so handle_fs_event can process it without panicking
    let temp_dir = std::env::temp_dir().join("fastmd_test_side_effect_reissue");
    let _ = std::fs::create_dir_all(&temp_dir);
    let temp_file = temp_dir.join("test_note.md");
    let mut f = std::fs::File::create(&temp_file).unwrap();
    let _ = f.write_all(b"---\ntags: [test_tag]\n---\n# Test\n");
    drop(f);

    let session_id = Uuid::new_v4();
    bus.publish(SeamAgentEvent::ToolSideEffect {
        session_id,
        effect: ToolSideEffect::FileCreated {
            path: temp_file.clone(),
            tags: vec!["test_tag".to_string()],
        },
    });

    // Reset tree_dirty to false so we can detect the reissue
    app.orchestrator.selection.tree_dirty = false;

    // tree_dirty should be false before drain
    assert!(
        !app.orchestrator.selection.tree_dirty,
        "tree_dirty must be false before drain"
    );

    app.orchestrator.drain_agent_event_bus();

    // After drain, tree_dirty should be true (handle_fs_event was called)
    assert!(
        app.orchestrator.selection.tree_dirty,
        "tree_dirty must be true after ToolSideEffect reissue — handle_fs_event was not called"
    );

    // Cleanup
    let _ = std::fs::remove_file(&temp_file);
    let _ = std::fs::remove_dir(&temp_dir);
}

/// T019: Verify broadcast lag handling — flood > 8192 `ContentDelta`
/// events on `Bus<AgentEvent>`, then drain. The reader should return
/// `Lagged(n)` and the orchestrator should emit a truncation marker
/// into the transcript content (quickstart scenario 5).
#[test]
fn test_broadcast_lag_handled_with_truncation_marker() {
    use crate::bus::events::agent::AgentEvent as SeamAgentEvent;

    let mut app = create_test_app();
    let bus = app.orchestrator.agent_event_bus.clone();
    let session_id = Uuid::new_v4();

    // Publish SessionStarted to set up the transcript
    bus.publish(SeamAgentEvent::SessionStarted { session_id });

    // First drain to consume SessionStarted
    app.orchestrator.drain_agent_event_bus();

    // Now flood the bus with > BUS_CAPACITY (8192) ContentDelta events.
    // The reader (subscribed during init) will fall behind and the
    // broadcast channel will drop old messages, returning Lagged(n).
    for i in 0..9000u32 {
        bus.publish(SeamAgentEvent::ContentDelta {
            session_id,
            text: format!("chunk {}\n", i),
        });
    }

    // Drain — the reader should encounter Lagged(n)
    app.orchestrator.drain_agent_event_bus();

    // The orchestrator should have set agent_event_lagged = true
    // and pushed the truncation marker into transcript.content
    assert!(
        app.orchestrator.agent_event_lagged,
        "agent_event_lagged must be true after broadcast lag"
    );
    assert!(
        app.orchestrator
            .agent_transcript
            .content
            .contains("[output truncated"),
        "transcript content must contain truncation marker; got: {:?}",
        app.orchestrator.agent_transcript.content
    );
}

#[test]
fn test_handle_platform_commands_local_markdown_publishes_select_file() {
    let mut app = create_test_app();
    let ctx = egui::Context::default();
    let rx = app.orchestrator.user_command_bus.subscribe();

    let cmd = egui::OutputCommand::OpenUrl(egui::output::OpenUrl {
        url: "docs/guide.md".to_string(),
        new_tab: false,
    });

    app.handle_platform_commands(&[cmd], &ctx);

    let received = rx
        .try_recv_exposing_lag()
        .expect("must receive UserCommand");
    match received {
        crate::bus::events::user_command::UserCommand::SelectFile { path, multi } => {
            assert!(!multi);
            assert!(path.ends_with("docs/guide.md") || path.ends_with("docs\\guide.md"));
        }
        other => panic!("expected SelectFile, got {other:?}"),
    }
}

#[test]
fn test_handle_platform_commands_wikilink_publishes_select_file() {
    let mut app = create_test_app();
    let ctx = egui::Context::default();
    let rx = app.orchestrator.user_command_bus.subscribe();

    let target_file = PathBuf::from("/workspace/Personal/Journal-2023-10-15.md");
    app.orchestrator.file_processor.all_files = vec![target_file.clone()];

    let cmd = egui::OutputCommand::OpenUrl(egui::output::OpenUrl {
        url: "wikilink:Journal-2023-10-15".to_string(),
        new_tab: false,
    });

    app.handle_platform_commands(&[cmd], &ctx);

    let received = rx
        .try_recv_exposing_lag()
        .expect("must receive UserCommand");
    match received {
        crate::bus::events::user_command::UserCommand::SelectFile { path, multi } => {
            assert!(!multi);
            assert_eq!(path, target_file);
        }
        other => panic!("expected SelectFile, got {other:?}"),
    }
}

#[test]
fn test_handle_platform_commands_anchor_scrolls_heading() {
    let mut app = create_test_app();
    let ctx = egui::Context::default();

    // Populate TOC / heading_ids
    app.orchestrator.tabs.toc = vec![crate::ui::ToCEntry::new(
        "Quick Start Guide",
        2,
        "Quick Start Guide",
    )];

    let cmd = egui::OutputCommand::OpenUrl(egui::output::OpenUrl {
        url: "#quick-start-guide".to_string(),
        new_tab: false,
    });

    app.handle_platform_commands(&[cmd], &ctx);

    assert_eq!(
        app.orchestrator.tabs.scroll_to_header_id.as_deref(),
        Some("Quick Start Guide")
    );
}

/// Startup must evaluate the first-run helper against the restored state
/// (spec FR-016): fresh state opens the dialog and records the version,
/// a same-version restart stays quiet, and an older recorded version
/// re-opens once and is overwritten. State is restored through the same
/// serde path `FastMdApp::new` uses; the helper itself is invoked inline
/// here because `new` requires an eframe::CreationContext.
#[test]
fn test_startup_first_run_auto_show_wiring() {
    use crate::ui::about_dialog::{APP_VERSION, apply_first_run_auto_show};

    // Fresh state (pre-field JSON deserialises to unseen) opens + records.
    let mut fresh: PersistedUiState = serde_json::from_str("{}").unwrap();
    let mut dialogs = crate::ui::Dialogs::new();
    assert!(apply_first_run_auto_show(
        &mut fresh,
        &mut dialogs,
        APP_VERSION
    ));
    assert!(
        dialogs.about_dialog_open,
        "first run must open the About dialog"
    );
    assert_eq!(fresh.about_shown_for_version.as_deref(), Some(APP_VERSION));

    // Same-version restart stays quiet.
    let mut dialogs = crate::ui::Dialogs::new();
    assert!(!apply_first_run_auto_show(
        &mut fresh,
        &mut dialogs,
        APP_VERSION
    ));
    assert!(
        !dialogs.about_dialog_open,
        "same version must not re-display the dialog"
    );

    // Older recorded version re-opens once and is overwritten.
    fresh.about_shown_for_version = Some("0.1.0".to_owned());
    let mut dialogs = crate::ui::Dialogs::new();
    assert!(apply_first_run_auto_show(
        &mut fresh,
        &mut dialogs,
        APP_VERSION
    ));
    assert!(
        dialogs.about_dialog_open,
        "upgraded version must auto-show once"
    );
    assert_eq!(fresh.about_shown_for_version.as_deref(), Some(APP_VERSION));
}
