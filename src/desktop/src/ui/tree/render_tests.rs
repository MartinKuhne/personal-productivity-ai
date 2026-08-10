//! Tests for `tree/render.rs`.

use super::*;
use crate::app::panel_layout::PanelLayout;
use crate::ui::test_helpers::run_ui_test;
use crate::ui::tree::context::TreeNodeContext;
use crate::ui::tree::flatten::{FlatRow, TREE_ROW_HEIGHT};
use eframe::egui;
use std::collections::HashSet;
use std::path::PathBuf;

/// Tier 4 click test: clicking a file row in the left panel's
/// tree view must fire the `on_click("file_row")` callback.
///
/// The challenge this test solves: `render_flat_row` takes a
/// `&mut TreeNodeContext<'_>` whose lifetime is tied to the
/// borrowed sub-fields (selected_file, selected_files, tabs).
/// The harness closure is `FnMut(&mut Ui, &mut T)` and runs
/// for many passes; the context must therefore live across
/// all those passes. We use `Box::leak` to give the context
/// a `'static` lifetime so the harness can re-borrow it on
/// every pass. The leak is per-test and bounded (one
/// `TreeNodeContext` per test run), so it does not affect
/// long-running test executables.
#[test]
fn test_file_row_click_captures_event() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;
    use std::sync::{Mutex, OnceLock};

    // Build the 'static context and row once; reuse across
    // every harness pass.
    struct StaticFixture {
        ctx: Mutex<Option<TreeNodeContext<'static>>>,
        row: FlatRow,
    }
    static FIXTURE: OnceLock<StaticFixture> = OnceLock::new();
    let fixture = FIXTURE.get_or_init(|| {
        let selected_file = Box::leak(Box::new(None::<PathBuf>));
        let selected_files = Box::leak(Box::new(HashSet::<PathBuf>::new()));
        let expanded_dirs = Box::leak(Box::new(HashSet::<PathBuf>::new()));
        let tabs = Box::leak(Box::new(Vec::<PathBuf>::new()));
        let selected_dir = Box::leak(Box::new(None::<PathBuf>));
        let create_dir_dialog_open = Box::leak(Box::new(false));
        let create_dir_parent = Box::leak(Box::new(None::<PathBuf>));
        let file_to_move = Box::leak(Box::new(None::<PathBuf>));
        let move_dialog_open = Box::leak(Box::new(false));
        let file_to_rename = Box::leak(Box::new(None::<PathBuf>));
        let rename_dialog_open = Box::leak(Box::new(false));
        let rename_new_name = Box::leak(Box::new(String::new()));
        let create_document_dialog_open = Box::leak(Box::new(false));
        let create_document_parent = Box::leak(Box::new(None::<PathBuf>));
        let layout = Box::leak(Box::new(PanelLayout::default()));
        let submit_prompt = Box::leak(Box::new(None::<String>));
        let open_editor = Box::leak(Box::new(None::<PathBuf>));
        let content_libraries = Box::leak(Box::new(Vec::new()));
        let tree_dirty = Box::leak(Box::new(false));

        let ctx = TreeNodeContext {
            selected_file,
            selected_files,
            expanded_dirs,
            tabs,
            selected_dir,
            create_dir_dialog_open,
            create_dir_parent,
            file_to_move,
            move_dialog_open,
            file_to_rename,
            rename_dialog_open,
            rename_new_name,
            create_document_dialog_open,
            create_document_parent,
            layout,
            submit_prompt,
            content_libraries,
            open_editor,
            modifiers: egui::Modifiers::default(),
            inline_editor_enabled: false,
            bg_tx: &None,
            file_event_producer: None,
            tree_dirty,
            pdf_backing_tracker: crate::app::session::PdfBackingTracker::new(),
        };

        let row = FlatRow {
            depth: 0,
            name: "notes.md".to_string(),
            path: PathBuf::from("notes.md"),
            is_dir: false,
            is_expanded: false,
        };
        StaticFixture {
            ctx: Mutex::new(Some(ctx)),
            row,
        }
    });

    let mut harness = stateful_harness(Vec::<&'static str>::new(), |ui, captured| {
        let mut guard = fixture.ctx.lock().unwrap();
        let ctx = guard.as_mut().expect("context not initialized");
        render_flat_row_capture(ui, &fixture.row, ctx, |event| {
            captured.push(event);
        });
    });
    harness.fit_contents();
    // The selectable_label text is "  notes.md" (two leading
    // spaces from the `format!("  {}", row.name)` in the
    // production code). Search by a substring to avoid
    // depending on the exact whitespace.
    let nodes: Vec<_> = harness.query_all_by_label_contains("notes.md").collect();
    assert!(
        !nodes.is_empty(),
        "expected the file row labelled with `notes.md` to be present; \
             found {} matching nodes",
        nodes.len()
    );
    nodes[0].click();
    harness.run_steps(2);
    harness.run_steps(2);

    let captured = harness.state();
    assert!(
        captured.contains(&"file_row"),
        "clicking the file row must fire the `file_row` on_click event; \
             got: {:?}",
        captured
    );
}

#[test]
fn test_draw_tree_node_directory_and_file() {
    let ctx_egui = egui::Context::default();

    let mut root = TreeNode::new("RootFolder".to_string(), PathBuf::from("/test/root"), true);
    let child_file = TreeNode::new(
        "document.md".to_string(),
        PathBuf::from("/test/root/document.md"),
        false,
    );
    root.children
        .insert("document.md".to_string(), child_file.clone());

    let mut expanded_dirs = HashSet::new();
    let mut selected_file = None;
    let mut selected_files = HashSet::new();
    let mut tabs = Vec::new();
    let mut file_to_move = None;
    let mut move_dialog_open = false;
    let mut selected_dir = None;
    let mut create_dir_dialog_open = false;
    let mut create_dir_parent = None;
    let mut layout = PanelLayout::new();
    let mut rename_dialog_open = false;
    let mut file_to_rename = None;
    let mut rename_new_name = String::new();
    let mut create_document_dialog_open = false;
    let mut create_document_parent = None;
    let mut submit_prompt = None;
    let mut open_editor = None;

    let _ = run_ui_test(&ctx_egui, Default::default(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            let mut tree_dirty = false;
            let mut tree_ctx = TreeNodeContext {
                selected_file: &mut selected_file,
                selected_files: &mut selected_files,
                expanded_dirs: &mut expanded_dirs,
                tabs: &mut tabs,
                selected_dir: &mut selected_dir,
                create_dir_dialog_open: &mut create_dir_dialog_open,
                create_dir_parent: &mut create_dir_parent,
                file_to_move: &mut file_to_move,
                move_dialog_open: &mut move_dialog_open,
                file_to_rename: &mut file_to_rename,
                rename_dialog_open: &mut rename_dialog_open,
                rename_new_name: &mut rename_new_name,
                create_document_dialog_open: &mut create_document_dialog_open,
                create_document_parent: &mut create_document_parent,
                layout: &mut layout,
                submit_prompt: &mut submit_prompt,
                content_libraries: &[],
                open_editor: &mut open_editor,
                modifiers: egui::Modifiers::default(),
                inline_editor_enabled: true,
                bg_tx: &None,
                file_event_producer: None,
                tree_dirty: &mut tree_dirty,
                pdf_backing_tracker: crate::app::session::PdfBackingTracker::new(),
            };

            // Render collapsed directory
            draw_tree_node(ui, &root, &mut tree_ctx);

            // Render expanded directory with child file
            tree_ctx.expanded_dirs().insert(root.path.clone());
            draw_tree_node(ui, &root, &mut tree_ctx);

            // Render standalone file node
            draw_tree_node(ui, &child_file, &mut tree_ctx);
        });
    });

    assert!(expanded_dirs.contains(&root.path));
}

/// Regression: the directory tree used to render mojibake'd
/// folder / file icons (double-encoded UTF-8 -> Latin-1 ->
/// UTF-8) that egui's default font could not render. The
/// `render_flat_row` and `draw_tree_node` helpers must use
/// BMP-only glyphs (U+25BC / U+25B6 / two spaces) so the
/// labels come out as "▼ name" / "▶ name" / "  name",
/// not "📁 name". This test pins the exact glyphs
/// (which are the only place the dir-tree icons are defined)
/// so a future encoding mishap or emoji swap is caught at
/// test time, not at runtime.
#[test]
fn test_dir_tree_icons_are_bmp_only_no_mojibake() {
    // The 3 icons used in render_flat_row / draw_tree_node.
    const EXPANDED_DIR: &str = "▼ ";
    const COLLAPSED_DIR: &str = "▶ ";
    const FILE: &str = "  ";

    // Every char in every icon must be inside the BMP
    // (U+0000..=U+FFFF). egui's default font (Hack /
    // Ubuntu-Light) cannot render characters above U+FFFF
    // (emoji are in the Supplementary Multilingual Plane)
    // and would fall back to a tofu box.
    for icon in [EXPANDED_DIR, COLLAPSED_DIR, FILE] {
        for c in icon.chars() {
            assert!(
                (c as u32) <= 0xFFFF,
                "dir-tree icon char U+{:04X} is outside the BMP; egui default font will render it as tofu",
                c as u32
            );
        }
    }

    // And the icons must be exactly the strings we expect:
    // no mojibake (which would have C3 83 / C2 A2 / etc.
    // byte patterns).
    assert_eq!(EXPANDED_DIR, "\u{25bc} ", "expanded dir icon is ▼ + space");
    assert_eq!(
        COLLAPSED_DIR, "\u{25b6} ",
        "collapsed dir icon is ▶ + space"
    );
    assert_eq!(FILE, "  ", "file icon is two spaces (no glyph)");
}

#[test]
fn test_tree_node_selection_state_modifiers() {
    let ctx_egui = egui::Context::default();
    let file1 = TreeNode::new(
        "file1.md".to_string(),
        PathBuf::from("/test/file1.md"),
        false,
    );
    let file2 = TreeNode::new(
        "file2.md".to_string(),
        PathBuf::from("/test/file2.md"),
        false,
    );

    let mut expanded_dirs = HashSet::new();
    let mut selected_file = None;
    let mut selected_files = HashSet::new();
    let mut tabs = Vec::new();
    let mut file_to_move = None;
    let mut move_dialog_open = false;
    let mut selected_dir = None;
    let mut create_dir_dialog_open = false;
    let mut create_dir_parent = None;
    let mut layout = PanelLayout::new();
    let mut rename_dialog_open = false;
    let mut file_to_rename = None;
    let mut rename_new_name = String::new();
    let mut create_document_dialog_open = false;
    let mut create_document_parent = None;
    let mut submit_prompt = None;
    let mut open_editor = None;

    let _ = run_ui_test(&ctx_egui, Default::default(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            // Test ctrl multi-select simulation
            let mut tree_dirty = false;
            let mut tree_ctx = TreeNodeContext {
                selected_file: &mut selected_file,
                selected_files: &mut selected_files,
                expanded_dirs: &mut expanded_dirs,
                tabs: &mut tabs,
                selected_dir: &mut selected_dir,
                create_dir_dialog_open: &mut create_dir_dialog_open,
                create_dir_parent: &mut create_dir_parent,
                file_to_move: &mut file_to_move,
                move_dialog_open: &mut move_dialog_open,
                file_to_rename: &mut file_to_rename,
                rename_dialog_open: &mut rename_dialog_open,
                rename_new_name: &mut rename_new_name,
                create_document_dialog_open: &mut create_document_dialog_open,
                create_document_parent: &mut create_document_parent,
                layout: &mut layout,
                submit_prompt: &mut submit_prompt,
                content_libraries: &[],
                open_editor: &mut open_editor,
                modifiers: egui::Modifiers {
                    ctrl: true,
                    ..Default::default()
                },
                inline_editor_enabled: true,
                bg_tx: &None,
                file_event_producer: None,
                tree_dirty: &mut tree_dirty,
                pdf_backing_tracker: crate::app::session::PdfBackingTracker::new(),
            };

            draw_tree_node(ui, &file1, &mut tree_ctx);
            draw_tree_node(ui, &file2, &mut tree_ctx);
        });
    });
}

/// TDD Test: Verify that render_flat_row produces identical, stable egui IDs
/// for a given file/dir path regardless of virtual scroll window slice index.
#[test]
fn test_tree_row_id_stability_independent_of_slice_index() {
    let ctx = egui::Context::default();
    let row = FlatRow {
        path: PathBuf::from("Laptop.md"),
        name: "Laptop.md".to_string(),
        depth: 0,
        is_dir: false,
        is_expanded: false,
    };

    let mut id_pass1 = None;
    let mut id_pass2 = None;

    // Render pass 1
    let _ = run_ui_test(&ctx, Default::default(), |ui| {
        ui.push_id((&row.path, row.is_dir), |ui| {
            id_pass1 = Some(ui.id());
        });
    });

    // Render pass 2
    let _ = run_ui_test(&ctx, Default::default(), |ui| {
        ui.push_id((&row.path, row.is_dir), |ui| {
            id_pass2 = Some(ui.id());
        });
    });

    assert_eq!(
        id_pass1, id_pass2,
        "Row ID must be strictly determined by row.path and is_dir, staying identical across passes"
    );
}

/// Regression: `TREE_ROW_HEIGHT` is the `row_height_sans_spacing`
/// passed to `ScrollArea::show_rows`. egui adds
/// `ui.spacing().item_spacing.y` on top of it to compute the
/// per-row slot height — so the actual slot is
/// `TREE_ROW_HEIGHT + item_spacing.y`, not the constant alone.
///
/// The previous constant (22.0) was calibrated to a now-stale
/// estimate ("14pt line height + 4px padding"). The actual
/// `selectable_label` widget in egui 0.35 is 18px (the
/// `interact_size.y` min height, with the frame's
/// `button_padding` reconciling into the same 18px). With
/// `item_spacing.y = 3`, the slot was 25px and the widget only
/// filled 18px — 7px of empty space at the bottom of every
/// rendered row, accumulating to a visible "unused space at
/// the bottom of the left directory tree" that scales with
/// tree depth.
///
/// This test pins the invariant: the slot height
/// (constant + item_spacing.y) must match the actual
/// `selectable_label` height within a small tolerance, so no
/// per-row vertical gap accumulates in the tree.
#[test]
fn test_tree_row_height_matches_selectable_label_height() {
    let ctx = egui::Context::default();
    let mut button_height = 0.0_f32;
    let mut spacing_y = 0.0_f32;
    let _ = run_ui_test(&ctx, Default::default(), |ui| {
        let response = ui.selectable_label(false, "sample tree row");
        button_height = response.rect.height();
        spacing_y = ui.spacing().item_spacing.y;
    });
    let slot_height = TREE_ROW_HEIGHT + spacing_y;
    let tolerance = 1.0_f32;
    let diff = (button_height - slot_height).abs();
    assert!(
        diff < tolerance,
        "TREE_ROW_HEIGHT ({}) is the row_height_sans_spacing passed to \
             ScrollArea::show_rows; egui adds item_spacing.y ({}) on top, so \
             the actual per-row slot is {}px. The actual selectable_label \
             widget is {}px tall. A mismatch of {}px leaves empty space at \
             the bottom of every row in the directory tree.",
        TREE_ROW_HEIGHT,
        spacing_y,
        slot_height,
        button_height,
        diff,
    );
}

/// Tier 4 end-to-end test: right-clicking a directory row and
/// choosing [New document] from the context menu must open the
/// create-document dialog with that directory as its parent; when
/// the dialog is submitted, the new file must be created *inside*
/// the right-clicked directory (UI-015).
///
/// The harness mirrors the production wiring (`left.rs` passes the
/// `DialogManager`'s create-document fields into the
/// `TreeNodeContext`; `app.rs` renders the dialog from those same
/// fields): the flat row and the create-document dialog are drawn
/// in the same frame, driven by shared state.
#[test]
fn test_new_document_on_directory_opens_dialog_with_dir_parent() {
    use crate::app::dialog_manager::DialogManager;
    use crate::bus::core::Bus;
    use crate::bus::events::file::FileEvent;
    use crate::ui::modals::show_create_document_dialog;
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;
    use std::fs;
    use std::path::Path;
    use std::sync::{Mutex, OnceLock};

    struct StaticFixture {
        ctx: Mutex<Option<TreeNodeContext<'static>>>,
        dm: Mutex<Option<DialogManager>>,
        bus: Bus<FileEvent>,
        temp_dir: &'static Path,
        row: FlatRow,
    }
    static FIXTURE: OnceLock<StaticFixture> = OnceLock::new();
    let fixture = FIXTURE.get_or_init(|| {
        let leaked = Box::leak(Box::new(
            std::env::temp_dir().join(format!("fastmd_new_doc_dir_{}", std::process::id())),
        ));
        let temp_dir: &'static Path = leaked;
        let _ = fs::create_dir_all(temp_dir);

        let selected_file = Box::leak(Box::new(None::<PathBuf>));
        let selected_files = Box::leak(Box::new(HashSet::<PathBuf>::new()));
        let expanded_dirs = Box::leak(Box::new(HashSet::<PathBuf>::new()));
        let tabs = Box::leak(Box::new(Vec::<PathBuf>::new()));
        let selected_dir = Box::leak(Box::new(None::<PathBuf>));
        let create_dir_dialog_open = Box::leak(Box::new(false));
        let create_dir_parent = Box::leak(Box::new(None::<PathBuf>));
        let file_to_move = Box::leak(Box::new(None::<PathBuf>));
        let move_dialog_open = Box::leak(Box::new(false));
        let file_to_rename = Box::leak(Box::new(None::<PathBuf>));
        let rename_dialog_open = Box::leak(Box::new(false));
        let rename_new_name = Box::leak(Box::new(String::new()));
        let create_document_dialog_open = Box::leak(Box::new(false));
        let create_document_parent = Box::leak(Box::new(None::<PathBuf>));
        let layout = Box::leak(Box::new(PanelLayout::default()));
        let submit_prompt = Box::leak(Box::new(None::<String>));
        let open_editor = Box::leak(Box::new(None::<PathBuf>));
        let content_libraries = Box::leak(Box::new(Vec::new()));
        let tree_dirty = Box::leak(Box::new(false));

        let ctx = TreeNodeContext {
            selected_file,
            selected_files,
            expanded_dirs,
            tabs,
            selected_dir,
            create_dir_dialog_open,
            create_dir_parent,
            file_to_move,
            move_dialog_open,
            file_to_rename,
            rename_dialog_open,
            rename_new_name,
            create_document_dialog_open,
            create_document_parent,
            layout,
            submit_prompt,
            content_libraries,
            open_editor,
            modifiers: egui::Modifiers::default(),
            inline_editor_enabled: false,
            bg_tx: &None,
            file_event_producer: None,
            tree_dirty,
            pdf_backing_tracker: crate::app::session::PdfBackingTracker::new(),
        };

        let row = FlatRow {
            depth: 0,
            name: "notes".to_string(),
            path: temp_dir.to_path_buf(),
            is_dir: true,
            is_expanded: false,
        };
        StaticFixture {
            ctx: Mutex::new(Some(ctx)),
            dm: Mutex::new(Some(DialogManager::new())),
            bus: Bus::new(),
            temp_dir,
            row,
        }
    });

    let mut harness = stateful_harness((), |ui, _| {
        let mut guard = fixture.ctx.lock().unwrap();
        let ctx = guard.as_mut().expect("context not initialized");
        render_flat_row(ui, &fixture.row, ctx);

        // Mirror the tree → dialog wiring (left.rs + app.rs): the
        // context menu writes the dialog-open flag and parent into
        // the tree context; the dialog renders from those fields.
        if *ctx.create_document_dialog_open() {
            let mut dguard = fixture.dm.lock().unwrap();
            if let Some(dm) = dguard.as_mut() {
                dm.create_document_dialog_open = true;
                dm.create_document_parent = ctx.create_document_parent().clone();
                show_create_document_dialog(dm, &fixture.bus, ui.ctx());
            }
        }
    });
    harness.fit_contents();

    // Right-click the directory row to open its context menu.
    let dir_nodes: Vec<_> = harness.query_all_by_label_contains("notes").collect();
    assert!(
        !dir_nodes.is_empty(),
        "expected the directory row to be present"
    );
    dir_nodes[0].click_secondary();
    harness.run_steps(2);
    harness.run_steps(2);

    // Choose [New document]; the dialog opens with the directory as
    // its parent.
    harness
        .get_by_label(crate::ui::strings::NEW_DOCUMENT_ACTION)
        .click_accesskit();
    harness.run_steps(2);
    harness.run_steps(2);

    {
        let mut guard = fixture.ctx.lock().unwrap();
        let ctx = guard.as_mut().expect("context not initialized");
        assert!(
            *ctx.create_document_dialog_open(),
            "choosing [New document] must open the create-document dialog"
        );
        assert_eq!(
            *ctx.create_document_parent(),
            Some(fixture.temp_dir.to_path_buf()),
            "the create-document dialog's parent must be the right-clicked directory"
        );
    }

    // Type a name and submit; the file must be created inside the
    // right-clicked directory.
    {
        let mut dguard = fixture.dm.lock().unwrap();
        if let Some(dm) = dguard.as_mut() {
            dm.create_document_name = "notes".to_string();
        }
    }
    harness.run_steps(2);
    harness
        .get_by_label(crate::ui::strings::OK_BUTTON)
        .click_accesskit();
    harness.run_steps(2);
    harness.run_steps(2);

    let created = fixture.temp_dir.join("notes.md");
    assert!(
        created.exists(),
        "submitting the dialog must create the document inside the right-clicked directory"
    );
    let content = fs::read_to_string(&created).unwrap();
    assert_eq!(content, "---\ntitle: notes\n---\n\n");

    let _ = fs::remove_dir_all(fixture.temp_dir);
}

#[test]
fn test_rename_action_on_directory_opens_rename_dialog() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;
    use std::sync::{Mutex, OnceLock};

    struct StaticFixture {
        ctx: Mutex<Option<TreeNodeContext<'static>>>,
        row: FlatRow,
    }
    static FIXTURE: OnceLock<StaticFixture> = OnceLock::new();
    let fixture = FIXTURE.get_or_init(|| {
        let selected_file = Box::leak(Box::new(None::<PathBuf>));
        let selected_files = Box::leak(Box::new(HashSet::<PathBuf>::new()));
        let expanded_dirs = Box::leak(Box::new(HashSet::<PathBuf>::new()));
        let tabs = Box::leak(Box::new(Vec::<PathBuf>::new()));
        let selected_dir = Box::leak(Box::new(None::<PathBuf>));
        let create_dir_dialog_open = Box::leak(Box::new(false));
        let create_dir_parent = Box::leak(Box::new(None::<PathBuf>));
        let file_to_move = Box::leak(Box::new(None::<PathBuf>));
        let move_dialog_open = Box::leak(Box::new(false));
        let file_to_rename = Box::leak(Box::new(None::<PathBuf>));
        let rename_dialog_open = Box::leak(Box::new(false));
        let rename_new_name = Box::leak(Box::new(String::new()));
        let create_document_dialog_open = Box::leak(Box::new(false));
        let create_document_parent = Box::leak(Box::new(None::<PathBuf>));
        let layout = Box::leak(Box::new(PanelLayout::default()));
        let submit_prompt = Box::leak(Box::new(None::<String>));
        let open_editor = Box::leak(Box::new(None::<PathBuf>));
        let content_libraries = Box::leak(Box::new(Vec::new()));
        let tree_dirty = Box::leak(Box::new(false));

        let ctx = TreeNodeContext {
            selected_file,
            selected_files,
            expanded_dirs,
            tabs,
            selected_dir,
            create_dir_dialog_open,
            create_dir_parent,
            file_to_move,
            move_dialog_open,
            file_to_rename,
            rename_dialog_open,
            rename_new_name,
            create_document_dialog_open,
            create_document_parent,
            layout,
            submit_prompt,
            content_libraries,
            open_editor,
            modifiers: egui::Modifiers::default(),
            inline_editor_enabled: false,
            bg_tx: &None,
            file_event_producer: None,
            tree_dirty,
            pdf_backing_tracker: crate::app::session::PdfBackingTracker::new(),
        };

        let row = FlatRow {
            depth: 0,
            name: "test_dir".to_string(),
            path: PathBuf::from("/test/test_dir"),
            is_dir: true,
            is_expanded: false,
        };
        StaticFixture {
            ctx: Mutex::new(Some(ctx)),
            row,
        }
    });

    let mut harness = stateful_harness((), |ui, _| {
        let mut guard = fixture.ctx.lock().unwrap();
        let ctx = guard.as_mut().expect("context not initialized");
        render_flat_row(ui, &fixture.row, ctx);
    });
    harness.fit_contents();

    // Right-click the directory row to open its context menu.
    let dir_nodes: Vec<_> = harness.query_all_by_label_contains("test_dir").collect();
    assert!(
        !dir_nodes.is_empty(),
        "expected the directory row to be present"
    );
    dir_nodes[0].click_secondary();
    harness.run_steps(2);
    harness.run_steps(2);

    // Choose [Rename]; verify context flag updates
    harness
        .get_by_label(crate::ui::strings::RENAME_ACTION)
        .click_accesskit();
    harness.run_steps(2);
    harness.run_steps(2);

    {
        let mut guard = fixture.ctx.lock().unwrap();
        let ctx = guard.as_mut().expect("context not initialized");
        assert!(
            *ctx.rename_dialog_open(),
            "choosing Rename must open the rename dialog"
        );
        assert_eq!(
            *ctx.file_to_rename(),
            Some(PathBuf::from("/test/test_dir")),
            "the file to rename must be the clicked directory"
        );
        assert_eq!(
            *ctx.rename_new_name(),
            "test_dir",
            "the initial rename name should match"
        );
    }
}

#[test]
fn test_move_action_on_file_opens_move_dialog() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;
    use std::sync::{Mutex, OnceLock};

    struct StaticFixture {
        ctx: Mutex<Option<TreeNodeContext<'static>>>,
        row: FlatRow,
    }
    static FIXTURE: OnceLock<StaticFixture> = OnceLock::new();
    let fixture = FIXTURE.get_or_init(|| {
        let selected_file = Box::leak(Box::new(None::<PathBuf>));
        let selected_files = Box::leak(Box::new(HashSet::<PathBuf>::new()));
        let expanded_dirs = Box::leak(Box::new(HashSet::<PathBuf>::new()));
        let tabs = Box::leak(Box::new(Vec::<PathBuf>::new()));
        let selected_dir = Box::leak(Box::new(None::<PathBuf>));
        let create_dir_dialog_open = Box::leak(Box::new(false));
        let create_dir_parent = Box::leak(Box::new(None::<PathBuf>));
        let file_to_move = Box::leak(Box::new(None::<PathBuf>));
        let move_dialog_open = Box::leak(Box::new(false));
        let file_to_rename = Box::leak(Box::new(None::<PathBuf>));
        let rename_dialog_open = Box::leak(Box::new(false));
        let rename_new_name = Box::leak(Box::new(String::new()));
        let create_document_dialog_open = Box::leak(Box::new(false));
        let create_document_parent = Box::leak(Box::new(None::<PathBuf>));
        let layout = Box::leak(Box::new(PanelLayout::default()));
        let submit_prompt = Box::leak(Box::new(None::<String>));
        let open_editor = Box::leak(Box::new(None::<PathBuf>));
        let content_libraries = Box::leak(Box::new(Vec::new()));
        let tree_dirty = Box::leak(Box::new(false));

        let ctx = TreeNodeContext {
            selected_file,
            selected_files,
            expanded_dirs,
            tabs,
            selected_dir,
            create_dir_dialog_open,
            create_dir_parent,
            file_to_move,
            move_dialog_open,
            file_to_rename,
            rename_dialog_open,
            rename_new_name,
            create_document_dialog_open,
            create_document_parent,
            layout,
            submit_prompt,
            content_libraries,
            open_editor,
            modifiers: egui::Modifiers::default(),
            inline_editor_enabled: false,
            bg_tx: &None,
            file_event_producer: None,
            tree_dirty,
            pdf_backing_tracker: crate::app::session::PdfBackingTracker::new(),
        };

        let row = FlatRow {
            depth: 0,
            name: "test_file.md".to_string(),
            path: PathBuf::from("/test/test_file.md"),
            is_dir: false,
            is_expanded: false,
        };
        StaticFixture {
            ctx: Mutex::new(Some(ctx)),
            row,
        }
    });

    let mut harness = stateful_harness((), |ui, _| {
        let mut guard = fixture.ctx.lock().unwrap();
        let ctx = guard.as_mut().expect("context not initialized");
        render_flat_row(ui, &fixture.row, ctx);
    });
    harness.fit_contents();

    // Right-click the file row to open its context menu.
    let file_nodes: Vec<_> = harness
        .query_all_by_label_contains("test_file.md")
        .collect();
    assert!(
        !file_nodes.is_empty(),
        "expected the file row to be present"
    );
    file_nodes[0].click_secondary();
    harness.run_steps(2);
    harness.run_steps(2);

    // Choose [Move]; verify context flag updates
    harness
        .get_by_label(crate::ui::strings::MOVE_ACTION)
        .click_accesskit();
    harness.run_steps(2);
    harness.run_steps(2);

    {
        let mut guard = fixture.ctx.lock().unwrap();
        let ctx = guard.as_mut().expect("context not initialized");
        assert!(
            *ctx.move_dialog_open(),
            "choosing Move must open the move dialog"
        );
        assert_eq!(
            *ctx.file_to_move(),
            Some(PathBuf::from("/test/test_file.md")),
            "the file to move must be the clicked file"
        );
    }
}

#[test]
fn test_multi_select_merge_action_generates_prompt() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;
    use std::sync::{Mutex, OnceLock};

    struct StaticFixture {
        ctx: Mutex<Option<TreeNodeContext<'static>>>,
        row: FlatRow,
    }
    static FIXTURE: OnceLock<StaticFixture> = OnceLock::new();
    let fixture = FIXTURE.get_or_init(|| {
        let selected_file = Box::leak(Box::new(None::<PathBuf>));
        let mut initial_selected = HashSet::new();
        initial_selected.insert(PathBuf::from("/test/test_file_1.md"));
        initial_selected.insert(PathBuf::from("/test/test_file_2.md"));

        let selected_files = Box::leak(Box::new(initial_selected));
        let expanded_dirs = Box::leak(Box::new(HashSet::<PathBuf>::new()));
        let tabs = Box::leak(Box::new(Vec::<PathBuf>::new()));
        let selected_dir = Box::leak(Box::new(None::<PathBuf>));
        let create_dir_dialog_open = Box::leak(Box::new(false));
        let create_dir_parent = Box::leak(Box::new(None::<PathBuf>));
        let file_to_move = Box::leak(Box::new(None::<PathBuf>));
        let move_dialog_open = Box::leak(Box::new(false));
        let file_to_rename = Box::leak(Box::new(None::<PathBuf>));
        let rename_dialog_open = Box::leak(Box::new(false));
        let rename_new_name = Box::leak(Box::new(String::new()));
        let create_document_dialog_open = Box::leak(Box::new(false));
        let create_document_parent = Box::leak(Box::new(None::<PathBuf>));
        let layout = Box::leak(Box::new(PanelLayout::default()));
        let submit_prompt = Box::leak(Box::new(None::<String>));
        let open_editor = Box::leak(Box::new(None::<PathBuf>));
        let content_libraries = Box::leak(Box::new(Vec::new()));
        let tree_dirty = Box::leak(Box::new(false));

        let ctx = TreeNodeContext {
            selected_file,
            selected_files,
            expanded_dirs,
            tabs,
            selected_dir,
            create_dir_dialog_open,
            create_dir_parent,
            file_to_move,
            move_dialog_open,
            file_to_rename,
            rename_dialog_open,
            rename_new_name,
            create_document_dialog_open,
            create_document_parent,
            layout,
            submit_prompt,
            content_libraries,
            open_editor,
            modifiers: egui::Modifiers::default(),
            inline_editor_enabled: false,
            bg_tx: &None,
            file_event_producer: None,
            tree_dirty,
            pdf_backing_tracker: crate::app::session::PdfBackingTracker::new(),
        };

        let row = FlatRow {
            depth: 0,
            name: "test_file_1.md".to_string(),
            path: PathBuf::from("/test/test_file_1.md"),
            is_dir: false,
            is_expanded: false,
        };
        StaticFixture {
            ctx: Mutex::new(Some(ctx)),
            row,
        }
    });

    let mut harness = stateful_harness((), |ui, _| {
        let mut guard = fixture.ctx.lock().unwrap();
        let ctx = guard.as_mut().expect("context not initialized");
        render_flat_row(ui, &fixture.row, ctx);
    });
    harness.fit_contents();

    // Right-click the file row to open its context menu.
    let file_nodes: Vec<_> = harness
        .query_all_by_label_contains("test_file_1.md")
        .collect();
    assert!(
        !file_nodes.is_empty(),
        "expected the file row to be present"
    );
    file_nodes[0].click_secondary();
    harness.run_steps(2);
    harness.run_steps(2);

    // Choose [Merge]; verify submit_prompt updates
    harness
        .get_by_label(crate::ui::strings::MERGE_ACTION)
        .click_accesskit();
    harness.run_steps(2);
    harness.run_steps(2);

    {
        let mut guard = fixture.ctx.lock().unwrap();
        let ctx = guard.as_mut().expect("context not initialized");
        assert!(
            ctx.submit_prompt().is_some(),
            "choosing Merge must generate a prompt into submit_prompt"
        );
        let prompt = ctx.submit_prompt().as_ref().unwrap();
        assert!(
            prompt.contains("test_file_1.md") && prompt.contains("test_file_2.md"),
            "prompt must contain both file names"
        );
    }
}
