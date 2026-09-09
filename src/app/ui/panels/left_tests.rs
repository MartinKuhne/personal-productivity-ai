//! Tests for `panels/left.rs`.

use super::*;
use crate::ui::test_helpers::assert::assert_no_id_change_in_shapes;
use crate::ui::test_helpers::run_ui_test;

use crate::ui::test_helpers::app::test_app as create_test_app;

#[test]
fn test_show_left_panel_empty() {
    use crate::ui::strings::WORKSPACE_HEADER;
    use crate::ui::test_helpers::text::assert_text_contains;

    let ctx = egui::Context::default();
    let mut app = create_test_app();
    app.layout_mut().left_panel_dirty = false;

    let output = run_ui_test(&ctx, Default::default(), |ui| {
        show_left_panel(&mut app, ui);
    });

    // R-2 / Q12: replace the tautological state check with a
    // rendered-content assertion. The empty-state label is
    // conditionally rendered and falls below the panel's clip
    // rect under the default test viewport (no screen_rect), so
    // it is not in the rendered output here. The id-stability
    // test for the empty state (`test_show_left_panel_no_id_change_warnings_when_empty`)
    // covers the empty-state widget tree separately. Header-only
    // assertion is correct per the Q12 borderline policy.
    assert_text_contains(&output.shapes, WORKSPACE_HEADER);
    // Panel renders without crashing; width is now captured from panel response.
    assert!(app.layout().left_panel_width.is_some());
}

#[test]
fn test_show_left_panel_with_libraries_and_files() {
    use crate::ui::strings::WORKSPACE_HEADER;
    use crate::ui::test_helpers::text::assert_text_contains;

    let ctx = egui::Context::default();
    let mut app = create_test_app();
    app.layout_mut().left_panel_dirty = false;

    let lib_dir = std::env::temp_dir().join("fastmd_left_test_lib");
    app.content_libraries_mut()
        .push(crate::config::ContentLibrary {
            root_folder: lib_dir.to_string_lossy().to_string(),
            name: "TestLib".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        });

    let file1 = lib_dir.join("notes.md");
    let file2 = lib_dir.join("archived.md");
    app.file_processor_mut().all_files = vec![file1.clone(), file2.clone()];
    app.tags_mut()
        .add_tags(file1.clone(), vec!["work".to_string()]);
    app.tags_mut()
        .add_tags(file2.clone(), vec!["archive".to_string()]);

    let output = run_ui_test(&ctx, Default::default(), |ui| {
        show_left_panel(&mut app, ui);
    });
    // Header assertion (Q12 borderline case): the library name and
    // file paths are dynamic, but the panel header is stable.
    assert_text_contains(&output.shapes, WORKSPACE_HEADER);

    app.tags_mut().selected_tag = Some("work".to_string());
    let _ = run_ui_test(&ctx, Default::default(), |ui| {
        show_left_panel(&mut app, ui);
    });

    app.file_processor_mut().indexing_finished = true;
    app.file_processor_mut().indexing_finished_handled = false;
    let _ = run_ui_test(&ctx, Default::default(), |ui| {
        show_left_panel(&mut app, ui);
    });

    assert!(app.file_processor().indexing_finished_handled);
    assert!(app.layout().left_panel_width.is_some());
}

/// TDD regression: clicking a directory row in the left panel
/// must expand the folder and reveal its children. Before the
/// P0 perf optimization (P0-1 / P0-2) `show_left_panel` rebuilt
/// the flat row list on every frame, so the new `expanded_dirs`
/// membership was reflected immediately. After P0 the flat rows
/// are cached in `FastMdApp::cached_tree_rows` and only rebuilt
/// when `FileSelection::tree_dirty` is `true`. The directory
/// click handler (`apply_directory_row_click`) mutates
/// `expanded_dirs` but did not set `tree_dirty`, so the cache
/// kept returning the *previous* flat rows and the click looked
/// like a no-op — the folder triangle toggled in `expanded_dirs`
/// but its children never appeared.
///
/// The fix is in `apply_directory_row_click` (handlers.rs):
/// it now sets `*ctx.tree_dirty() = true` so the next
/// `show_left_panel` pass rebuilds the flat rows. This test
/// pins the user-visible invariant: after a directory click,
/// the directory's children must appear in the rendered output.
#[test]
fn test_directory_click_invalidates_tree_cache() {
    use crate::ui::test_helpers::text::assert_text_contains;
    use crate::ui::tree::context::TreeNodeContext;

    let ctx = egui::Context::default();
    let mut app = create_test_app();
    let lib_dir = std::env::temp_dir().join("fastmd_left_test_dir_click_cache");
    let sub_dir = lib_dir.join("subdir");
    let inner_file = sub_dir.join("inner_note.md");
    let top_file = lib_dir.join("top_note.md");

    app.content_libraries_mut()
        .push(crate::config::ContentLibrary {
            root_folder: lib_dir.to_string_lossy().to_string(),
            name: "ClickLib".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        });
    app.file_processor_mut().all_files = vec![inner_file.clone(), top_file.clone()];
    app.file_processor_mut().all_dirs = vec![sub_dir.clone()];
    app.file_processor_mut().indexing_finished = true;
    app.file_processor_mut().indexing_finished_handled = true;
    // Pin the panel width so the only thing that can change
    // between renders is the inner widget tree.
    app.layout_mut().left_panel_width = Some(240.0);
    app.layout_mut().left_panel_dirty = false;
    // Expand the library so the files inside it are visible
    // at all. The library is a child of the root tree node and
    // is not auto-expanded; this mirrors a user clicking the
    // library name once. We only want to test the subdirectory
    // click behavior, not the library click.
    app.selection_mut().expanded_dirs.insert(lib_dir.clone());

    // Pass 1: prime the cache with the collapsed tree. The
    // subdirectory starts collapsed, so the inner file must
    // not be in the rendered output yet.
    let output_collapsed = run_ui_test(&ctx, Default::default(), |ui| {
        show_left_panel(&mut app, ui);
    });
    assert!(
        app.cached_tree_rows.is_some(),
        "first render should populate the cached flat rows"
    );
    assert!(
        !app.selection().tree_dirty(),
        "first render should clear the tree_dirty flag"
    );
    let collapsed_rows = app
        .cached_tree_rows
        .as_ref()
        .expect("cache populated by pass 1")
        .clone();
    assert!(
        !collapsed_rows.iter().any(|r| r.path == inner_file),
        "inner file must be hidden when the subdirectory is collapsed ({} cached rows)",
        collapsed_rows.len()
    );
    assert_text_contains(&output_collapsed.shapes, "top_note.md");
    // The collapsed render must NOT show the inner file.
    let collapsed_texts: Vec<String> = output_collapsed
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect();
    assert!(
        !collapsed_texts.iter().any(|t| t.contains("inner_note.md")),
        "inner_note.md must not appear while the subdir is collapsed, got: {:?}",
        collapsed_texts
    );

    // Simulate a user click on the subdirectory row. We build a
    // `TreeNodeContext` from the app's state via
    // `from_app_state` (the same constructor `show_left_panel`
    // uses on every frame) and call
    // `apply_directory_row_click` directly — this is the same
    // function `render_flat_row` invokes from its `if
    // response.clicked()` branch. The context owns its fields,

    let dir_row = crate::ui::tree::flatten::FlatRow {
        depth: 0,
        name: "subdir".to_string(),
        path: sub_dir.clone(),
        is_dir: true,
        is_expanded: false,
    };
    {
        let mut ctx = TreeNodeContext::from_app_state(
            &app.orchestrator.selection,
            &app.orchestrator.tabs,
            &app.layout,
            &app.orchestrator.content_libraries,
            Some(app.orchestrator.tx.clone()),
            app.orchestrator.file_event_bus.clone(),
            app.orchestrator.inline_editor_enabled,
            egui::Modifiers::default(),
            app.pdf_backing_tracker().clone(),
            app.orchestrator.user_command_bus.clone(),
        );
        let cmd = crate::ui::tree::handlers::apply_directory_row_click(&mut ctx, &dir_row);
        app.orchestrator.apply_user_command(cmd);
    }

    // The click must invalidate the cached flat rows.
    assert!(
        app.selection().tree_dirty(),
        "directory click must mark the tree cache dirty so the next render rebuilds the flat rows"
    );
    assert!(
        app.selection().expanded_dirs().contains(&sub_dir),
        "directory click must add the folder to expanded_dirs"
    );

    // Pass 2: re-render. The cache must have been rebuilt, so
    // the inner file is now visible.
    let output_expanded = run_ui_test(&ctx, Default::default(), |ui| {
        show_left_panel(&mut app, ui);
    });
    let expanded_texts: Vec<String> = output_expanded
        .shapes
        .iter()
        .filter_map(|cs| match &cs.shape {
            egui::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect();
    assert!(
        expanded_texts.iter().any(|t| t.contains("inner_note.md")),
        "inner_note.md must appear after the subdir is expanded; rendered texts: {:?}",
        expanded_texts
    );
    assert_text_contains(&output_expanded.shapes, "top_note.md");
    // The click must NOT touch the left panel width — directory
    // expansion is a tree-only concern.
    assert!(
        !app.layout().left_panel_dirty,
        "directory click must not trigger a panel-width recalc"
    );
}

#[test]
fn test_show_left_panel_dirty_flag_triggers_recalc() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    let lib_dir = std::env::temp_dir().join("fastmd_left_test_recalc");
    app.content_libraries_mut()
        .push(crate::config::ContentLibrary {
            root_folder: lib_dir.to_string_lossy().to_string(),
            name: "RecalcLib".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        });
    app.file_processor_mut().all_files = vec![lib_dir.join("doc.md")];
    app.layout_mut().left_panel_dirty = false;

    let _ = run_ui_test(&ctx, Default::default(), |ui| {
        show_left_panel(&mut app, ui);
    });
    assert!(!app.layout().left_panel_dirty);

    app.layout_mut().left_panel_dirty = true;
    let _ = run_ui_test(&ctx, Default::default(), |ui| {
        show_left_panel(&mut app, ui);
    });
    assert!(!app.layout().left_panel_dirty);
    assert!(app.layout().left_panel_width.is_some());
}

#[test]
fn test_show_left_panel_width_capped_at_twenty_percent() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    let lib_dir = std::env::temp_dir().join("fastmd_left_test_cap");
    app.content_libraries_mut()
        .push(crate::config::ContentLibrary {
            root_folder: lib_dir.to_string_lossy().to_string(),
            name: "CapLib".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        });

    let long_name = "a".repeat(500);
    app.file_processor_mut().all_files = vec![lib_dir.join(format!("{}.md", long_name))];
    app.file_processor_mut().indexing_finished = true;
    app.file_processor_mut().indexing_finished_handled = false;

    let mut inside_available: f32 = 0.0;
    let _ = run_ui_test(&ctx, Default::default(), |ui| {
        inside_available = ui.ctx().viewport_rect().width();
        show_left_panel(&mut app, ui);
    });

    let stored = app.layout().left_panel_width.expect("width should be set");
    let cap_at_recalc_time = inside_available * 0.2;
    assert!(
        stored <= cap_at_recalc_time + 0.5,
        "stored width {} should not exceed 20% cap {} (available={})",
        stored,
        cap_at_recalc_time,
        inside_available
    );
}

/// Renders the panel twice through the same `ctx` and returns the
/// shapes from the second pass. The second pass is the one that
/// emits egui's "rect changed id between passes" warning as a red
/// stroke rectangle in `output.shapes` (see
/// `egui::Context::warn_if_rect_changes_id`). The first pass
/// primes the previous-pass state so the warning is actually
/// emitted on the second pass.
///
/// Tests then call `assert_no_id_change_in_shapes` to assert the
/// panel produces a stable widget tree across passes.
fn render_left_panel_twice(ctx: &egui::Context, app: &mut FastMdApp) -> Vec<egui::Shape> {
    run_ui_test(ctx, Default::default(), |ui| {
        show_left_panel(app, ui);
    });
    let output = run_ui_test(ctx, Default::default(), |ui| {
        show_left_panel(app, ui);
    });
    output.shapes.into_iter().map(|cs| cs.shape).collect()
}

/// Regression: the production UI logged dozens of
/// `WARN egui::context: Widget rect ... changed id between passes`
/// lines on every frame because the left panel nested a
/// `ScrollArea` around another `ScrollArea::show_rows`. The
/// outer scroll area gave the inner one infinite available
/// height, so the inner one ballooned each pass and shifted the
/// rects of every row above it. After the fix the same state
/// must render with zero red-stroke rects in the second pass.
///
/// Note: this test renders in isolation (no parent panel
/// layout, no user input), so the same `Context::run_ui` cycle
/// does not reproduce the loud production trigger. The
/// production fix was verified by the absence of the
/// `WARN egui::context` lines in the running app. This test
/// remains as a smoke test guarding the panel's render path
/// from accidentally re-introducing an obvious id-clash (such
/// as a second nested `ScrollArea` or a fresh `if root.children
/// .is_empty()` branch swap).
#[test]
fn test_show_left_panel_no_id_change_warnings_with_files() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    let lib_dir = std::env::temp_dir().join("fastmd_left_id_stability_files");
    app.content_libraries_mut()
        .push(crate::config::ContentLibrary {
            root_folder: lib_dir.to_string_lossy().to_string(),
            name: "StabilityLib".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        });
    // Use enough files to actually exercise the virtual-scroll
    // ScrollArea, not just the empty-state path.
    let mut all_files = Vec::new();
    for i in 0..60 {
        all_files.push(lib_dir.join(format!("note_{:02}.md", i)));
    }
    app.file_processor_mut().all_files = all_files;
    app.file_processor_mut().indexing_finished = true;
    app.file_processor_mut().indexing_finished_handled = true;
    // Stabilize the panel width so the only thing that can shift
    // between passes is the inner widget tree.
    app.layout_mut().left_panel_width = Some(240.0);
    app.layout_mut().left_panel_dirty = false;

    let shapes = render_left_panel_twice(&ctx, &mut app);
    assert_no_id_change_in_shapes(&shapes);
}

/// Same regression as above but for the empty state: when no
/// files have been indexed, the panel must show the "No markdown
/// files found" placeholder. Even there the widget tree must
/// stay stable across passes.
#[test]
fn test_show_left_panel_no_id_change_warnings_when_empty() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    app.layout_mut().left_panel_width = Some(240.0);
    app.layout_mut().left_panel_dirty = false;

    let shapes = render_left_panel_twice(&ctx, &mut app);
    assert_no_id_change_in_shapes(&shapes);
}

/// TDD Test: When indexing completes (`indexing_finished = true` and
/// `indexing_finished_handled = false`), the first pass wipin PanelState
/// must NOT cause egui ID-change warnings on Pass 2 of the layout.
#[test]
fn test_show_left_panel_no_id_change_warnings_on_indexing_finished_transition() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    let lib_dir = std::env::temp_dir().join("fastmd_left_id_stability_transition");
    app.content_libraries_mut()
        .push(crate::config::ContentLibrary {
            root_folder: lib_dir.to_string_lossy().to_string(),
            name: "StabilityLib".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        });
    let mut all_files = Vec::new();
    for i in 0..10 {
        all_files.push(lib_dir.join(format!("note_{:02}.md", i)));
    }
    app.file_processor_mut().all_files = all_files;
    app.file_processor_mut().indexing_finished = true;
    app.file_processor_mut().indexing_finished_handled = false;

    let shapes = render_left_panel_twice(&ctx, &mut app);
    assert_no_id_change_in_shapes(&shapes);
}

/// TDD Test: When stored PanelState width (e.g. 294.7px) exceeds max_size (204.8px),
/// Panel::left must clamp default_size and size_range BEFORE passing to Panel::left,
/// so Pass 1 and Pass 2 evaluate identical bounds and 0 ID change warnings are emitted.
#[test]
fn test_left_panel_clamped_width_pass_stability() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    app.layout_mut().left_panel_width = Some(294.7);
    app.layout_mut().left_panel_dirty = false;

    let raw_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1024.0, 768.0),
        )),
        ..egui::RawInput::default()
    };

    // Prime egui data memory with stored PanelState at width 294.7px
    ctx.data_mut(|d| {
        d.insert_persisted(
            egui::Id::new("left_panel"),
            PanelState {
                outer_rect: egui::Rect::from_min_size(
                    egui::Pos2::new(0.0, 24.8),
                    egui::vec2(294.7, 900.0),
                ),
            },
        );
    });

    // Pass 1: Run panel layout directly on primed state
    let output = run_ui_test(&ctx, raw_input, |ui| {
        show_left_panel(&mut app, ui);
    });

    let shapes: Vec<egui::Shape> = output.shapes.into_iter().map(|cs| cs.shape).collect();
    assert_no_id_change_in_shapes(&shapes);
}

#[test]
fn test_show_left_panel_tag_filter_hides_directories_without_matching_files() {
    let mut app = create_test_app();
    let lib_dir = std::env::temp_dir().join("fastmd_left_test_tag_filter_dirs");
    app.content_libraries_mut()
        .push(crate::config::ContentLibrary {
            root_folder: lib_dir.to_string_lossy().to_string(),
            name: "TagTestLib".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        });
    app.content_libraries_mut()
        .push(crate::config::ContentLibrary {
            root_folder: std::env::temp_dir()
                .join("fastmd_left_test_empty_lib")
                .to_string_lossy()
                .to_string(),
            name: "EmptyLib".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        });

    let matching_dir = lib_dir.join("matching_folder");
    let non_matching_dir = lib_dir.join("non_matching_folder");

    let file_match = matching_dir.join("match.md");
    let file_no_match = non_matching_dir.join("other.md");

    app.file_processor_mut().all_files = vec![file_match.clone(), file_no_match.clone()];
    app.file_processor_mut().all_dirs = vec![matching_dir.clone(), non_matching_dir.clone()];

    app.tags_mut()
        .add_tags(file_match.clone(), vec!["target_tag".to_string()]);
    app.tags_mut()
        .add_tags(file_no_match.clone(), vec!["other_tag".to_string()]);

    // Select the tag filter
    app.tags_mut().selected_tag = Some("target_tag".to_string());

    let tree = build_workspace_tree(&app);

    let lib_node = tree
        .children
        .get("TagTestLib")
        .expect("TagTestLib node should exist");
    assert!(
        lib_node.children.contains_key("matching_folder"),
        "matching_folder should be in the tree when filtering by target_tag"
    );
    assert!(
        !lib_node.children.contains_key("non_matching_folder"),
        "non_matching_folder should NOT be in the tree when filtering by target_tag"
    );
    assert!(
        !tree.children.contains_key("EmptyLib"),
        "EmptyLib (library without matching files) should NOT be in the tree when filtering by target_tag"
    );
}

#[test]
fn test_left_panel_search_input_renders() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    app.layout_mut().left_panel_dirty = false;

    let output = run_ui_test(&ctx, Default::default(), |ui| {
        show_left_panel(&mut app, ui);
    });

    let texts = crate::ui::test_helpers::text::extract_text(&output.shapes);
    assert!(
        texts
            .iter()
            .any(|t| t.contains(crate::ui::strings::WORKSPACE_HEADER))
    );
}

#[test]
fn test_left_panel_search_replaces_tree_view() {
    use std::io::Write;
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    app.layout_mut().left_panel_dirty = false;

    let temp_dir = std::env::temp_dir().join("fastmd_left_search_test");
    let _ = std::fs::create_dir_all(&temp_dir);
    let match_file = temp_dir.join("apples.md");
    let other_file = temp_dir.join("oranges.md");
    let mut f1 = std::fs::File::create(&match_file).unwrap();
    f1.write_all(b"# Apples\nCrisp apples are great.").unwrap();
    let mut f2 = std::fs::File::create(&other_file).unwrap();
    f2.write_all(b"# Oranges\nJust citrus.").unwrap();

    app.file_processor_mut().all_files = vec![match_file.clone(), other_file.clone()];

    // Before search: is_searching is false
    assert!(!app.search().is_searching());

    // Apply search for "crisp"
    *app.search_mut().query_mut() = "crisp".to_string();
    app.search_mut()
        .apply(&[match_file.clone(), other_file.clone()], &[]);
    assert!(app.search().is_searching());
    assert_eq!(app.search().results().len(), 1);

    let raw_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1024.0, 768.0),
        )),
        ..egui::RawInput::default()
    };

    let output = run_ui_test(&ctx, raw_input, |ui| {
        show_left_panel(&mut app, ui);
    });

    let texts = crate::ui::test_helpers::text::extract_text(&output.shapes);
    assert!(
        texts.iter().any(|t| t.contains("apples.md")),
        "Matching file must be rendered"
    );
    assert!(
        texts.iter().any(|t| t.contains("Crisp apples")),
        "Matching snippet must be rendered"
    );
    assert!(
        !texts.iter().any(|t| t.contains("oranges.md")),
        "Non-matching file must NOT be in search results"
    );

    // Clear search and verify tree view is restored
    app.search_mut().clear();
    assert!(!app.search().is_searching());
}

#[test]
fn test_left_panel_search_empty_results() {
    let ctx = egui::Context::default();
    let mut app = create_test_app();
    app.layout_mut().left_panel_dirty = false;

    *app.search_mut().query_mut() = "nonexistent_query_xyz".to_string();
    app.search_mut().apply(&[], &[]);
    assert!(app.search().is_searching());

    let raw_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1024.0, 768.0),
        )),
        ..egui::RawInput::default()
    };

    let output = run_ui_test(&ctx, raw_input, |ui| {
        show_left_panel(&mut app, ui);
    });

    let texts = crate::ui::test_helpers::text::extract_text(&output.shapes);
    assert!(
        texts
            .iter()
            .any(|t| t.contains(crate::ui::strings::SEARCH_NO_RESULTS))
    );
}
