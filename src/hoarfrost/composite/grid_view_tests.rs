//! Unit tests for the 2D virtualized tile/card grid composite (`UIF-COMP-013`, `UIF-VER-002`, `UIF-PRIN-006`).

use super::*;
use crate::test_helpers::Harness;
use crate::token::contrast::contrast_ratio;
use iced::advanced::Widget;
use iced::advanced::layout;
use iced::advanced::widget::tree::Tree;
use iced::keyboard::Key;
use iced::keyboard::key::Named;
use iced::mouse;
use std::collections::BTreeSet;

fn sample_items() -> Vec<GridTile<usize>> {
    (0..8)
        .map(|i| {
            GridTile::new(i, format!("Tile {i}"))
                .subtitle(format!("Subtitle {i}"))
                .preview_icon(Icon::new("📁"))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// 1. Column Solver Tests (§3B.13, UIF-VER-002)
// ---------------------------------------------------------------------------

#[test]
fn test_solve_columns_narrow_clamps_to_single_column() {
    // Narrower than min_tile_width (160) + gutter (10)
    let (cols, width) = solve_columns(100.0, DEFAULT_MIN_TILE_WIDTH, GAP, GUTTER_WIDTH);
    assert_eq!(cols, 1);
    assert_eq!(width, DEFAULT_MIN_TILE_WIDTH);

    // Exact min_tile_width + gutter
    let (cols, width) = solve_columns(170.0, DEFAULT_MIN_TILE_WIDTH, GAP, GUTTER_WIDTH);
    assert_eq!(cols, 1);
    assert_eq!(width, DEFAULT_MIN_TILE_WIDTH);
}

#[test]
fn test_solve_columns_two_columns_calculation() {
    // 500px width: gutter is 10px -> effective = 490px.
    // (490 + 12) / (160 + 12) = 502 / 172 = 2.918 -> floor = 2 cols.
    // tile_width = (490 - 12) / 2 = 239.0px.
    let (cols, width) = solve_columns(500.0, DEFAULT_MIN_TILE_WIDTH, GAP, GUTTER_WIDTH);
    assert_eq!(cols, 2);
    assert!((width - 239.0).abs() < 1e-4);

    // Verify exact closure of width
    let total = cols as f32 * width + (cols - 1) as f32 * GAP;
    assert!((total - 490.0).abs() < 1e-4);
}

#[test]
fn test_solve_columns_wide_viewport() {
    // 1000px width: gutter 10px -> effective = 990px.
    // (990 + 12) / (160 + 12) = 1002 / 172 = 5.825 -> floor = 5 cols.
    // tile_width = (990 - 4 * 12) / 5 = (990 - 48) / 5 = 942 / 5 = 188.4px.
    let (cols, width) = solve_columns(1000.0, DEFAULT_MIN_TILE_WIDTH, GAP, GUTTER_WIDTH);
    assert_eq!(cols, 5);
    assert!((width - 188.4).abs() < 1e-4);

    let total = cols as f32 * width + (cols - 1) as f32 * GAP;
    assert!((total - 990.0).abs() < 1e-4);
}

#[test]
fn test_tile_rect_calculation() {
    let origin = Point::new(10.0, 20.0);
    let cols = 3;
    let tile_w = 200.0;
    let tile_h = 100.0;
    let gap = 12.0;

    // Col 0, Row 0
    let r0 = tile_rect(origin, 0, cols, tile_w, tile_h, gap);
    assert_eq!(
        r0,
        Rectangle::new(Point::new(10.0, 20.0), Size::new(200.0, 100.0))
    );

    // Col 1, Row 0
    let r1 = tile_rect(origin, 1, cols, tile_w, tile_h, gap);
    assert_eq!(
        r1,
        Rectangle::new(Point::new(222.0, 20.0), Size::new(200.0, 100.0))
    );

    // Col 2, Row 0
    let r2 = tile_rect(origin, 2, cols, tile_w, tile_h, gap);
    assert_eq!(
        r2,
        Rectangle::new(Point::new(434.0, 20.0), Size::new(200.0, 100.0))
    );

    // Col 0, Row 1 (index 3)
    let r3 = tile_rect(origin, 3, cols, tile_w, tile_h, gap);
    assert_eq!(
        r3,
        Rectangle::new(Point::new(10.0, 132.0), Size::new(200.0, 100.0))
    );

    // Col 2, Row 1 (index 5)
    let r5 = tile_rect(origin, 5, cols, tile_w, tile_h, gap);
    assert_eq!(
        r5,
        Rectangle::new(Point::new(434.0, 132.0), Size::new(200.0, 100.0))
    );
}

// ---------------------------------------------------------------------------
// 2. 2D Navigation Clamping Tests (§3B.13)
// ---------------------------------------------------------------------------

#[test]
fn test_2d_navigation_clamps_and_does_not_wrap() {
    let model = GridViewModel::new(sample_items()); // 8 items: indices 0..=7
    let cols = 3;
    // Row 0: 0, 1, 2
    // Row 1: 3, 4, 5
    // Row 2: 6, 7

    // Left clamps at 0
    assert_eq!(model.step_left(0, cols), 0);
    assert_eq!(model.step_left(1, cols), 0);

    // Right clamps at last index (7)
    assert_eq!(model.step_right(6, cols), 7);
    assert_eq!(model.step_right(7, cols), 7);

    // Up clamps at 0 and does not wrap to bottom row
    assert_eq!(model.step_up(0, cols), 0);
    assert_eq!(model.step_up(1, cols), 0);
    assert_eq!(model.step_up(2, cols), 0);
    assert_eq!(model.step_up(4, cols), 1);
    assert_eq!(model.step_up(6, cols), 3);

    // Down clamps at last item and does not wrap to top
    assert_eq!(model.step_down(0, cols), 3);
    assert_eq!(model.step_down(3, cols), 6);
    assert_eq!(model.step_down(4, cols), 7);
    assert_eq!(model.step_down(5, cols), 7); // 5 + 3 = 8 -> clamped to 7
    assert_eq!(model.step_down(6, cols), 7);
    assert_eq!(model.step_down(7, cols), 7);

    // Home and End
    assert_eq!(model.step_home(), 0);
    assert_eq!(model.step_end(), 7);
}

#[test]
fn test_2d_navigation_by_id() {
    let model = GridViewModel::new(sample_items());
    let cols = 3;

    assert_eq!(model.step_from_id(&0, cols, GridNavKey::Right), Some(1));
    assert_eq!(model.step_from_id(&1, cols, GridNavKey::Down), Some(4));
    assert_eq!(model.step_from_id(&4, cols, GridNavKey::Left), Some(3));
    assert_eq!(model.step_from_id(&3, cols, GridNavKey::Up), Some(0));
    assert_eq!(model.step_from_id(&4, cols, GridNavKey::End), Some(7));
    assert_eq!(model.step_from_id(&7, cols, GridNavKey::Home), Some(0));

    // Unknown ID returns None
    assert_eq!(model.step_from_id(&999, cols, GridNavKey::Down), None);
}

// ---------------------------------------------------------------------------
// 3. Selection Mechanics Tests (§3B.6, §3B.13)
// ---------------------------------------------------------------------------

#[test]
fn test_single_selection_mode() {
    let mut model = GridViewModel::new(sample_items());
    assert_eq!(model.selection_mode, TreeSelectionMode::Single);

    // Select single
    let changed = model.select_single(2);
    assert!(changed);
    assert_eq!(model.selected_ids, BTreeSet::from([2]));
    assert_eq!(model.anchor_id, Some(2));
    assert_eq!(model.focused_id, Some(2));

    // Replace selection
    let changed = model.select_single(5);
    assert!(changed);
    assert_eq!(model.selected_ids, BTreeSet::from([5]));
    assert_eq!(model.anchor_id, Some(5));

    // Toggle in Single mode replaces selection
    model.toggle_selection(3);
    assert_eq!(model.selected_ids, BTreeSet::from([3]));

    // Range in Single mode degenerates to single selection
    model.select_range_to(6);
    assert_eq!(model.selected_ids, BTreeSet::from([6]));

    // Select all is a no-op in Single mode
    assert!(!model.select_all());
    assert_eq!(model.selected_ids, BTreeSet::from([6]));

    // Clear selection
    assert!(model.clear_selection());
    assert!(model.selected_ids.is_empty());
}

#[test]
fn test_extended_multi_selection_mode() {
    let mut model =
        GridViewModel::new(sample_items()).selection_mode(TreeSelectionMode::ExtendedMulti);

    // 1. Initial single select
    model.select_single(2);
    assert_eq!(model.selected_ids, BTreeSet::from([2]));
    assert_eq!(model.anchor_id, Some(2));

    // 2. Shift range forward (2 to 5)
    model.select_range_to(5);
    assert_eq!(model.selected_ids, BTreeSet::from([2, 3, 4, 5]));
    assert_eq!(model.anchor_id, Some(2)); // Anchor stays at 2
    assert_eq!(model.focused_id, Some(5));

    // 3. Shift range backward (2 to 0)
    model.select_range_to(0);
    assert_eq!(model.selected_ids, BTreeSet::from([0, 1, 2]));

    // 4. Ctrl toggle item
    model.toggle_selection(6);
    assert_eq!(model.selected_ids, BTreeSet::from([0, 1, 2, 6]));
    assert_eq!(model.anchor_id, Some(6));

    // Toggle off
    model.toggle_selection(1);
    assert_eq!(model.selected_ids, BTreeSet::from([0, 2, 6]));

    // 5. Select all
    assert!(model.select_all());
    assert_eq!(model.selected_ids.len(), 8);

    // 6. Clear
    assert!(model.clear_selection());
    assert!(model.selected_ids.is_empty());
}

// ---------------------------------------------------------------------------
// 4. Contrast Requirements Tests (§3B.13, UIF-PRIN-006)
// ---------------------------------------------------------------------------

#[test]
fn test_selection_contrast_ratios() {
    let palette = Palette::dark();

    // 1. Selected border vs surface.active >= 3.0:1 (spec states 4.79:1)
    let border_contrast = contrast_ratio(palette.accent_border, palette.surface_active);
    assert!(
        border_contrast >= 3.0,
        "accent.border on surface.active contrast {border_contrast:.2} must be >= 3.0:1"
    );

    // 2. Selected text.strong vs surface.active >= 4.5:1 (spec states 14.29:1)
    let text_contrast = contrast_ratio(palette.text_strong, palette.surface_active);
    assert!(
        text_contrast >= 4.5,
        "text.strong on surface.active contrast {text_contrast:.2} must be >= 4.5:1"
    );
}

// ---------------------------------------------------------------------------
// 5. Headless Widget Layout & Rendering Passes
// ---------------------------------------------------------------------------

#[test]
fn test_grid_view_widget_layout_geometry() {
    let items = sample_items(); // 8 items
    let model = GridViewModel::new(items)
        .min_tile_width(160.0)
        .tile_height(120.0);
    let mut widget = GridView::<(), usize>::new(model, 500.0);

    let mut tree = Tree::new(&widget as &dyn Widget<(), iced::Theme, iced::Renderer>);
    let limits = layout::Limits::NONE.max_width(500.0).max_height(2000.0);
    let renderer = crate::test_helpers::headless_renderer();

    let node = widget.layout(&mut tree, &renderer, &limits);
    let size = node.size();

    // cols = 2 for 500px width; 8 items / 2 cols = 4 rows.
    // total_height = 4 * 120 + 3 * 12 = 480 + 36 = 516.0.
    assert_eq!(size.width, 500.0);
    assert_eq!(size.height, 516.0);
}

#[test]
fn test_grid_view_widget_headless_render_passes() {
    // 1. Normal multi-tile render
    let model = GridViewModel::new(sample_items()).selection_mode(TreeSelectionMode::ExtendedMulti);
    let mut harness = Harness::<(), _>::headless(move || grid_view(model.clone(), 600.0).into());
    harness.render();

    // 2. Empty grid render
    let empty_model = GridViewModel::<usize>::new(Vec::new());
    let mut empty_harness =
        Harness::<(), _>::headless(move || grid_view(empty_model.clone(), 600.0).into());
    empty_harness.render();

    // 3. Render with selection and disabled item
    let mut selected_items = sample_items();
    selected_items[1] = selected_items[1].clone().disabled(true);
    let mut selected_model = GridViewModel::new(selected_items);
    selected_model.select_single(0);
    selected_model.selected_ids.insert(3);
    let mut sel_harness =
        Harness::<(), _>::headless(move || grid_view(selected_model.clone(), 600.0).into());
    sel_harness.render();
}

#[test]
fn test_grid_view_event_selection_callback() {
    let items = sample_items();
    let model = GridViewModel::new(items).selection_mode(TreeSelectionMode::ExtendedMulti);
    let mut widget = GridView::<usize, usize>::new(model, 500.0)
        .on_selection_change(|commit| commit.into_value().len())
        .on_activate(|commit| commit.into_value());

    let mut tree = Tree::new(&widget as &dyn Widget<usize, iced::Theme, iced::Renderer>);
    let limits = layout::Limits::NONE.max_width(500.0).max_height(2000.0);
    let renderer = crate::test_helpers::headless_renderer();
    let layout_node = widget.layout(&mut tree, &renderer, &limits);
    let layout = Layout::new(&layout_node);

    let mut messages = Vec::new();
    let mut clipboard = iced::advanced::clipboard::Null;

    // Simulate clicking tile 1 (located at x ~ 256.0, y ~ 20.0)
    let click_event = iced::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
    let cursor = mouse::Cursor::Available(Point::new(260.0, 30.0));
    let viewport = Rectangle::new(Point::ORIGIN, Size::new(500.0, 1000.0));

    // First update cursor position
    let move_event = iced::Event::Mouse(mouse::Event::CursorMoved {
        position: Point::new(260.0, 30.0),
    });

    {
        let mut shell = Shell::new(&mut messages);
        widget.update(
            &mut tree,
            &move_event,
            layout,
            cursor,
            &renderer,
            &mut clipboard,
            &mut shell,
            &viewport,
        );

        // Then click
        widget.update(
            &mut tree,
            &click_event,
            layout,
            cursor,
            &renderer,
            &mut clipboard,
            &mut shell,
            &viewport,
        );
    }

    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0], 1); // 1 item selected

    // Press Enter to activate focused tile
    let enter_event = iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
        key: Key::Named(Named::Enter),
        modified_key: Key::Named(Named::Enter),
        physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::Enter),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::NONE,
        text: None,
        repeat: false,
    });

    {
        let mut shell = Shell::new(&mut messages);
        widget.update(
            &mut tree,
            &enter_event,
            layout,
            cursor,
            &renderer,
            &mut clipboard,
            &mut shell,
            &viewport,
        );
    }

    assert_eq!(messages.len(), 2);
    assert_eq!(messages[1], 1); // Activated tile 1
}
