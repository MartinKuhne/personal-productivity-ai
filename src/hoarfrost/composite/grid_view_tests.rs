//! Unit tests for the 2D virtualized tile/card grid composite (`UIF-COMP-013`, `UIF-VER-002`, `UIF-PRIN-006`).

use super::*;
use crate::test_helpers::{
    Capture, Frame, Harness, PIXEL_SCALE, SURFACE_TOLERANCE, assert_repainted, centre,
    headless_renderer, moved,
};
use crate::token::contrast::contrast_ratio;
use iced::Color;
use iced::advanced::Widget;
use iced::advanced::layout;
use iced::advanced::renderer::Headless as _;
use iced::advanced::widget::tree::Tree;
use iced::keyboard::Key;
use iced::keyboard::Modifiers;
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

// ---------------------------------------------------------------------------
// 6. Solver & Model Corner Cases
// ---------------------------------------------------------------------------

#[test]
fn test_solve_columns_clamps_degenerate_inputs() {
    // Nothing to work with: the minimum tile is floored at one pixel and the
    // grid still resolves to exactly one column.
    let (cols, width) = solve_columns(0.0, 0.0, -GAP, -GUTTER_WIDTH);
    assert_eq!(cols, 1, "a zero-width request still needs one column");
    assert_eq!(width, 1.0, "a zero minimum tile is floored at one pixel");

    // A negative width cannot deduct a gutter, so the minimum tile wins.
    let (cols, width) = solve_columns(-100.0, DEFAULT_MIN_TILE_WIDTH, GAP, GUTTER_WIDTH);
    assert_eq!(cols, 1, "a negative width cannot fit more than one tile");
    assert_eq!(width, DEFAULT_MIN_TILE_WIDTH);

    // A negative gap is clamped to zero, so the tiles pack tighter and wider.
    let (cols, width) = solve_columns(500.0, DEFAULT_MIN_TILE_WIDTH, -GAP, GUTTER_WIDTH);
    assert_eq!(cols, 3, "with no gaps to reserve, a third tile fits");
    assert!(
        (cols as f32 * width - 490.0).abs() < 1e-4,
        "with the gap clamped away the tiles must fill the whole effective width, got {width}"
    );

    // A negative gutter is clamped to zero, so nothing is deducted up front.
    let (cols, width) = solve_columns(500.0, DEFAULT_MIN_TILE_WIDTH, GAP, -GUTTER_WIDTH);
    assert_eq!(cols, 2);
    assert!(
        (cols as f32 * width + (cols - 1) as f32 * GAP - 500.0).abs() < 1e-4,
        "a clamped gutter means the full 500px is handed to the tiles"
    );
}

#[test]
fn test_tile_rect_treats_zero_columns_as_one() {
    let rect = tile_rect(Point::ORIGIN, 5, 0, 100.0, 50.0, 0.0);
    assert_eq!(
        rect.x, 0.0,
        "with no columns every tile sits in column zero"
    );
    assert_eq!(rect.y, 250.0, "and the grid stacks into a single column");
}

#[test]
fn test_step_nav_survives_an_empty_grid_and_a_stale_index() {
    let empty = GridViewModel::<usize>::new(Vec::new());
    for key in [
        GridNavKey::Left,
        GridNavKey::Right,
        GridNavKey::Up,
        GridNavKey::Down,
        GridNavKey::Home,
        GridNavKey::End,
    ] {
        assert_eq!(
            empty.step_nav(3, 2, key),
            0,
            "a grid with no tiles has nowhere to step to"
        );
    }

    let model = GridViewModel::new(sample_items()); // 8 items, indices 0..=7

    // An index past the end is clamped before it is stepped.
    assert_eq!(
        model.step_nav(99, 3, GridNavKey::Left),
        6,
        "a stale index is clamped to the last tile before stepping left"
    );
    assert_eq!(model.step_nav(99, 3, GridNavKey::Right), 7);
    assert_eq!(model.step_nav(99, 3, GridNavKey::Up), 4);
    assert_eq!(model.step_nav(99, 3, GridNavKey::Down), 7);

    // Zero columns behave as one column, not as a divide-by-zero.
    assert_eq!(
        model.step_nav(4, 0, GridNavKey::Up),
        3,
        "zero columns mean one column"
    );
    assert_eq!(model.step_nav(4, 0, GridNavKey::Down), 5);
}

#[test]
fn test_index_lookup_and_the_default_model() {
    let model = GridViewModel::new(sample_items());
    assert_eq!(model.index_of(&3), Some(3));
    assert_eq!(
        model.index_of(&99),
        None,
        "an id that is not in the grid has no index"
    );
    assert_eq!(model.id_at(7), Some(7));
    assert_eq!(model.id_at(8), None, "there is no tile past the last one");

    let default = GridViewModel::<usize>::default();
    assert!(default.items.is_empty(), "the default grid shows nothing");
    assert!(default.selected_ids.is_empty());
    assert_eq!(default.anchor_id, None);
    assert_eq!(default.focused_id, None);
    assert_eq!(default.min_tile_width, DEFAULT_MIN_TILE_WIDTH);
    assert_eq!(default.tile_height, DEFAULT_TILE_HEIGHT);
    assert_eq!(default.selection_mode, TreeSelectionMode::Single);
}

#[test]
fn test_grid_tile_builders_fill_in_the_optional_parts() {
    let bare = GridTile::new(7, "Report");
    assert_eq!(bare.id, 7);
    assert_eq!(bare.title, "Report");
    assert_eq!(bare.subtitle, None, "a bare tile carries no subtitle");
    assert_eq!(bare.preview_icon, None, "and no preview icon");
    assert!(!bare.disabled, "and is interactive by default");

    let full = bare
        .clone()
        .subtitle("Q3")
        .preview_icon(Icon::new("★"))
        .disabled(true);
    assert_eq!(full.subtitle.as_deref(), Some("Q3"));
    assert_eq!(full.preview_icon, Some(Icon::new("★")));
    assert!(full.disabled);
    assert_eq!(
        full.id, bare.id,
        "the builders only add, they never replace the tile"
    );
}

#[test]
fn test_selection_mutations_report_whether_they_changed_anything() {
    let mut single = GridViewModel::new(sample_items());
    assert!(single.select_single(2));
    assert!(
        !single.select_single(2),
        "re-selecting the only selected tile changes nothing"
    );
    assert!(single.clear_selection());
    assert!(
        !single.clear_selection(),
        "clearing an already empty selection changes nothing"
    );
    assert!(
        single.toggle_selection(1),
        "a first toggle in single mode does change the selection"
    );
    assert!(
        !single.toggle_selection(1),
        "in single mode a toggle on the selected tile cannot deselect it"
    );

    let mut multi =
        GridViewModel::new(sample_items()).selection_mode(TreeSelectionMode::ExtendedMulti);
    assert!(
        multi.select_range_to(4),
        "a range with no anchor degenerates to the one tile it names"
    );
    assert_eq!(multi.selected_ids, BTreeSet::from([4]));
    assert_eq!(
        multi.anchor_id, None,
        "a range with no anchor does not invent one"
    );
    assert!(
        !multi.select_range_to(4),
        "the same range a second time changes nothing"
    );
    assert!(
        !multi.select_range_to(99),
        "an id that is not in the grid selects nothing"
    );
    assert_eq!(multi.selected_ids, BTreeSet::from([4]));
    assert!(multi.select_all());
    assert!(
        !multi.select_all(),
        "selecting everything twice changes nothing"
    );
    assert!(multi.clear_selection());
    assert!(
        !multi.clear_selection(),
        "clearing an empty selection changes nothing"
    );
}

// ---------------------------------------------------------------------------
// 7. Layout Protocol
// ---------------------------------------------------------------------------

/// The height a grid lays out to at `width`.
fn laid_out_height(model: &GridViewModel<usize>, width: f32, renderer: &iced::Renderer) -> f32 {
    let mut widget = GridView::<GridMsg, usize>::new(model.clone(), width);
    let mut tree = Tree::new(&widget as &dyn Widget<GridMsg, iced::Theme, iced::Renderer>);
    let limits = layout::Limits::NONE
        .max_width(width.max(1.0))
        .max_height(5_000.0);

    widget.layout(&mut tree, renderer, &limits).size().height
}

#[test]
fn test_size_reports_a_fixed_width_only_when_one_was_given() {
    let fixed = GridView::<GridMsg, usize>::new(GridViewModel::new(sample_items()), 400.0);
    assert_eq!(fixed.size().width, Length::Fixed(400.0));
    assert_eq!(
        fixed.size().height,
        Length::Shrink,
        "the grid is always as tall as its rows"
    );

    let fluid = GridView::<GridMsg, usize>::new(GridViewModel::new(sample_items()), 0.0);
    assert_eq!(
        fluid.size().width,
        Length::Fill,
        "a zero width means 'take whatever the parent offers'"
    );

    let negative = GridView::<GridMsg, usize>::new(GridViewModel::new(sample_items()), -20.0);
    assert_eq!(
        negative.size().width,
        Length::Fill,
        "a negative width is not a fixed width either"
    );
}

#[test]
fn test_layout_uses_the_room_it_is_given_when_no_width_was_asked_for() {
    let renderer = headless_renderer();
    let model = GridViewModel::new(sample_items());

    let empty = GridViewModel::<usize>::new(Vec::new());
    assert_eq!(
        laid_out_height(&empty, 400.0, &renderer),
        0.0,
        "no tiles means no rows and no height"
    );

    assert_eq!(
        laid_out_height(&model, 300.0, &renderer),
        8.0 * DEFAULT_TILE_HEIGHT + 7.0 * GAP,
        "300px fits a single column, so eight tiles stack into eight rows"
    );

    assert_eq!(
        laid_out_height(&model, 60.0, &renderer),
        8.0 * DEFAULT_TILE_HEIGHT + 7.0 * GAP,
        "a viewport narrower than one tile still gets a full tile, never a squeezed one"
    );
}

#[test]
fn test_a_fifth_tile_opens_a_third_row() {
    let renderer = headless_renderer();
    let items = sample_items();
    let four = GridViewModel::new(items[..4].to_vec());
    let five = GridViewModel::new(items[..5].to_vec());

    assert_eq!(
        laid_out_height(&five, 400.0, &renderer) - laid_out_height(&four, 400.0, &renderer),
        DEFAULT_TILE_HEIGHT + GAP,
        "at two columns a fifth tile opens a whole new row, separated by one gap"
    );
}

// ---------------------------------------------------------------------------
// 8. The driven widget
// ---------------------------------------------------------------------------

/// The width every driven grid is laid out at.
const DRIVE_WIDTH: f32 = 400.0;

/// The frame the culling test rasterises into — deliberately taller than the
/// grid it draws, so a viewport can stop in the middle of it.
const TALL_FRAME: Size = Size::new(400.0, 600.0);

/// Everything a wired grid is allowed to publish.
#[derive(Debug, Clone, PartialEq)]
enum GridMsg {
    /// The whole selection, after it changed.
    Selection(BTreeSet<usize>),
    /// The tile `Enter` activated.
    Activate(usize),
    /// The selection and the point a context menu was asked for.
    Context(BTreeSet<usize>, Point),
}

/// Shorthand for a selection commit over the ids a test names.
fn selection(ids: impl IntoIterator<Item = usize>) -> GridMsg {
    GridMsg::Selection(ids.into_iter().collect())
}

/// A grid with every callback wired, so each branch reports which one fired.
fn wired(model: GridViewModel<usize>) -> GridView<'static, GridMsg, usize> {
    GridView::new(model, DRIVE_WIDTH)
        .on_selection_change(|commit| GridMsg::Selection(commit.into_value()))
        .on_activate(|commit| GridMsg::Activate(commit.into_value()))
        .on_context_menu(|commit| {
            let (ids, at) = commit.into_value();
            GridMsg::Context(ids, at)
        })
}

/// A model with `focused_id` preset and nothing selected.
fn focused_model(focused: Option<usize>) -> GridViewModel<usize> {
    let mut model = GridViewModel::new(sample_items());
    model.focused_id = focused;
    model
}

/// The column geometry `model` resolves to at `width`.
fn grid_columns(model: &GridViewModel<usize>, width: f32) -> (usize, f32) {
    solve_columns(width, model.min_tile_width, GAP, GUTTER_WIDTH)
}

/// The box tile `index` occupies in a grid laid out at the origin.
fn tile_at(model: &GridViewModel<usize>, width: f32, index: usize) -> Rectangle {
    let (cols, tile_width) = grid_columns(model, width);
    tile_rect(
        Point::ORIGIN,
        index,
        cols,
        tile_width,
        model.tile_height,
        GAP,
    )
}

/// A laid-out grid driven event by event, the way the runtime drives one.
///
/// [`Harness`] only delivers pointer events, and every keyboard branch of
/// [`GridView::update`] reads the modifiers carried by its own `KeyPressed`.
struct Driver {
    widget: GridView<'static, GridMsg, usize>,
    tree: Tree,
    node: layout::Node,
    renderer: iced::Renderer,
}

impl Driver {
    fn new(mut widget: GridView<'static, GridMsg, usize>) -> Self {
        let width = widget.available_width();
        let renderer = headless_renderer();
        let mut tree = Tree::new(&widget as &dyn Widget<GridMsg, iced::Theme, iced::Renderer>);
        let limits = layout::Limits::NONE
            .max_width(width.max(1.0))
            .max_height(5_000.0);
        let node = widget.layout(&mut tree, &renderer, &limits);

        Self {
            widget,
            tree,
            node,
            renderer,
        }
    }

    fn model(&self) -> &GridViewModel<usize> {
        self.widget.model()
    }

    /// Deliver one event with the cursor where the test says, and report both
    /// what the grid published and whether it swallowed the event.
    fn send(&mut self, event: iced::Event, cursor: mouse::Cursor) -> (Vec<GridMsg>, bool) {
        let mut messages = Vec::new();
        let mut clipboard = iced::advanced::clipboard::Null;
        let layout = Layout::new(&self.node);
        let viewport = Rectangle::with_size(self.node.size());

        let captured;
        {
            let mut shell = Shell::new(&mut messages);
            self.widget.update(
                &mut self.tree,
                &event,
                layout,
                cursor,
                &self.renderer,
                &mut clipboard,
                &mut shell,
                &viewport,
            );
            captured = shell.is_event_captured();
        }

        (messages, captured)
    }

    fn key(&mut self, key: Key, modifiers: Modifiers) -> (Vec<GridMsg>, bool) {
        self.send(
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                modified_key: key.clone(),
                physical_key: iced::keyboard::key::Physical::Code(iced::keyboard::key::Code::Space),
                location: iced::keyboard::Location::Standard,
                key,
                modifiers,
                text: None,
                repeat: false,
            }),
            mouse::Cursor::Unavailable,
        )
    }

    /// Push a modifier state the way a real keyboard does, ahead of a click.
    fn hold_modifiers(&mut self, modifiers: Modifiers) -> (Vec<GridMsg>, bool) {
        self.send(
            iced::Event::Keyboard(iced::keyboard::Event::ModifiersChanged(modifiers)),
            mouse::Cursor::Unavailable,
        )
    }

    fn press_at(&mut self, at: Point, button: mouse::Button) -> (Vec<GridMsg>, bool) {
        self.send(
            iced::Event::Mouse(mouse::Event::ButtonPressed(button)),
            mouse::Cursor::Available(at),
        )
    }

    fn left(&mut self, at: Point) -> (Vec<GridMsg>, bool) {
        self.press_at(at, mouse::Button::Left)
    }

    fn right(&mut self, at: Point) -> (Vec<GridMsg>, bool) {
        self.press_at(at, mouse::Button::Right)
    }

    /// The centre of tile `index`.
    fn tile(&self, index: usize) -> Point {
        centre(tile_at(self.model(), self.widget.available_width(), index))
    }

    /// The centre of tile `index`'s checkbox overlay.
    fn checkbox(&self, index: usize) -> Point {
        let rect = tile_at(self.model(), self.widget.available_width(), index);
        centre(Rectangle::new(
            Point::new(rect.x + CHECKBOX_INSET, rect.y + CHECKBOX_INSET),
            Size::new(CHECKBOX_SIZE, CHECKBOX_SIZE),
        ))
    }
}

#[test]
fn test_modifier_state_decides_what_a_click_does() {
    let mut model =
        GridViewModel::new(sample_items()).selection_mode(TreeSelectionMode::ExtendedMulti);
    model.select_single(0);
    let mut driver = Driver::new(wired(model));

    assert_eq!(
        driver.hold_modifiers(Modifiers::CTRL),
        (vec![], false),
        "tracking a modifier publishes nothing and swallows nothing"
    );

    let at = driver.tile(2);
    assert_eq!(
        driver.left(at).0,
        vec![selection([0, 2])],
        "a Ctrl+click adds the tile instead of replacing the selection"
    );

    driver.hold_modifiers(Modifiers::SHIFT);
    let at = driver.tile(4);
    assert_eq!(
        driver.left(at).0,
        vec![selection([2, 3, 4])],
        "Shift+click ranges from the anchor the toggle left behind"
    );

    driver.hold_modifiers(Modifiers::NONE);
    let at = driver.tile(0);
    assert_eq!(
        driver.left(at).0,
        vec![selection([0])],
        "once the modifiers are released a click replaces the selection again"
    );
}

#[test]
fn test_ctrl_a_selects_every_tile_only_in_multi_mode() {
    let items = sample_items();
    let multi = GridViewModel::new(items.clone()).selection_mode(TreeSelectionMode::ExtendedMulti);
    let mut driver = Driver::new(wired(multi));

    assert_eq!(
        driver.key(Key::Character("a".into()), Modifiers::CTRL),
        (vec![selection(0..8)], true),
        "Ctrl+A commits the whole grid at once and swallows the shortcut"
    );
    assert_eq!(driver.model().selected_ids.len(), 8);

    assert_eq!(
        driver.key(Key::Character("A".into()), Modifiers::CTRL),
        (vec![], true),
        "the shortcut is case-insensitive, and selecting what is already selected commits nothing"
    );

    let mut single = Driver::new(wired(GridViewModel::new(items)));
    assert_eq!(
        single.key(Key::Character("a".into()), Modifiers::CTRL),
        (vec![], false),
        "a single-select grid has no 'select all' to offer, so it must leave Ctrl+A alone"
    );
    assert!(single.model().selected_ids.is_empty());
}

#[test]
fn test_enter_activates_only_a_focused_tile_with_a_listener() {
    let mut driver = Driver::new(wired(focused_model(Some(3))));
    assert_eq!(
        driver.key(Key::Named(Named::Enter), Modifiers::NONE),
        (vec![GridMsg::Activate(3)], true),
        "Enter activates the focused tile and keeps the key for itself"
    );

    let mut silent = Driver::new(GridView::new(focused_model(Some(3)), DRIVE_WIDTH));
    assert_eq!(
        silent.key(Key::Named(Named::Enter), Modifiers::NONE),
        (vec![], false),
        "with nobody listening, Enter must neither publish nor be swallowed"
    );

    let mut unfocused = Driver::new(wired(focused_model(None)));
    assert_eq!(
        unfocused.key(Key::Named(Named::Enter), Modifiers::NONE),
        (vec![], false),
        "with nothing focused there is no id to commit"
    );
}

#[test]
fn test_space_toggles_in_multi_and_replaces_in_single() {
    let mut multi =
        GridViewModel::new(sample_items()).selection_mode(TreeSelectionMode::ExtendedMulti);
    multi.select_single(2);
    let mut driver = Driver::new(wired(multi));
    assert_eq!(
        driver.key(Key::Named(Named::Space), Modifiers::NONE),
        (vec![GridMsg::Selection(BTreeSet::new())], true),
        "Space on a selected tile in multi mode takes it back out"
    );
    assert!(driver.model().selected_ids.is_empty());

    let mut add =
        GridViewModel::new(sample_items()).selection_mode(TreeSelectionMode::ExtendedMulti);
    add.focused_id = Some(5);
    let mut driver = Driver::new(wired(add));
    assert_eq!(
        driver.key(Key::Named(Named::Space), Modifiers::NONE),
        (vec![selection([5])], true),
        "Space on an unselected tile in multi mode adds it"
    );

    let mut driver = Driver::new(wired(focused_model(None)));
    assert_eq!(
        driver.key(Key::Named(Named::Space), Modifiers::NONE),
        (vec![], false),
        "Space with nothing focused does nothing at all"
    );

    let mut single = GridViewModel::new(sample_items());
    single.select_single(1);
    single.focused_id = Some(4);
    let mut driver = Driver::new(wired(single));
    assert_eq!(
        driver.key(Key::Named(Named::Space), Modifiers::NONE),
        (vec![selection([4])], true),
        "in single mode Space replaces the selection rather than toggling"
    );
}

#[test]
fn test_navigation_keys_respond_to_the_modifier_they_arrive_with() {
    let mut model =
        GridViewModel::new(sample_items()).selection_mode(TreeSelectionMode::ExtendedMulti);
    model.select_single(0);
    let mut driver = Driver::new(wired(model));

    assert_eq!(
        driver.key(Key::Named(Named::ArrowRight), Modifiers::NONE),
        (vec![selection([1])], true),
        "a plain arrow selects the tile it moved to"
    );
    assert_eq!(driver.model().focused_id, Some(1));

    assert_eq!(
        driver.key(Key::Named(Named::ArrowRight), Modifiers::CTRL),
        (vec![], true),
        "Ctrl+arrow moves the caret only, so it commits nothing"
    );
    assert_eq!(
        driver.model().focused_id,
        Some(2),
        "Ctrl+arrow still moves the focus"
    );
    assert_eq!(driver.model().selected_ids, BTreeSet::from([1]));

    assert_eq!(
        driver.key(Key::Named(Named::ArrowRight), Modifiers::SHIFT),
        (vec![selection([1, 2, 3])], true),
        "Shift+arrow extends the selection from the anchor to the new tile"
    );

    assert_eq!(
        driver.key(Key::Named(Named::Home), Modifiers::NONE),
        (vec![selection([0])], true),
        "Home jumps to the first tile without wrapping"
    );
    assert_eq!(
        driver.key(Key::Named(Named::End), Modifiers::NONE),
        (vec![selection([7])], true),
        "End jumps to the last tile"
    );
    assert_eq!(
        driver.key(Key::Named(Named::End), Modifiers::NONE),
        (vec![], true),
        "a step onto the tile that is already selected commits nothing but still swallows the key"
    );

    let mut empty = Driver::new(wired(GridViewModel::<usize>::new(Vec::new())));
    assert_eq!(
        empty.key(Key::Named(Named::ArrowDown), Modifiers::NONE),
        (vec![], false),
        "an empty grid must not claim an arrow it cannot act on"
    );
}

#[test]
fn test_a_click_hits_only_real_tiles() {
    let model = GridViewModel::new(sample_items());
    let (_cols, tile_width) = grid_columns(&model, DRIVE_WIDTH);
    let mut driver = Driver::new(wired(model));

    assert_eq!(
        driver.left(Point::new(tile_width + GAP / 2.0, 60.0)),
        (vec![], false),
        "the gutter between two columns belongs to no tile"
    );
    assert_eq!(
        driver.left(Point::new(60.0, DEFAULT_TILE_HEIGHT + GAP / 2.0)),
        (vec![], false),
        "the gutter between two rows belongs to no tile either"
    );
    assert_eq!(
        driver.left(Point::new(DRIVE_WIDTH - GUTTER_WIDTH / 2.0, 60.0)),
        (vec![], false),
        "the gutter reserved for the scrollbar is outside every tile"
    );
    let below_the_grid = Point::new(60.0, driver.node.size().height + 5.0);
    assert_eq!(
        driver.left(below_the_grid),
        (vec![], false),
        "below the last row there is nothing to hit"
    );
}

#[test]
fn test_a_disabled_tile_and_an_empty_cell_answer_no_click_at_all() {
    let mut items = sample_items();
    items[1] = items[1].clone().disabled(true);
    let mut driver = Driver::new(wired(GridViewModel::new(items)));

    let at = driver.tile(1);
    assert_eq!(
        driver.left(at),
        (vec![], false),
        "a disabled tile must not answer a left click"
    );
    let at = driver.tile(1);
    assert_eq!(
        driver.right(at),
        (vec![], false),
        "nor offer a context menu"
    );

    // Seven tiles across two columns leave the last cell of the grid empty.
    let mut driver = Driver::new(wired(GridViewModel::new(sample_items()[..7].to_vec())));
    let at = driver.tile(7);
    assert_eq!(
        driver.left(at),
        (vec![], false),
        "the trailing cell of a partial row is inside the grid but is not a tile"
    );
}

#[test]
fn test_the_checkbox_and_a_modifier_toggle_the_same_way() {
    let mut model =
        GridViewModel::new(sample_items()).selection_mode(TreeSelectionMode::ExtendedMulti);
    model.select_single(0);
    let mut driver = Driver::new(wired(model));

    let at = driver.checkbox(1);
    assert_eq!(
        driver.left(at).0,
        vec![selection([0, 1])],
        "clicking a tile's checkbox adds it without dropping the rest"
    );

    let at = driver.checkbox(0);
    assert_eq!(
        driver.left(at).0,
        vec![selection([1])],
        "clicking a selected tile's checkbox takes it back out"
    );

    driver.hold_modifiers(Modifiers::CTRL);
    let at = driver.tile(2);
    assert_eq!(
        driver.left(at).0,
        vec![selection([1, 2])],
        "Ctrl+click toggles exactly the way the checkbox does"
    );

    driver.hold_modifiers(Modifiers::SHIFT);
    let at = driver.tile(0);
    assert_eq!(
        driver.left(at).0,
        vec![selection([0, 1, 2])],
        "Shift+click ranges from the anchor the toggle left at 2"
    );

    let mut single = GridViewModel::new(sample_items());
    single.select_single(0);
    let mut single_driver = Driver::new(wired(single));
    let at = single_driver.checkbox(0);
    assert_eq!(
        single_driver.left(at),
        (vec![], true),
        "a single-select grid draws no checkbox, so that spot is just the already-selected tile"
    );
    assert_eq!(single_driver.model().selected_ids, BTreeSet::from([0]));
}

#[test]
fn test_a_right_click_selects_the_tile_then_reports_the_point() {
    let mut driver = Driver::new(wired(GridViewModel::new(sample_items())));
    let at = driver.tile(2);

    assert_eq!(
        driver.right(at),
        (
            vec![selection([2]), GridMsg::Context(BTreeSet::from([2]), at)],
            true
        ),
        "a context click on an unselected tile selects it first, then asks for the menu"
    );

    assert_eq!(
        driver.right(at),
        (vec![GridMsg::Context(BTreeSet::from([2]), at)], true),
        "the tile is already selected, so only the menu is asked for"
    );
}

#[test]
fn test_a_right_click_without_the_callbacks_it_would_need() {
    let mut selection_only = Driver::new(
        GridView::new(GridViewModel::new(sample_items()), DRIVE_WIDTH)
            .on_selection_change(|commit| GridMsg::Selection(commit.into_value())),
    );
    let at = selection_only.tile(1);
    assert_eq!(
        selection_only.right(at).0,
        vec![selection([1])],
        "with no menu callback the click still selects"
    );

    let mut bare = Driver::new(GridView::new(
        GridViewModel::new(sample_items()),
        DRIVE_WIDTH,
    ));
    let at = bare.tile(1);
    let (messages, captured) = bare.right(at);
    assert!(
        messages.is_empty(),
        "a grid with neither callback publishes nothing at all"
    );
    assert!(
        captured,
        "the click still landed on a tile, so it is the grid's to swallow"
    );
    assert_eq!(bare.model().selected_ids, BTreeSet::from([1]));
}

#[test]
fn test_the_grid_asks_for_a_pointer_only_over_its_box() {
    let renderer = headless_renderer();
    let model = GridViewModel::new(sample_items());
    let mut widget = GridView::<GridMsg, usize>::new(model.clone(), DRIVE_WIDTH);
    let mut tree = Tree::new(&widget as &dyn Widget<GridMsg, iced::Theme, iced::Renderer>);
    let limits = layout::Limits::NONE
        .max_width(DRIVE_WIDTH)
        .max_height(2_000.0);
    let node = widget.layout(&mut tree, &renderer, &limits);
    let layout = Layout::new(&node);
    let viewport = Rectangle::with_size(node.size());
    let on_tile = centre(tile_at(&model, DRIVE_WIDTH, 2));

    assert_eq!(
        widget.mouse_interaction(
            &tree,
            layout,
            mouse::Cursor::Available(on_tile),
            &viewport,
            &renderer,
        ),
        mouse::Interaction::Pointer,
        "the whole grid is clickable, so it invites the pointer anywhere inside it"
    );
    assert_eq!(
        widget.mouse_interaction(
            &tree,
            layout,
            mouse::Cursor::Available(Point::new(0.0, node.size().height + 10.0)),
            &viewport,
            &renderer,
        ),
        mouse::Interaction::None,
        "outside its own box the grid has no opinion about the cursor"
    );
    assert_eq!(
        widget.mouse_interaction(
            &tree,
            layout,
            mouse::Cursor::Unavailable,
            &viewport,
            &renderer,
        ),
        mouse::Interaction::None,
        "a pointer that is not on the window is not over the grid"
    );
}

#[test]
fn test_a_press_reaches_the_grid_through_a_real_interface() {
    let model = GridViewModel::new(sample_items()).selection_mode(TreeSelectionMode::ExtendedMulti);
    let on_tile = centre(tile_at(&model, DRIVE_WIDTH, 4));
    let in_gutter = Point::new(DRIVE_WIDTH - GUTTER_WIDTH / 2.0, DEFAULT_TILE_HEIGHT / 2.0);
    let mut harness = Harness::<GridMsg, _>::headless(move || {
        grid_view(model.clone(), DRIVE_WIDTH)
            .on_selection_change(|commit| GridMsg::Selection(commit.into_value()))
            .into()
    });

    assert_eq!(
        harness.press(on_tile),
        vec![selection([4])],
        "a press delivered through `UserInterface::update` selects the tile under it"
    );

    assert_eq!(
        harness.press(in_gutter),
        vec![],
        "a press in the scrollbar gutter lands on no tile and publishes nothing"
    );
}

// ---------------------------------------------------------------------------
// 9. Painted states and virtualized culling
// ---------------------------------------------------------------------------

/// Rasterise a grid into [`TALL_FRAME`] while telling it its viewport is
/// `viewport`.
///
/// A capture session's viewport is always its whole frame, so the only way to
/// watch the row cull work is to hand a shorter viewport to [`Widget::draw`]
/// by hand and rasterise the frame it was drawn into.
fn paint_into_tall_frame(
    capture: &mut Capture,
    model: &GridViewModel<usize>,
    viewport: Rectangle,
) -> Frame {
    let mut widget = GridView::<GridMsg, usize>::new(model.clone(), TALL_FRAME.width);
    let mut tree = Tree::new(&widget as &dyn Widget<GridMsg, iced::Theme, iced::Renderer>);
    let limits = layout::Limits::NONE
        .max_width(TALL_FRAME.width)
        .max_height(TALL_FRAME.height);
    let node = widget.layout(&mut tree, capture.renderer(), &limits);
    let layout = Layout::new(&node);

    renderer::Renderer::reset(capture.renderer_mut(), Rectangle::with_size(TALL_FRAME));
    widget.draw(
        &tree,
        capture.renderer_mut(),
        &iced::Theme::Dark,
        &renderer::Style {
            text_color: Color::WHITE,
        },
        layout,
        mouse::Cursor::Unavailable,
        &viewport,
    );

    let physical = Size::new(
        (TALL_FRAME.width * PIXEL_SCALE).round() as u32,
        (TALL_FRAME.height * PIXEL_SCALE).round() as u32,
    );

    Frame::new(
        capture
            .renderer_mut()
            .screenshot(physical, PIXEL_SCALE, Color::BLACK),
        TALL_FRAME,
        PIXEL_SCALE,
    )
}

#[test]
fn test_only_rows_inside_the_viewport_are_painted() {
    let mut capture = Capture::new();
    let model = GridViewModel::new(sample_items());
    let visible = Rectangle::new(
        Point::ORIGIN,
        Size::new(TALL_FRAME.width, TALL_FRAME.height - 300.0),
    );
    let last_row = tile_at(&model, TALL_FRAME.width, 6);
    let below_the_fold = Rectangle::new(
        Point::new(0.0, last_row.y),
        Size::new(TALL_FRAME.width, last_row.height),
    );

    let culled = paint_into_tall_frame(&mut capture, &model, visible);
    let whole = paint_into_tall_frame(&mut capture, &model, Rectangle::with_size(TALL_FRAME));

    assert!(
        culled.painted_count(visible, [0, 0, 0], SURFACE_TOLERANCE) > 0,
        "the rows inside the viewport are painted — the frame is not simply blank"
    );
    assert_eq!(
        culled.painted_count(below_the_fold, [0, 0, 0], SURFACE_TOLERANCE),
        0,
        "the last row starts below the viewport and must be culled entirely"
    );
    assert!(
        whole.painted_count(below_the_fold, [0, 0, 0], SURFACE_TOLERANCE) > 1_000,
        "the very same row does appear once the viewport covers it"
    );
}

#[test]
fn test_the_tile_states_paint_four_different_pictures() {
    let mut capture = Capture::new();
    let viewport = Size::new(400.0, 300.0);
    let model = GridViewModel::new(sample_items());
    let tile0 = tile_at(&model, 400.0, 0);
    let tile1 = tile_at(&model, 400.0, 1);
    let checkbox = Rectangle::new(
        Point::new(tile1.x + CHECKBOX_INSET, tile1.y + CHECKBOX_INSET),
        Size::new(CHECKBOX_SIZE, CHECKBOX_SIZE),
    );

    let rest = capture.frame::<GridMsg>(viewport, grid_view(model.clone(), 400.0).into());
    assert!(
        rest.has_ink(Rectangle::with_size(viewport)),
        "a populated grid paints its titles and its preview icons"
    );

    let at = centre(tile1);
    let hovered = capture.frame_after_at::<GridMsg>(
        viewport,
        grid_view(model.clone(), 400.0).into(),
        &[moved(at)],
        mouse::Cursor::Available(at),
    );
    assert_repainted(
        &rest,
        &hovered,
        tile1,
        "hovering a tile swaps its surface to surface.hover",
    );
    assert_eq!(
        rest.differing_pixels(&hovered, tile0),
        0,
        "hovering one tile must leave its neighbour's picture alone"
    );

    let mut chosen_model = model.clone();
    chosen_model.select_single(1);
    let chosen = capture.frame::<GridMsg>(viewport, grid_view(chosen_model, 400.0).into());
    assert_repainted(
        &rest,
        &chosen,
        tile1,
        "a selected tile swaps its surface, its border and its title weight",
    );

    let mut dimmed = sample_items();
    dimmed[1] = dimmed[1].clone().disabled(true);
    let disabled = capture.frame::<GridMsg>(
        viewport,
        grid_view(GridViewModel::new(dimmed), 400.0).into(),
    );
    assert_repainted(
        &rest,
        &disabled,
        tile1,
        "a disabled tile dims its title and its preview icon",
    );

    let multi = model
        .clone()
        .selection_mode(TreeSelectionMode::ExtendedMulti);
    let boxed = capture.frame::<GridMsg>(viewport, grid_view(multi.clone(), 400.0).into());
    assert_repainted(
        &rest,
        &boxed,
        checkbox,
        "ExtendedMulti draws a checkbox box a single-select grid does not",
    );

    let mut checked = multi;
    checked.select_single(1);
    let ticked = capture.frame::<GridMsg>(viewport, grid_view(checked, 400.0).into());
    assert_repainted(
        &boxed,
        &ticked,
        checkbox,
        "a checked box paints the accent fill and the check glyph over it",
    );

    let mut iconless = sample_items();
    iconless[0] = GridTile::new(0, "Tile 0").subtitle("Subtitle 0");
    let no_icon = capture.frame::<GridMsg>(
        viewport,
        grid_view(GridViewModel::new(iconless), 400.0).into(),
    );
    assert_repainted(
        &rest,
        &no_icon,
        tile0,
        "a tile with no preview icon moves its title up into the icon's place",
    );
}
