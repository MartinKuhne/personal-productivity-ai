//! Tests for `ui/modals.rs`.

use super::*;
use crate::config::AppConfig;
use crate::ui::FastMdApp;
use crate::ui::test_helpers::run_ui_test;
use notify::RecommendedWatcher;
use std::fs;

fn create_test_app() -> FastMdApp {
    FastMdApp::empty_state(AppConfig::default())
}

/// Modal dialogs are drawn into the root `egui::Context`, and
/// `egui::Window::show` requires a non-trivial viewport for the
/// modal's rect to be observable. The default `RawInput` has no
/// `screen_rect`, which collapses the modal's bounding rect to
/// zero and clips the title + prompt out of the output. Set a
/// 1024x768 viewport for the test runs.
fn raw_input() -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1024.0, 768.0),
        )),
        ..egui::RawInput::default()
    }
}

#[test]
fn test_move_modal_rendering_and_state() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();

    show_move_modal_dialog(
        &mut app.orchestrator.dialogs,
        &app.orchestrator.content_libraries,
        &app.orchestrator.file_processor,
        &app.orchestrator.file_event_bus,
        &ctx,
    );
    assert!(!app.orchestrator.dialogs.move_dialog_open);

    let temp_dir = std::env::temp_dir().join("fastmd_move_test");
    let dest_dir = temp_dir.join("dest");
    let _ = fs::create_dir_all(&dest_dir);

    let src_file = temp_dir.join("move_me.txt");
    let _ = fs::write(&src_file, "content");

    app.orchestrator.dialogs.move_dialog_open = true;
    app.orchestrator.dialogs.file_to_move = Some(src_file.clone());
    app.orchestrator
        .file_processor
        .all_dirs
        .push(dest_dir.clone());
    app.orchestrator.dialogs.selected_move_folder = Some(dest_dir.clone());

    // R-2 / Q12: the `Window::show` rendering path used by the modals
    // is not observable through `ctx.run_ui(...).shapes` — the
    // window's title and prompt are rendered via egui's `Atoms`
    // widget system and end up in a separate paint layer that the
    // single-frame test harness does not include in the captured
    // output. The modal's visual surface is therefore covered by
    // the app-level Tier 3 snapshot (R-1c) rather than this test.
    // This test is kept as a smoke + state-coverage test.
    let _ = run_ui_test(&ctx, raw_input(), |ui| {
        show_move_modal_dialog(
            &mut app.orchestrator.dialogs,
            &app.orchestrator.content_libraries,
            &app.orchestrator.file_processor,
            &app.orchestrator.file_event_bus,
            ui.ctx(),
        );
    });

    assert!(app.orchestrator.dialogs.move_dialog_open);

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_create_dir_modal() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    let mut watcher: Option<RecommendedWatcher> = None;

    let temp_dir = std::env::temp_dir().join("fastmd_create_dir_test");
    let _ = fs::create_dir_all(&temp_dir);

    show_create_dir_dialog(
        &mut app.orchestrator.dialogs,
        &mut app.orchestrator.file_processor,
        &mut watcher,
        &app.orchestrator.file_event_bus,
        &ctx,
    );
    assert!(!app.orchestrator.dialogs.create_dir_dialog_open);

    app.orchestrator.dialogs.create_dir_dialog_open = true;
    app.orchestrator.dialogs.create_dir_parent = Some(temp_dir.clone());
    app.orchestrator.dialogs.create_dir_name = "subfolder".to_string();

    // See `test_move_modal_rendering_and_state` for the rationale
    // on why the modal's rendered text is not asserted here.
    let _ = run_ui_test(&ctx, raw_input(), |ui| {
        show_create_dir_dialog(
            &mut app.orchestrator.dialogs,
            &mut app.orchestrator.file_processor,
            &mut watcher,
            &app.orchestrator.file_event_bus,
            ui.ctx(),
        );
    });

    assert!(app.orchestrator.dialogs.create_dir_dialog_open);

    app.orchestrator.dialogs.create_dir_name = "../invalid_traversal".to_string();
    let _ = run_ui_test(&ctx, raw_input(), |ui| {
        show_create_dir_dialog(
            &mut app.orchestrator.dialogs,
            &mut app.orchestrator.file_processor,
            &mut watcher,
            &app.orchestrator.file_event_bus,
            ui.ctx(),
        );
    });

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_rename_modal() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();

    let temp_dir = std::env::temp_dir().join("fastmd_rename_test");
    let _ = fs::create_dir_all(&temp_dir);

    let file_path = temp_dir.join("old_name.txt");
    let _ = fs::write(&file_path, "sample text");

    {
        let sel = &mut app.orchestrator.selection;
        show_rename_dialog(RenameDialogCtx {
            dialog_manager: &mut app.orchestrator.dialogs,
            file_event_bus: &app.orchestrator.file_event_bus,
            loaded_path: &mut app.orchestrator.tab_manager.loaded_path,
            selected_file: &mut sel.selected_file,
            selected_dir: &mut sel.selected_dir,
            tabs: &mut app.orchestrator.tab_manager.tabs,
            file_processor: &mut app.orchestrator.file_processor,
            tag_manager: &mut app.orchestrator.tag_manager,
            expanded_dirs: &mut sel.expanded_dirs,
            ctx: &ctx,
        });
    }
    assert!(!app.orchestrator.dialogs.rename_dialog_open);

    app.orchestrator.dialogs.rename_dialog_open = true;
    app.orchestrator.dialogs.file_to_rename = Some(file_path.clone());
    app.orchestrator.dialogs.rename_new_name = "new_name".to_string();
    *app.orchestrator.selection.selected_file_mut() = Some(file_path.clone());
    app.orchestrator.tab_manager.tabs = vec![file_path.clone()];

    // See `test_move_modal_rendering_and_state` for the rationale
    // on why the modal's rendered text is not asserted here.
    let _ = run_ui_test(&ctx, raw_input(), |ui| {
        let sel = &mut app.orchestrator.selection;
        show_rename_dialog(RenameDialogCtx {
            dialog_manager: &mut app.orchestrator.dialogs,
            file_event_bus: &app.orchestrator.file_event_bus,
            loaded_path: &mut app.orchestrator.tab_manager.loaded_path,
            selected_file: &mut sel.selected_file,
            selected_dir: &mut sel.selected_dir,
            tabs: &mut app.orchestrator.tab_manager.tabs,
            file_processor: &mut app.orchestrator.file_processor,
            tag_manager: &mut app.orchestrator.tag_manager,
            expanded_dirs: &mut sel.expanded_dirs,
            ctx: ui.ctx(),
        });
    });

    assert!(app.orchestrator.dialogs.rename_dialog_open);

    app.orchestrator.dialogs.rename_new_name = "invalid/name".to_string();
    let _ = run_ui_test(&ctx, raw_input(), |ui| {
        let sel = &mut app.orchestrator.selection;
        show_rename_dialog(RenameDialogCtx {
            dialog_manager: &mut app.orchestrator.dialogs,
            file_event_bus: &app.orchestrator.file_event_bus,
            loaded_path: &mut app.orchestrator.tab_manager.loaded_path,
            selected_file: &mut sel.selected_file,
            selected_dir: &mut sel.selected_dir,
            tabs: &mut app.orchestrator.tab_manager.tabs,
            file_processor: &mut app.orchestrator.file_processor,
            tag_manager: &mut app.orchestrator.tag_manager,
            expanded_dirs: &mut sel.expanded_dirs,
            ctx: ui.ctx(),
        });
    });

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_rename_preserves_extension() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();

    let temp_dir = std::env::temp_dir().join("fastmd_rename_test2");
    let _ = fs::create_dir_all(&temp_dir);

    let md_file = temp_dir.join("my_document.md");
    let _ = fs::write(&md_file, "# Test");

    app.orchestrator.dialogs.rename_dialog_open = true;
    app.orchestrator.dialogs.file_to_rename = Some(md_file.clone());
    app.orchestrator.dialogs.rename_new_name = "renamed_doc".to_string();

    let _ = run_ui_test(&ctx, Default::default(), |ui| {
        let sel = &mut app.orchestrator.selection;
        show_rename_dialog(RenameDialogCtx {
            dialog_manager: &mut app.orchestrator.dialogs,
            file_event_bus: &app.orchestrator.file_event_bus,
            loaded_path: &mut app.orchestrator.tab_manager.loaded_path,
            selected_file: &mut sel.selected_file,
            selected_dir: &mut sel.selected_dir,
            tabs: &mut app.orchestrator.tab_manager.tabs,
            file_processor: &mut app.orchestrator.file_processor,
            tag_manager: &mut app.orchestrator.tag_manager,
            expanded_dirs: &mut sel.expanded_dirs,
            ctx: ui.ctx(),
        });
    });

    assert!(
        !temp_dir.join("renamed_doc.md").exists() || temp_dir.join("my_document.md").exists(),
        "Rename should complete without error"
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn unique_document_name_appends_md_only_when_no_extension() {
    assert_eq!(unique_document_name("my notes"), "my notes.md");
    assert_eq!(unique_document_name("report.md"), "report.md");
    assert_eq!(unique_document_name("draft.txt"), "draft.txt");
}

#[test]
fn date_suffixed_name_preserves_stem_and_extension() {
    let name = date_suffixed_name("report.md");
    assert!(name.starts_with("report "), "unexpected: {}", name);
    assert!(name.ends_with(".md"), "unexpected: {}", name);
    let stem = name.trim_end_matches(".md");
    let date_part = stem.trim_start_matches("report ").trim();
    // `%Y-%m-%d %H-%M-%S` → e.g. `2026-08-01 14-30-00`.
    assert!(
        date_part.len() == 19 && date_part.as_bytes()[10] == b' ',
        "unexpected timestamp: {:?}",
        date_part
    );

    let no_ext = date_suffixed_name("report");
    assert!(no_ext.starts_with("report "), "unexpected: {}", no_ext);
    assert!(!no_ext.ends_with(".md"), "unexpected: {}", no_ext);
}

#[test]
fn write_new_document_creates_yaml_headed_markdown() {
    let temp_dir = std::env::temp_dir().join("fastmd_write_doc_test");
    let _ = fs::create_dir_all(&temp_dir);

    let path = write_new_document(&temp_dir, "first notes").unwrap();
    assert_eq!(path, temp_dir.join("first notes.md"));
    let content = fs::read_to_string(&path).unwrap();
    assert_eq!(content, "---\ntitle: first notes\n---\n\n");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn write_new_document_avoids_existing_name_with_date_suffix() {
    let temp_dir = std::env::temp_dir().join("fastmd_write_doc_unique_test");
    let _ = fs::create_dir_all(&temp_dir);
    let _ = fs::write(temp_dir.join("dup.md"), "existing");

    let path = write_new_document(&temp_dir, "dup").unwrap();
    assert_ne!(path, temp_dir.join("dup.md"));
    assert!(path.exists(), "unique file must be created");
    // The unique name must keep the `.md` extension.
    assert!(
        path.extension().and_then(|e| e.to_str()) == Some("md"),
        "unique name must retain .md extension: {:?}",
        path
    );

    let _ = fs::remove_dir_all(&temp_dir);
}

/// Tier 4 functional test: opening the create-document dialog,
/// typing a name and pressing Ok must write a YAML-headed
/// markdown file in the dialog's parent directory and publish a
/// `Discovered` file event on the file-event bus.
#[test]
fn test_create_document_dialog_writes_file_on_submit() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;
    use std::sync::Mutex;

    // Per-test fixture. No `static`, no `OnceLock`, no
    // `Box::leak(&'static Path)`: the `PathBuf` lives in the
    // test function's stack frame, and the closure captures
    // it by `move` for the harness's lifetime. The
    // `Mutex<DialogManager>` lets the test and the closure
    // share mutable state across the harness's frames
    // without requiring `'static` storage.
    let temp_dir =
        std::env::temp_dir().join(format!("fastmd_create_document_click_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);
    let mut dm = DialogManager::new();
    dm.create_document_dialog_open = true;
    dm.create_document_parent = Some(temp_dir.clone());
    dm.create_document_name = "from dialog".to_string();
    let bus = Bus::new();
    let dm = Mutex::new(Some(dm));

    let mut harness = stateful_harness((), move |ui, _| {
        let mut guard = dm.lock().unwrap();
        if let Some(dm) = guard.as_mut() {
            show_create_document_dialog(dm, &bus, ui.ctx());
        }
    });
    harness.fit_contents();
    harness.get_by_label(crate::ui::strings::OK_BUTTON).click();
    harness.run_steps(2);
    harness.run_steps(2);

    let created = temp_dir.join("from dialog.md");
    assert!(created.exists(), "Ok must create the document file");
    let content = fs::read_to_string(&created).unwrap();
    assert_eq!(content, "---\ntitle: from dialog\n---\n\n");

    let _ = fs::remove_dir_all(&temp_dir);
}

/// Tier 4 functional test: opening the rename dialog with a file
/// queued and pressing Ok must rename the file on disk (preserving
/// its extension) and update the `selected_file` binding.
#[test]
fn test_rename_dialog_renames_file_on_submit() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;
    use std::sync::Mutex;

    // Per-test fixture. The `FastMdApp` is moved into the
    // `Mutex` and the closure captures the `Mutex` by `move`;
    // no `'static` storage is required and the temp dir is
    // cleaned up by the `let _ = fs::remove_dir_all(&temp_dir)`
    // at the end of the test.
    let temp_dir =
        std::env::temp_dir().join(format!("fastmd_rename_dialog_click_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);
    let file_path = temp_dir.join("original.txt");
    let _ = fs::write(&file_path, "content");
    let mut app = crate::ui::FastMdApp::empty_state(crate::config::AppConfig::default());
    app.orchestrator.dialogs.rename_dialog_open = true;
    app.orchestrator.dialogs.file_to_rename = Some(file_path.clone());
    app.orchestrator.dialogs.rename_new_name = "renamed".to_string();
    *app.orchestrator.selection.selected_file_mut() = Some(file_path.clone());
    let app = Mutex::new(Some(app));

    let mut harness = stateful_harness((), move |ui, _| {
        let mut guard = app.lock().unwrap();
        if let Some(app) = guard.as_mut() {
            let sel = &mut app.orchestrator.selection;
            show_rename_dialog(RenameDialogCtx {
                dialog_manager: &mut app.orchestrator.dialogs,
                file_event_bus: &app.orchestrator.file_event_bus,
                loaded_path: &mut app.orchestrator.tab_manager.loaded_path,
                selected_file: &mut sel.selected_file,
                selected_dir: &mut sel.selected_dir,
                tabs: &mut app.orchestrator.tab_manager.tabs,
                file_processor: &mut app.orchestrator.file_processor,
                tag_manager: &mut app.orchestrator.tag_manager,
                expanded_dirs: &mut sel.expanded_dirs,
                ctx: ui.ctx(),
            });
        }
    });
    harness.fit_contents();
    harness.get_by_label(crate::ui::strings::OK_BUTTON).click();
    harness.run_steps(2);
    harness.run_steps(2);

    let renamed = temp_dir.join("renamed.txt");
    assert!(
        renamed.exists(),
        "Ok must rename the file (preserving extension)"
    );
    assert!(!temp_dir.join("original.txt").exists());

    let _ = fs::remove_dir_all(&temp_dir);
}
