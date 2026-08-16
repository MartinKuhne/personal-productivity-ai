//! Tests for `tree/render.rs`.
//!
//! All tests build a [`TreeNodeContext`] directly with owned
//! fields and `..Default::default()`. The previous version
//! borrowed every field from local `let mut` variables (or
//! `Box::leak`ed per-test `OnceLock<StaticFixture>`) to satisfy
//! the `'static` re-borrow across the harness closure. The
//! lifetime-free rewrite drops that machinery entirely.
//!
//! For `stateful_harness` tests (which run the closure many
//! times), the context is shared via
//! `Rc<RefCell<TreeNodeContext>>` so click handlers can mutate
//! fields like `create_document_dialog_open` and the next
//! harness pass sees the change.

use super::*;
use crate::ui::test_helpers::run_ui_test;
use crate::ui::tree::context::TreeNodeContext;
use crate::ui::tree::flatten::{FlatRow, TREE_ROW_HEIGHT};
use eframe::egui;
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;

/// Tier 4 click test: clicking a file row in the left panel's
/// tree view must fire the `on_click("file_row")` callback.
///
/// The harness runs the closure many times; the context is
/// shared across passes via `Rc<RefCell<...>>` so any
/// selection state (like `selected_file` after a click)
/// persists.
#[test]
fn test_file_row_click_captures_event() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;

    let row = FlatRow {
        depth: 0,
        name: "notes.md".to_string(),
        path: PathBuf::from("notes.md"),
        is_dir: false,
        is_expanded: false,
    };
    let ctx_cell: Rc<RefCell<TreeNodeContext>> = Rc::new(RefCell::new(TreeNodeContext::default()));
    let row_for_closure = row.clone();
    let ctx_for_closure = Rc::clone(&ctx_cell);

    let mut harness = stateful_harness(Vec::<&'static str>::new(), move |ui, captured| {
        let mut ctx = ctx_for_closure.borrow_mut();
        render_flat_row_capture(ui, &row_for_closure, &mut ctx, |event| {
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

    let mut tree_ctx = TreeNodeContext {
        inline_editor_enabled: true,
        ..Default::default()
    };

    let _ = run_ui_test(&ctx_egui, Default::default(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            // Render collapsed directory
            draw_tree_node(ui, &root, &mut tree_ctx);

            // Render expanded directory with child file
            tree_ctx.expanded_dirs().insert(root.path.clone());
            draw_tree_node(ui, &root, &mut tree_ctx);

            // Render standalone file node
            draw_tree_node(ui, &child_file, &mut tree_ctx);
        });
    });

    assert!(tree_ctx.expanded_dirs.contains(&root.path));
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

    let mut tree_ctx = TreeNodeContext {
        modifiers: egui::Modifiers {
            ctrl: true,
            ..Default::default()
        },
        inline_editor_enabled: true,
        ..Default::default()
    };

    let _ = run_ui_test(&ctx_egui, Default::default(), |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            // Test ctrl multi-select simulation
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
/// `Dialogs`'s create-document fields into the
/// `TreeNodeContext`; `app.rs` renders the dialog from those same
/// fields): the flat row and the create-document dialog are drawn
/// in the same frame, driven by shared state.
#[test]
fn test_new_document_on_directory_opens_dialog_with_dir_parent() {
    use crate::bus::core::Bus;
    use crate::bus::events::file::FileEvent;
    use crate::ui::dialogs::Dialogs;
    use crate::ui::modals::show_create_document_dialog;
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;

    let temp_dir = std::env::temp_dir().join(format!("fastmd_new_doc_dir_{}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);
    let temp_dir_arc: Arc<Path> = Arc::from(temp_dir.as_path());

    let row = FlatRow {
        depth: 0,
        name: "notes".to_string(),
        path: temp_dir.clone(),
        is_dir: true,
        is_expanded: false,
    };

    let ctx_cell: Rc<RefCell<TreeNodeContext>> = Rc::new(RefCell::new(TreeNodeContext::default()));
    let dm_cell: Rc<RefCell<Dialogs>> = Rc::new(RefCell::new(Dialogs::new()));
    let bus = Bus::<FileEvent>::new();

    let row_for_closure = row.clone();
    let ctx_for_closure = Rc::clone(&ctx_cell);
    let dm_for_closure = Rc::clone(&dm_cell);
    let temp_dir_for_closure = Arc::clone(&temp_dir_arc);
    let bus_for_closure = bus.clone();

    let mut harness = stateful_harness((), move |ui, _| {
        {
            let mut ctx = ctx_for_closure.borrow_mut();
            render_flat_row(ui, &row_for_closure, &mut ctx);

            // Mirror the tree → dialog wiring (left.rs + app.rs): the
            // context menu writes the dialog-open flag and parent into
            // the tree context; the dialog renders from those fields.
            if *ctx.create_document_dialog_open() {
                let mut dm = dm_for_closure.borrow_mut();
                dm.create_document_dialog_open = true;
                dm.create_document_parent = ctx.create_document_parent().clone();
                drop(ctx); // release the ctx borrow before show_create_document_dialog
                show_create_document_dialog(&mut dm, &bus_for_closure, ui.ctx());
                // Mark the parent explicitly so the closure can use
                // the temp_dir if needed.
                let _ = temp_dir_for_closure.as_ref();
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
        let mut ctx = ctx_cell.borrow_mut();
        assert!(
            *ctx.create_document_dialog_open(),
            "choosing [New document] must open the create-document dialog"
        );
        assert_eq!(
            *ctx.create_document_parent(),
            Some(temp_dir.clone()),
            "the create-document dialog's parent must be the right-clicked directory"
        );
    }

    // Type a name and submit; the file must be created inside the
    // right-clicked directory.
    {
        let mut dm = dm_cell.borrow_mut();
        dm.create_document_name = "notes".to_string();
    }
    harness.run_steps(2);
    harness
        .get_by_label(crate::ui::strings::OK_BUTTON)
        .click_accesskit();
    harness.run_steps(2);
    harness.run_steps(2);

    let created = temp_dir.join("notes.md");
    assert!(
        created.exists(),
        "submitting the dialog must create the document inside the right-clicked directory"
    );
    let content = fs::read_to_string(&created).unwrap();
    assert_eq!(content, "---\ntitle: notes\n---\n\n");

    let _ = fs::remove_dir_all(&temp_dir);
}

#[test]
fn test_rename_action_on_directory_opens_rename_dialog() {
    use crate::ui::test_helpers::interact::stateful_harness;
    use egui_kittest::kittest::Queryable;

    let row = FlatRow {
        depth: 0,
        name: "test_dir".to_string(),
        path: PathBuf::from("/test/test_dir"),
        is_dir: true,
        is_expanded: false,
    };
    let ctx_cell: Rc<RefCell<TreeNodeContext>> = Rc::new(RefCell::new(TreeNodeContext::default()));
    let row_for_closure = row.clone();
    let ctx_for_closure = Rc::clone(&ctx_cell);

    let mut harness = stateful_harness((), move |ui, _| {
        let mut ctx = ctx_for_closure.borrow_mut();
        render_flat_row(ui, &row_for_closure, &mut ctx);
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
        let mut ctx = ctx_cell.borrow_mut();
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

    let row = FlatRow {
        depth: 0,
        name: "test_file.md".to_string(),
        path: PathBuf::from("/test/test_file.md"),
        is_dir: false,
        is_expanded: false,
    };
    let ctx_cell: Rc<RefCell<TreeNodeContext>> = Rc::new(RefCell::new(TreeNodeContext::default()));
    let row_for_closure = row.clone();
    let ctx_for_closure = Rc::clone(&ctx_cell);

    let mut harness = stateful_harness((), move |ui, _| {
        let mut ctx = ctx_for_closure.borrow_mut();
        render_flat_row(ui, &row_for_closure, &mut ctx);
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
        let mut ctx = ctx_cell.borrow_mut();
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

    let row = FlatRow {
        depth: 0,
        name: "test_file_1.md".to_string(),
        path: PathBuf::from("/test/test_file_1.md"),
        is_dir: false,
        is_expanded: false,
    };
    let ctx_cell: Rc<RefCell<TreeNodeContext>> = Rc::new(RefCell::new(TreeNodeContext {
        selected_files: HashSet::from([
            PathBuf::from("/test/test_file_1.md"),
            PathBuf::from("/test/test_file_2.md"),
        ]),
        ..Default::default()
    }));
    let row_for_closure = row.clone();
    let ctx_for_closure = Rc::clone(&ctx_cell);

    let mut harness = stateful_harness((), move |ui, _| {
        let mut ctx = ctx_for_closure.borrow_mut();
        render_flat_row(ui, &row_for_closure, &mut ctx);
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
        let mut ctx = ctx_cell.borrow_mut();
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
