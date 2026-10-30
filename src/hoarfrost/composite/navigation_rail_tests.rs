//! Unit tests for `navigation_rail` composite (`UIF-COMP-009`, `UIF-PRIN-005`, `UIF-PRIN-006`).

use super::*;
use crate::binding::GestureStart;
use crate::composite::Icon;
use crate::test_helpers::Harness;
use crate::test_helpers::headless_renderer;
use crate::token::color::Palette;
use crate::token::contrast::contrast_ratio;
use crate::widget::badge_model::BadgeModel;
use crate::widget::range_model::RangeKey;
use iced::Event;
use iced::advanced::Widget;
use iced::keyboard::Key;
use iced::keyboard::key::Named;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum PaneId {
    Files,
    Search,
    SourceControl,
    Debug,
    Settings,
    Help,
}

fn sample_items() -> (Vec<NavDestination<PaneId>>, Vec<NavDestination<PaneId>>) {
    let top = vec![
        NavDestination::new(PaneId::Files, "Files", Icon::new("📁")),
        NavDestination::new(PaneId::Search, "Search", Icon::new("🔍"))
            .with_badge(BadgeModel::count(3)),
        NavDestination::new(PaneId::SourceControl, "Source Control", Icon::new("🌿")),
        NavDestination::new(PaneId::Debug, "Debug", Icon::new("🐞")).with_disabled(true),
    ];
    let bottom = vec![
        NavDestination::new(PaneId::Settings, "Settings", Icon::new("⚙")),
        NavDestination::new(PaneId::Help, "Help", Icon::new("❓")),
    ];
    (top, bottom)
}

// ---------------------------------------------------------------------------
// 1. Data Model Tests
// ---------------------------------------------------------------------------

#[test]
fn test_nav_destination_construction_and_traits() {
    let dest = NavDestination::new(PaneId::Files, "Files", Icon::new("📁"));
    assert_eq!(dest.id, PaneId::Files);
    assert_eq!(dest.label, "Files");
    assert_eq!(dest.icon.0, "📁");
    assert_eq!(dest.badge, None);
    assert!(!dest.disabled);

    let with_badge = dest.clone().with_badge(BadgeModel::count(42));
    assert_eq!(with_badge.badge, Some(BadgeModel::count(42)));

    let disabled = dest.clone().with_disabled(true);
    assert!(disabled.disabled);

    let cloned = dest.clone();
    assert_eq!(dest, cloned);
}

#[test]
fn test_nav_destination_badge_variants() {
    let dotted =
        NavDestination::new(PaneId::Files, "Files", Icon::new("📁")).with_badge(BadgeModel::dot());
    let badge = dotted.badge.expect("a dot badge must be attached");
    assert!(
        badge.text().is_none(),
        "a dot badge paints no text alongside the dot"
    );
    assert!(
        !badge.is_empty(),
        "a dot badge is a visible indicator, never folded away"
    );

    assert!(
        BadgeModel::count(0).is_empty(),
        "a zero count is the only case that renders as nothing"
    );

    let cleared = NavDestination::new(PaneId::Help, "Help", Icon::new("❓")).with_badge(None);
    assert_eq!(
        cleared.badge, None,
        "with_badge(None) must clear an existing badge"
    );
}

#[test]
fn test_navigation_rail_model_defaults_and_builders() {
    let (top, bottom) = sample_items();
    let model = NavigationRailModel::new(top.clone(), bottom.clone(), PaneId::Files);

    assert_eq!(model.top_items.len(), 4);
    assert_eq!(model.bottom_items.len(), 2);
    assert_eq!(model.selected, PaneId::Files);
    assert!(model.expanded);
    assert_eq!(model.expanded_width, EXPANDED_DEFAULT_WIDTH);
    assert_eq!(model.current_width(), EXPANDED_DEFAULT_WIDTH);

    let compact = model.clone().with_expanded(false);
    assert!(!compact.expanded);
    assert_eq!(compact.current_width(), COMPACT_WIDTH);

    let resized = model.clone().with_expanded_width(260.0);
    assert_eq!(resized.expanded_width, 260.0);
    assert_eq!(resized.current_width(), 260.0);
}

#[test]
fn test_navigation_rail_width_clamping() {
    assert_eq!(
        NavigationRailModel::<()>::clamp_expanded_width(100.0),
        EXPANDED_MIN_WIDTH
    );
    assert_eq!(
        NavigationRailModel::<()>::clamp_expanded_width(179.9),
        EXPANDED_MIN_WIDTH
    );
    assert_eq!(
        NavigationRailModel::<()>::clamp_expanded_width(180.0),
        EXPANDED_MIN_WIDTH
    );
    assert_eq!(
        NavigationRailModel::<()>::clamp_expanded_width(220.0),
        220.0
    );
    assert_eq!(
        NavigationRailModel::<()>::clamp_expanded_width(320.0),
        EXPANDED_MAX_WIDTH
    );
    assert_eq!(
        NavigationRailModel::<()>::clamp_expanded_width(320.1),
        EXPANDED_MAX_WIDTH
    );
    assert_eq!(
        NavigationRailModel::<()>::clamp_expanded_width(500.0),
        EXPANDED_MAX_WIDTH
    );
}

#[test]
fn test_all_items_and_enabled_destinations() {
    let (top, bottom) = sample_items();
    let model = NavigationRailModel::new(top, bottom, PaneId::Files);

    let all: Vec<_> = model.all_items().map(|i| i.id).collect();
    assert_eq!(
        all,
        vec![
            PaneId::Files,
            PaneId::Search,
            PaneId::SourceControl,
            PaneId::Debug,
            PaneId::Settings,
            PaneId::Help,
        ]
    );

    // Debug is disabled so it must be absent from enabled_destinations
    let enabled = model.enabled_destinations();
    assert_eq!(
        enabled,
        vec![
            PaneId::Files,
            PaneId::Search,
            PaneId::SourceControl,
            PaneId::Settings,
            PaneId::Help,
        ]
    );
}

// ---------------------------------------------------------------------------
// 2. Keyboard Roving Focus Tests (UIF-COMP-009)
// ---------------------------------------------------------------------------

#[test]
fn test_roving_keyboard_navigation() {
    let (top, bottom) = sample_items();
    let model = NavigationRailModel::new(top, bottom, PaneId::Files);

    // Home jumps to first enabled
    assert_eq!(model.rove(RangeKey::Home), Some(PaneId::Files));

    // End jumps to last enabled
    assert_eq!(model.rove(RangeKey::End), Some(PaneId::Help));

    // Increase steps forward
    assert_eq!(model.rove(RangeKey::Increase), Some(PaneId::Search));

    // Stepping forward from SourceControl must skip disabled Debug and land on Settings
    let at_sc = NavigationRailModel::new(
        model.top_items.clone(),
        model.bottom_items.clone(),
        PaneId::SourceControl,
    );
    assert_eq!(at_sc.rove(RangeKey::Increase), Some(PaneId::Settings));

    // Stepping forward from Help wraps around to Files
    let at_help = NavigationRailModel::new(
        model.top_items.clone(),
        model.bottom_items.clone(),
        PaneId::Help,
    );
    assert_eq!(at_help.rove(RangeKey::Increase), Some(PaneId::Files));

    // Stepping backward from Settings must skip disabled Debug and land on SourceControl
    let at_settings = NavigationRailModel::new(
        model.top_items.clone(),
        model.bottom_items.clone(),
        PaneId::Settings,
    );
    assert_eq!(
        at_settings.rove(RangeKey::Decrease),
        Some(PaneId::SourceControl)
    );

    // Stepping backward from Files wraps around to Help
    assert_eq!(model.rove(RangeKey::Decrease), Some(PaneId::Help));

    // PageDown and PageUp behave identically to Increase and Decrease
    assert_eq!(model.rove(RangeKey::PageDown), Some(PaneId::Search));
    assert_eq!(model.rove(RangeKey::PageUp), Some(PaneId::Help));
}

#[test]
fn test_roving_with_all_disabled() {
    let top = vec![
        NavDestination::new(1, "A", Icon::new("A")).with_disabled(true),
        NavDestination::new(2, "B", Icon::new("B")).with_disabled(true),
    ];
    let model = NavigationRailModel::new(top, vec![], 1);

    assert_eq!(model.rove(RangeKey::Home), None);
    assert_eq!(model.rove(RangeKey::End), None);
    assert_eq!(model.rove(RangeKey::Increase), None);
    assert_eq!(model.rove(RangeKey::Decrease), None);
}

// ---------------------------------------------------------------------------
// 3. Resize Gesture Tests (UIF-PRIN-006)
// ---------------------------------------------------------------------------

#[test]
fn test_nav_rail_resize_lifecycle() {
    let mut resize = NavRailResize::new();
    assert!(!resize.is_active());
    assert_eq!(resize.start_width(), EXPANDED_DEFAULT_WIDTH);
    assert_eq!(resize.current_width(), EXPANDED_DEFAULT_WIDTH);

    // Begin drag at x = 220.0
    assert_eq!(resize.begin(220.0, 220.0), GestureStart::Started);
    assert!(resize.is_active());
    assert_eq!(resize.start_width(), 220.0);

    // Drag right by +40px -> 260px
    let live = resize.update(260.0).expect("live update expected");
    assert_eq!(*live.value(), 260.0);
    assert_eq!(resize.current_width(), 260.0);

    // Commit drag
    let commit = resize.commit().expect("commit expected");
    assert_eq!(*commit.value(), 260.0);
    assert!(!resize.is_active());
}

#[test]
fn test_nav_rail_resize_clamping_and_cancel() {
    let mut resize = NavRailResize::new();
    assert_eq!(resize.begin(200.0, 200.0), GestureStart::Started);

    // Drag way past maximum (320px) -> clamps to 320px
    let live = resize.update(600.0).expect("live update");
    assert_eq!(*live.value(), EXPANDED_MAX_WIDTH);

    // Drag way past minimum (180px) -> clamps to 180px
    let live_min = resize.update(50.0).expect("live update");
    assert_eq!(*live_min.value(), EXPANDED_MIN_WIDTH);

    // Cancel reverts to pre-drag start width (200px)
    let cancel = resize.cancel().expect("cancel expected");
    assert_eq!(*cancel.restored(), 200.0);
    assert_eq!(resize.current_width(), 200.0);
    assert!(!resize.is_active());
}

// ---------------------------------------------------------------------------
// 4. Contrast & Visual Compliance Tests (§3B.9, UIF-PRIN-006)
// ---------------------------------------------------------------------------

#[test]
fn test_navigation_rail_contrast_ratios() {
    let palette = Palette::dark();

    // 1. Selected destination text.strong on surface.active (spec asserts 14.29:1, clear >= 7.0:1 AAA)
    let text_active_contrast = contrast_ratio(palette.text_strong, palette.surface_active);
    assert!(
        text_active_contrast >= 7.0,
        "selected destination text contrast ({text_active_contrast:.2}:1) must clear 7.0:1"
    );

    // 2. Selected leading indicator accent.border on surface.active (spec asserts 4.79:1, clear >= 3.0:1)
    let indicator_contrast = contrast_ratio(palette.accent_border, palette.surface_active);
    assert!(
        indicator_contrast >= 3.0,
        "selected indicator contrast ({indicator_contrast:.2}:1) must clear 3.0:1"
    );

    // 3. Right border promoted accent.border on surface.raised must clear 3.0:1
    let border_promoted = contrast_ratio(palette.accent_border, palette.surface_raised);
    assert!(
        border_promoted >= 3.0,
        "promoted divider contrast ({border_promoted:.2}:1) must clear 3.0:1"
    );

    // 4. Tooltip text.primary on surface.overlay must clear 4.5:1 AA
    let tooltip_contrast = contrast_ratio(palette.text_primary, palette.surface_overlay);
    assert!(
        tooltip_contrast >= 4.5,
        "tooltip text contrast ({tooltip_contrast:.2}:1) must clear 4.5:1"
    );
}

// ---------------------------------------------------------------------------
// 5. Widget Architecture and Layout Tests
// ---------------------------------------------------------------------------

#[test]
fn test_navigation_rail_widget_construction_and_getters() {
    let (top, bottom) = sample_items();
    let model = NavigationRailModel::new(top, bottom, PaneId::Files)
        .with_expanded(true)
        .with_expanded_width(240.0);

    let rail: NavigationRail<'_, (), PaneId> = navigation_rail(model.clone());

    assert_eq!(rail.model(), &model);
    assert_eq!(rail.current_width(), 240.0);
    assert!(rail.is_expanded());
}

#[test]
fn test_navigation_rail_widget_size_expanded_vs_compact() {
    let (top, bottom) = sample_items();
    let model_exp = NavigationRailModel::new(top.clone(), bottom.clone(), PaneId::Files)
        .with_expanded(true)
        .with_expanded_width(220.0);
    let rail_exp: NavigationRail<'_, (), PaneId> = navigation_rail(model_exp);
    let size_exp = rail_exp.size();
    assert_eq!(size_exp.width, Length::Fixed(220.0));
    assert_eq!(size_exp.height, Length::Fill);

    let model_compact = NavigationRailModel::new(top, bottom, PaneId::Files).with_expanded(false);
    let rail_compact: NavigationRail<'_, (), PaneId> = navigation_rail(model_compact);
    let size_compact = rail_compact.size();
    assert_eq!(size_compact.width, Length::Fixed(COMPACT_WIDTH));
    assert_eq!(size_compact.height, Length::Fill);
}

#[test]
fn test_navigation_rail_widget_headless_render_and_tab_stop() {
    // (case label, expanded mode, expected painted width)
    let cases = [
        ("expanded", true, EXPANDED_DEFAULT_WIDTH),
        ("compact", false, COMPACT_WIDTH),
    ];

    for (label, expanded, expected_width) in cases {
        let (top, bottom) = sample_items();
        let model = NavigationRailModel::new(top, bottom, PaneId::Files).with_expanded(expanded);

        let mut harness = Harness::<(), _>::headless(move || navigation_rail(model.clone()).into());
        harness.render();

        // The rail is a single tab stop ([RUST-058] `operate` hook), in both modes.
        let count = harness.focus_count();
        assert_eq!(
            count.total, 1,
            "[{label}] the rail must register exactly one focusable tab stop"
        );

        let recorded = harness.recorded();
        assert_eq!(
            recorded.len(),
            1,
            "[{label}] the rail must register exactly one focusable tab stop"
        );
        assert_eq!(
            recorded[0].0, None,
            "[{label}] the rail registers its tab stop anonymously"
        );
        assert_eq!(
            recorded[0].1.width, expected_width,
            "[{label}] the tab stop must span the rail's current width"
        );
    }
}

#[test]
fn test_navigation_rail_widget_tag_and_state() {
    let (top, bottom) = sample_items();
    let model = NavigationRailModel::new(top, bottom, PaneId::Files);
    let rail: NavigationRail<'_, (), PaneId> = navigation_rail(model);

    let tag = rail.tag();
    assert_eq!(
        tag,
        iced::advanced::widget::tree::Tag::of::<NavigationRailState<PaneId>>()
    );

    let state = rail.state();
    let downcasted = state.downcast_ref::<NavigationRailState<PaneId>>();
    assert!(!downcasted.resize.is_active());
    assert!(!downcasted.is_hovered_grab);
    assert!(!downcasted.is_hovered_toggle);
    assert_eq!(downcasted.hovered_dest, None);
    assert_eq!(downcasted.focused_dest, None);
    assert!(!downcasted.is_focused());
}

#[test]
fn test_navigation_rail_element_conversion() {
    let (top, bottom) = sample_items();
    let model = NavigationRailModel::new(top, bottom, PaneId::Files);
    let rail: NavigationRail<'_, (), PaneId> = navigation_rail(model);
    let _element: Element<'_, ()> = rail.into();
}

#[test]
fn test_toggle_rect_horizontal_non_shifting() {
    let (top, bottom) = sample_items();
    let model = NavigationRailModel::new(top, bottom, PaneId::Files);
    let rail: NavigationRail<'_, (), PaneId> = navigation_rail(model);

    let bounds_exp = Rectangle::new(Point::new(0.0, 0.0), Size::new(220.0, 600.0));
    let bounds_compact = Rectangle::new(Point::new(0.0, 0.0), Size::new(48.0, 600.0));

    let rect_exp = rail.toggle_rect(bounds_exp);
    let rect_compact = rail.toggle_rect(bounds_compact);

    // Both modes position the toggle button at 10px from the left, ensuring 0px shift (UIF-PRIN-004)
    assert_eq!(rect_exp.x, 10.0);
    assert_eq!(rect_compact.x, 10.0);
    assert_eq!(rect_exp.width, TOGGLE_SIZE);
    assert_eq!(rect_compact.width, TOGGLE_SIZE);
}

#[test]
fn test_hit_destination_detection() {
    let (top, bottom) = sample_items();
    let model = NavigationRailModel::new(top, bottom, PaneId::Files);
    let rail: NavigationRail<'_, (), PaneId> = navigation_rail(model);

    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(220.0, 400.0));

    // Middle items start after top row (36px)
    // Row 0 (Files): 36..72px
    let hit_files = rail.hit_destination(bounds, Point::new(50.0, 50.0), 0.0);
    assert_eq!(hit_files.map(|(_, d)| d.id), Some(PaneId::Files));

    // Row 1 (Search): 72..108px
    let hit_search = rail.hit_destination(bounds, Point::new(50.0, 80.0), 0.0);
    assert_eq!(hit_search.map(|(_, d)| d.id), Some(PaneId::Search));

    // Bottom items (2 items = 72px): 328..400px
    // Row 0 of bottom (Settings): 328..364px
    let hit_settings = rail.hit_destination(bounds, Point::new(50.0, 350.0), 0.0);
    assert_eq!(hit_settings.map(|(_, d)| d.id), Some(PaneId::Settings));

    // Row 1 of bottom (Help): 364..400px
    let hit_help = rail.hit_destination(bounds, Point::new(50.0, 380.0), 0.0);
    assert_eq!(hit_help.map(|(_, d)| d.id), Some(PaneId::Help));
}

/// Region boundaries of the rail at `220x400`: toggle row `0..36`, middle
/// `36..327`, divider band `327..328`, pinned bottom `328..400`.
#[test]
fn test_hit_destination_regions_and_overflow() {
    let (top, bottom) = sample_items();
    let model = NavigationRailModel::new(top, bottom, PaneId::Files);
    let rail: NavigationRail<'_, (), PaneId> = navigation_rail(model);
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(220.0, 400.0));

    // (case label, cursor, scroll offset, expected destination)
    let cases: [(&str, Point, f32, Option<PaneId>); 9] = [
        (
            "toggle row is not a destination",
            Point::new(50.0, 10.0),
            0.0,
            None,
        ),
        (
            "middle row 0",
            Point::new(50.0, 36.0),
            0.0,
            Some(PaneId::Files),
        ),
        (
            "middle row 1",
            Point::new(50.0, 100.0),
            0.0,
            Some(PaneId::Search),
        ),
        // Middle band is 291px tall = 8.08 rows, but only 4 top items exist.
        ("middle past last item", Point::new(50.0, 300.0), 0.0, None),
        ("divider hairline", Point::new(50.0, 327.0), 0.0, None),
        (
            "bottom row 0",
            Point::new(50.0, 328.0),
            0.0,
            Some(PaneId::Settings),
        ),
        (
            "bottom last pixel",
            Point::new(50.0, 399.0),
            0.0,
            Some(PaneId::Help),
        ),
        ("below the widget", Point::new(50.0, 400.0), 0.0, None),
        // A disabled destination is still hit-tested; `update` filters it.
        (
            "disabled middle row",
            Point::new(50.0, 150.0),
            0.0,
            Some(PaneId::Debug),
        ),
    ];

    for (label, cursor, scroll, expected) in cases {
        let hit = rail.hit_destination(bounds, cursor, scroll);
        assert_eq!(
            hit.map(|(_, d)| d.id),
            expected,
            "[{label}] hit test at {cursor:?} disagreed"
        );
    }
}

#[test]
fn test_hit_destination_scroll_offset_shifts_middle_rows() {
    let (top, bottom) = sample_items();
    let model = NavigationRailModel::new(top, bottom, PaneId::Files);
    let rail: NavigationRail<'_, (), PaneId> = navigation_rail(model);
    // Middle band: 36..127 (91px), content 144px -> max scroll 53px.
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(220.0, 200.0));

    // One row of scroll lifts the content by one row, so y = 72 now lands on
    // the third destination instead of the second.
    let scrolled = rail.hit_destination(bounds, Point::new(50.0, 72.0), ROW_HEIGHT);
    assert_eq!(
        scrolled.map(|(_, d)| d.id),
        Some(PaneId::SourceControl),
        "a one-row scroll must move the middle rows up by one row"
    );

    // Scrolled to the bottom of the list: the last top item is reachable.
    let at_end = rail.hit_destination(bounds, Point::new(50.0, 100.0), 53.0);
    assert_eq!(
        at_end.map(|(_, d)| d.id),
        Some(PaneId::Debug),
        "the fully scrolled last row must be hit-testable"
    );

    // A negative offset (defensive: `update` clamps to >= 0) hides the first rows.
    let negative = rail.hit_destination(bounds, Point::new(50.0, 40.0), -100.0);
    assert_eq!(
        negative.map(|(_, d)| d.id),
        None,
        "a row scrolled above the viewport must not be hit-testable"
    );
}

#[test]
fn test_hit_destination_without_bottom_items() {
    let (top, _) = sample_items();
    let model = NavigationRailModel::new(top, vec![], PaneId::Files);
    let rail: NavigationRail<'_, (), PaneId> = navigation_rail(model);
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(220.0, 400.0));

    // No bottom section: the divider moves to the very bottom and the whole
    // remaining band belongs to the middle list.
    let hit = rail.hit_destination(bounds, Point::new(50.0, 390.0), 0.0);
    assert_eq!(
        hit.map(|(_, d)| d.id),
        None,
        "row 10 does not exist in a 4-item list"
    );
    let first = rail.hit_destination(bounds, Point::new(50.0, 39.0), 0.0);
    assert_eq!(
        first.map(|(_, d)| d.id),
        Some(PaneId::Files),
        "the middle list must start right after the toggle row"
    );
}

// ---------------------------------------------------------------------------
// 6. Resize Gesture Edge Cases (UIF-PRIN-006)
// ---------------------------------------------------------------------------

#[test]
fn test_nav_rail_resize_idle_gesture_emits_nothing() {
    let mut resize = NavRailResize::new();

    // Every transition asked of an idle slot is a no-op.
    assert!(
        resize.update(260.0).is_none(),
        "a tick with no open gesture must emit no live value"
    );
    assert!(
        resize.commit().is_none(),
        "committing an idle gesture must emit nothing"
    );
    assert!(
        resize.cancel().is_none(),
        "cancelling an idle gesture must emit nothing"
    );
    assert!(!resize.is_active(), "the slot must stay closed");
}

#[test]
fn test_nav_rail_resize_second_begin_is_refused() {
    let mut resize = NavRailResize::new();
    assert_eq!(resize.begin(220.0, 220.0), GestureStart::Started);

    // A second press while the drag is open must not re-time the outer snapshot.
    assert_eq!(
        resize.begin(300.0, 300.0),
        GestureStart::AlreadyOpen,
        "a second press must not open a nested gesture"
    );
    assert_eq!(
        resize.start_width(),
        220.0,
        "the outer snapshot still governs the cancel target"
    );
}

#[test]
fn test_nav_rail_resize_begin_clamps_start_width() {
    // (case label, requested start width, expected clamped start width)
    let cases = [
        ("past maximum", 500.0, EXPANDED_MAX_WIDTH),
        ("past minimum", 100.0, EXPANDED_MIN_WIDTH),
    ];

    for (label, requested, expected) in cases {
        let mut resize = NavRailResize::new();
        assert_eq!(
            resize.begin(requested, requested),
            GestureStart::Started,
            "[{label}] begin must open the slot"
        );
        assert_eq!(
            resize.start_width(),
            expected,
            "[{label}] the captured start width must be clamped into the spec range"
        );
        assert_eq!(
            resize.current_width(),
            expected,
            "[{label}] the live width must start at the clamped width"
        );

        // Zero travel keeps the clamped width rather than the requested one.
        let live = resize
            .update(requested)
            .unwrap_or_else(|| panic!("[{label}] an open slot must emit a live value"));
        assert_eq!(
            *live.value(),
            expected,
            "[{label}] a zero-travel tick must report the clamped width"
        );
    }
}

// ---------------------------------------------------------------------------
// 7. Direct Widget::update Event Tests
// ---------------------------------------------------------------------------

/// Host message emitted by the driven rail.
#[derive(Debug, Clone, PartialEq)]
enum TestMsg {
    /// A destination was activated.
    Select(PaneId),
    /// Compact/expanded mode was toggled.
    Toggle(bool),
    /// Live resize width.
    ResizeLive(f32),
    /// Settled resize width.
    ResizeCommit(f32),
    /// Reverted resize width.
    ResizeCancel(f32),
}

/// The rail's right-edge grab band for `bounds` in expanded mode.
fn grab_band(bounds: Rectangle) -> Rectangle {
    Rectangle {
        x: bounds.x + bounds.width - GRAB_WIDTH / 2.0,
        y: bounds.y,
        width: GRAB_WIDTH,
        height: bounds.height,
    }
}

/// Build a `KeyPressed` event for a named key.
fn key_event(named: Named) -> Event {
    key_press(Key::Named(named))
}

/// Build a `KeyPressed` event for a logical [`Key`].
fn key_press(key: Key) -> Event {
    Event::Keyboard(iced::keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::default(),
        repeat: false,
        text: None,
    })
}

/// Drives [`NavigationRail`] through the real `Widget::update` entry point so a
/// test can read the widget's internal state back out of its [`Tree`].
///
/// One renderer is built per driver and reused across every event the test
/// delivers, because building an iced renderer is the expensive part.
struct RailDriver<'a> {
    widget: NavigationRail<'a, TestMsg, PaneId>,
    tree: Tree,
    renderer: iced::Renderer,
    bounds: Rectangle,
    messages: Vec<TestMsg>,
}

impl<'a> RailDriver<'a> {
    /// Wrap `widget` laid out at `bounds`, with every callback attached.
    fn new(model: NavigationRailModel<PaneId>, bounds: Rectangle) -> Self {
        let widget = navigation_rail(model)
            .on_select(|commit| TestMsg::Select(commit.into_value()))
            .on_expand_toggle(|commit| TestMsg::Toggle(commit.into_value()))
            .on_resize_change(|live| TestMsg::ResizeLive(live.into_value()))
            .on_resize_commit(|commit| TestMsg::ResizeCommit(commit.into_value()))
            .on_resize_cancel(|cancel| TestMsg::ResizeCancel(*cancel.restored()));
        Self::with_widget(widget, bounds)
    }

    /// Wrap an already-assembled widget, e.g. one with callbacks left off.
    fn with_widget(widget: NavigationRail<'a, TestMsg, PaneId>, bounds: Rectangle) -> Self {
        let tree = Tree::new(&widget as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>);
        Self {
            widget,
            tree,
            renderer: headless_renderer(),
            bounds,
            messages: Vec::new(),
        }
    }

    /// The default expanded rail box.
    fn expanded(model: NavigationRailModel<PaneId>) -> Self {
        Self::new(
            model,
            Rectangle::new(Point::new(0.0, 0.0), Size::new(220.0, 400.0)),
        )
    }

    /// Deliver `event` with the pointer at `cursor`; report whether the widget
    /// claimed the event.
    fn drive(&mut self, event: &Event, cursor: mouse::Cursor) -> bool {
        let Self {
            widget,
            tree,
            renderer,
            bounds,
            messages,
        } = self;

        let node = layout::Node::new(bounds.size());
        let layout = Layout::new(&node);
        let mut clipboard = iced::advanced::clipboard::Null;
        let mut shell = Shell::new(messages);
        widget.update(
            tree,
            event,
            layout,
            cursor,
            renderer,
            &mut clipboard,
            &mut shell,
            bounds,
        );
        shell.is_event_captured()
    }

    /// Left button press with the pointer at `at`.
    fn press(&mut self, at: Point) -> bool {
        self.drive(
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            mouse::Cursor::Available(at),
        )
    }

    /// Left button release with the pointer at `at`.
    fn release(&mut self, at: Point) -> bool {
        self.drive(
            &Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            mouse::Cursor::Available(at),
        )
    }

    /// Pointer move with the pointer at `at`.
    ///
    /// The rail tracks hover on every event but only *claims* a move while a
    /// resize gesture owns the pointer, so a plain hover returns `false`.
    fn hover(&mut self, at: Point) -> bool {
        self.drive(
            &Event::Mouse(mouse::Event::CursorMoved { position: at }),
            mouse::Cursor::Available(at),
        )
    }

    /// Wheel scroll with the pointer at `at`, in device pixels.
    fn scroll_pixels(&mut self, at: Point, y: f32) -> bool {
        self.drive(
            &Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Pixels { x: 0.0, y },
            }),
            mouse::Cursor::Available(at),
        )
    }

    /// Wheel scroll with the pointer at `at`, in lines.
    fn scroll_lines(&mut self, at: Point, y: f32) -> bool {
        self.drive(
            &Event::Mouse(mouse::Event::WheelScrolled {
                delta: mouse::ScrollDelta::Lines { x: 0.0, y },
            }),
            mouse::Cursor::Available(at),
        )
    }

    /// Key press with no pointer position reported to the widget.
    fn key(&mut self, named: Named) -> bool {
        self.drive(&key_event(named), mouse::Cursor::Unavailable)
    }

    /// Character-key press with no pointer position reported to the widget.
    fn char_key(&mut self, text: &str) -> bool {
        self.drive(
            &key_press(Key::Character(text.into())),
            mouse::Cursor::Unavailable,
        )
    }

    /// The widget's live internal state.
    fn state(&self) -> &NavigationRailState<PaneId> {
        self.tree
            .state
            .downcast_ref::<NavigationRailState<PaneId>>()
    }

    /// Take every message emitted so far.
    fn take_messages(&mut self) -> Vec<TestMsg> {
        std::mem::take(&mut self.messages)
    }

    /// Ask the widget what cursor shape it wants at `at`.
    fn interaction(&self, at: Point) -> mouse::Interaction {
        let node = layout::Node::new(self.bounds.size());
        let layout = Layout::new(&node);
        self.widget.mouse_interaction(
            &self.tree,
            layout,
            mouse::Cursor::Available(at),
            &self.bounds,
            &self.renderer,
        )
    }
}

#[test]
fn test_press_destination_selects_and_focuses() {
    let (top, bottom) = sample_items();
    let mut driver = RailDriver::expanded(NavigationRailModel::new(top, bottom, PaneId::Files));

    // Row 0 of the middle list: 36..72px.
    let captured = driver.press(Point::new(50.0, 50.0));
    assert!(
        captured,
        "a press on an enabled destination must claim the event"
    );
    assert_eq!(
        driver.take_messages(),
        vec![TestMsg::Select(PaneId::Files)],
        "the press must emit exactly one selection commit"
    );
    assert_eq!(
        driver.state().focused_dest,
        Some(PaneId::Files),
        "a clicked destination takes the roving focus"
    );
    assert!(
        driver.state().is_focused(),
        "clicking a destination must focus the rail's single tab stop"
    );
}

#[test]
fn test_press_disabled_destination_is_ignored() {
    let (top, bottom) = sample_items();
    let mut driver = RailDriver::expanded(NavigationRailModel::new(top, bottom, PaneId::Files));

    // Row 3 of the middle list (Debug, disabled): 144..180px.
    let captured = driver.press(Point::new(50.0, 150.0));
    assert!(
        !captured,
        "a press on a disabled destination must not claim the event"
    );
    assert!(
        driver.take_messages().is_empty(),
        "a disabled destination must not emit a selection"
    );
    assert_eq!(
        driver.state().focused_dest,
        None,
        "a disabled destination must not take the roving focus"
    );
}

#[test]
fn test_press_toggle_commits_inverted_mode() {
    // (case label, expanded, expected commit)
    let cases = [
        ("expanded -> compact", true, false),
        ("compact -> expanded", false, true),
    ];

    for (label, expanded, expected) in cases {
        let (top, bottom) = sample_items();
        let model = NavigationRailModel::new(top, bottom, PaneId::Files).with_expanded(expanded);
        let width = model.current_width();
        let mut driver = RailDriver::new(
            model,
            Rectangle::new(Point::new(0.0, 0.0), Size::new(width, 400.0)),
        );

        // The toggle sits at (10, 4, 28, 28) in both modes (UIF-PRIN-004).
        let centre = Point::new(10.0 + TOGGLE_SIZE / 2.0, 4.0 + TOGGLE_SIZE / 2.0);
        assert!(
            driver.press(centre),
            "[{label}] a press on the toggle must claim the event"
        );
        assert_eq!(
            driver.take_messages(),
            vec![TestMsg::Toggle(expected)],
            "[{label}] the toggle must commit the inverted mode"
        );
    }
}

#[test]
fn test_press_without_attached_callback_emits_nothing() {
    let (top, bottom) = sample_items();
    let model = NavigationRailModel::new(top, bottom, PaneId::Files);
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(220.0, 400.0));
    let mut driver = RailDriver::with_widget(navigation_rail(model), bounds);

    // Toggle row, then a destination row: neither has a host callback attached.
    driver.press(Point::new(24.0, 18.0));
    driver.press(Point::new(50.0, 50.0));

    assert!(
        driver.take_messages().is_empty(),
        "a rail with no callbacks must not invent messages"
    );
}

#[test]
fn test_resize_drag_emits_live_then_commit() {
    let (top, bottom) = sample_items();
    let mut driver = RailDriver::expanded(NavigationRailModel::new(top, bottom, PaneId::Files));
    let grab = grab_band(driver.bounds);

    // Press exactly on the rail's right edge so the travel maths is readable.
    let start = Point::new(driver.bounds.width, 200.0);
    assert!(
        grab.contains(start),
        "the press must land inside the grab band"
    );
    assert!(
        driver.press(start),
        "a press on the grab band must claim the event"
    );
    assert!(
        driver.state().resize.is_active(),
        "the gesture slot must open"
    );

    // Drag +40px.
    assert!(driver.hover(Point::new(260.0, 200.0)));
    assert_eq!(
        driver.take_messages(),
        vec![TestMsg::ResizeLive(260.0)],
        "the drag must emit the live width"
    );

    // Drag far past the maximum: the live value clamps, the gesture stays open.
    assert!(driver.hover(Point::new(900.0, 200.0)));
    assert_eq!(
        driver.take_messages(),
        vec![TestMsg::ResizeLive(EXPANDED_MAX_WIDTH)],
        "the live width must clamp to the spec maximum"
    );

    // Release over the band: commit the clamped width and close the slot.
    assert!(driver.release(Point::new(220.0, 200.0)));
    assert_eq!(
        driver.take_messages(),
        vec![TestMsg::ResizeCommit(EXPANDED_MAX_WIDTH)],
        "release must commit the settled width exactly once"
    );
    assert!(
        !driver.state().resize.is_active(),
        "the gesture slot must close after commit"
    );
    assert!(
        driver.state().is_hovered_grab,
        "releasing over the band must keep the grab hovered"
    );
}

#[test]
fn test_move_and_release_without_active_resize_are_ignored() {
    let (top, bottom) = sample_items();
    let mut driver = RailDriver::expanded(NavigationRailModel::new(top, bottom, PaneId::Files));

    assert!(
        !driver.hover(Point::new(260.0, 200.0)),
        "a move with no open gesture must not claim the event"
    );
    assert!(
        !driver.release(Point::new(220.0, 200.0)),
        "a release with no open gesture must not claim the event"
    );
    assert!(
        driver.take_messages().is_empty(),
        "an idle rail must not emit resize traffic"
    );
}

#[test]
fn test_escape_cancels_active_resize_and_clears_grab_hover() {
    let (top, bottom) = sample_items();
    let mut driver = RailDriver::expanded(NavigationRailModel::new(top, bottom, PaneId::Files));

    assert!(driver.press(Point::new(220.0, 200.0)));
    assert!(
        driver.state().is_hovered_grab,
        "pressing the band hovers it"
    );
    assert!(driver.hover(Point::new(280.0, 200.0)));
    driver.take_messages();

    assert!(
        driver.key(Named::Escape),
        "Escape must claim the event while a resize is open"
    );
    assert_eq!(
        driver.take_messages(),
        vec![TestMsg::ResizeCancel(EXPANDED_DEFAULT_WIDTH)],
        "Escape must restore the pre-drag width"
    );
    assert!(
        !driver.state().resize.is_active(),
        "the gesture slot must close after cancel"
    );
    assert!(
        !driver.state().is_hovered_grab,
        "cancel with no pointer over the band must drop the grab hover"
    );
}

#[test]
fn test_escape_without_resize_suppresses_the_tooltip() {
    let (top, bottom) = sample_items();
    let mut driver = RailDriver::new(
        NavigationRailModel::new(top, bottom, PaneId::Files).with_expanded(false),
        Rectangle::new(Point::new(0.0, 0.0), Size::new(COMPACT_WIDTH, 400.0)),
    );

    assert!(
        !driver.hover(Point::new(24.0, 50.0)),
        "a plain hover move arms the tooltip but must not be claimed"
    );
    assert!(
        driver.state().hover_controller.is_active(),
        "hovering a destination must arm the tooltip"
    );
    driver.take_messages();

    assert!(
        !driver.key(Named::Escape),
        "Escape with no open gesture must not claim the event"
    );
    assert!(
        driver.state().hover_controller.is_suppressed(),
        "Escape must suppress the pending tooltip until the pointer leaves"
    );
    assert!(
        driver.take_messages().is_empty(),
        "Escape with no open gesture must emit no message"
    );
}

#[test]
fn test_wheel_scroll_only_inside_middle_band() {
    let (top, bottom) = sample_items();
    let mut driver = RailDriver::new(
        NavigationRailModel::new(top, bottom, PaneId::Files),
        Rectangle::new(Point::new(0.0, 0.0), Size::new(220.0, 200.0)),
    );
    // Middle band is 36..127 with 144px of content -> max scroll 53px.

    assert!(
        driver.scroll_pixels(Point::new(50.0, 100.0), -30.0),
        "a wheel event inside the middle band must be claimed"
    );
    assert_eq!(driver.state().scroll_offset, 30.0);

    assert!(driver.scroll_lines(Point::new(50.0, 100.0), 1.0));
    assert_eq!(
        driver.state().scroll_offset,
        10.0,
        "one line must scroll 20px"
    );

    assert!(driver.scroll_pixels(Point::new(50.0, 100.0), -500.0));
    assert_eq!(
        driver.state().scroll_offset,
        53.0,
        "the offset must clamp to the scrollable content height"
    );

    assert!(driver.scroll_pixels(Point::new(50.0, 100.0), 500.0));
    assert_eq!(
        driver.state().scroll_offset,
        0.0,
        "the offset must clamp at the top of the list"
    );

    // Outside the middle band: the toggle row and the pinned bottom section.
    assert!(
        !driver.scroll_pixels(Point::new(50.0, 10.0), -30.0),
        "a wheel event over the toggle row must not be claimed"
    );
    assert!(
        !driver.scroll_pixels(Point::new(50.0, 150.0), -30.0),
        "a wheel event over the pinned bottom section must not be claimed"
    );
    assert_eq!(
        driver.state().scroll_offset,
        0.0,
        "scrolling outside the middle band must not move the list"
    );
}

#[test]
fn test_scroll_offset_applies_to_destination_hit_test() {
    let (top, bottom) = sample_items();
    let mut driver = RailDriver::new(
        NavigationRailModel::new(top, bottom, PaneId::Files),
        Rectangle::new(Point::new(0.0, 0.0), Size::new(220.0, 200.0)),
    );

    // Scroll one row up, then click the row now under y = 100.
    assert!(driver.scroll_pixels(Point::new(50.0, 100.0), -36.0));
    assert!(driver.press(Point::new(50.0, 100.0)));
    assert_eq!(
        driver.take_messages(),
        vec![TestMsg::Select(PaneId::SourceControl)],
        "the hit test must account for the scroll offset"
    );
}

#[test]
fn test_keyboard_roving_updates_focused_destination() {
    // (case label, key, expected focused destination — all roved from `selected = Files`)
    let cases = [
        ("ArrowDown steps forward", Named::ArrowDown, PaneId::Search),
        ("ArrowUp steps backward", Named::ArrowUp, PaneId::Help),
        ("Home jumps to the first", Named::Home, PaneId::Files),
        ("End jumps to the last", Named::End, PaneId::Help),
    ];

    for (label, key, expected) in cases {
        let (top, bottom) = sample_items();
        let mut driver = RailDriver::expanded(NavigationRailModel::new(top, bottom, PaneId::Files));

        // The rail only handles arrow keys once it holds focus.
        assert!(
            driver.press(Point::new(50.0, 50.0)),
            "[{label}] the priming click must be claimed"
        );
        driver.take_messages();

        assert!(
            driver.key(key),
            "[{label}] a roving key must claim the event"
        );
        assert_eq!(
            driver.state().focused_dest,
            Some(expected),
            "[{label}] roving landed on the wrong destination"
        );
        assert!(
            driver.take_messages().is_empty(),
            "[{label}] roving focus must not emit a selection"
        );
    }
}

#[test]
fn test_keyboard_activation_uses_focused_then_selected() {
    // (case label, activation key, focus primed by a click, expected activation)
    let cases = [
        (
            "Enter on the focused destination",
            Named::Enter,
            true,
            PaneId::Search,
        ),
        (
            "Space falls back to the selection",
            Named::Space,
            false,
            PaneId::Files,
        ),
    ];

    for (label, key, primed, expected) in cases {
        let (top, bottom) = sample_items();
        let mut driver = RailDriver::expanded(NavigationRailModel::new(top, bottom, PaneId::Files));

        if primed {
            // Focus Search without changing the model's selection.
            assert!(driver.press(Point::new(50.0, 80.0)));
            driver.take_messages();
        } else {
            // Focus the rail without focusing a specific destination.
            let state = driver
                .tree
                .state
                .downcast_mut::<NavigationRailState<PaneId>>();
            operation::Focusable::focus(&mut state.focusable);
        }

        assert!(driver.key(key), "[{label}] activation must claim the event");
        assert_eq!(
            driver.take_messages(),
            vec![TestMsg::Select(expected)],
            "[{label}] activation committed the wrong destination"
        );
    }
}

#[test]
fn test_keyboard_input_is_ignored_when_unfocused_or_unbound() {
    let (top, bottom) = sample_items();
    let mut driver = RailDriver::expanded(NavigationRailModel::new(top, bottom, PaneId::Files));

    // 1. The rail is not focused: roving keys pass through untouched.
    assert!(
        !driver.key(Named::ArrowDown),
        "an unfocused rail must not claim a roving key"
    );
    assert_eq!(driver.state().focused_dest, None);

    // 2. The rail is focused but the key is not bound.
    let state = driver
        .tree
        .state
        .downcast_mut::<NavigationRailState<PaneId>>();
    operation::Focusable::focus(&mut state.focusable);
    assert!(
        !driver.key(Named::Tab),
        "Tab is owned by iced's focus traversal, not the rail"
    );
    assert!(
        !driver.char_key("x"),
        "an unbound character key must not be claimed"
    );
    assert_eq!(
        driver.state().focused_dest,
        None,
        "an unbound key must not move the roving focus"
    );
    assert!(
        driver.take_messages().is_empty(),
        "unclaimed keys must emit nothing"
    );
}

#[test]
fn test_hover_tracking_arms_and_clears_the_tooltip() {
    let (top, bottom) = sample_items();
    let mut driver = RailDriver::expanded(NavigationRailModel::new(top, bottom, PaneId::Files));

    assert!(
        !driver.hover(Point::new(50.0, 50.0)),
        "a plain hover move must not be claimed"
    );
    assert_eq!(driver.state().hovered_dest, Some(PaneId::Files));
    assert!(
        driver.state().hover_controller.is_active(),
        "arriving on a destination must arm the tooltip"
    );
    assert!(
        driver.state().hover_controller.entered_at().is_some(),
        "the arrival must carry a timestamp"
    );
    assert!(
        !driver.state().warm_session.has_memory(),
        "an expanded rail paints no tooltip, so the warm session stays cold"
    );

    // Leaving the list clears the hover and disarms the controller.
    assert!(
        !driver.hover(Point::new(50.0, 300.0)),
        "a plain hover move must not be claimed"
    );
    assert_eq!(
        driver.state().hovered_dest,
        None,
        "leaving every destination must clear the hover"
    );
    assert!(
        !driver.state().hover_controller.is_active(),
        "leaving every destination must disarm the tooltip"
    );
}

#[test]
fn test_missing_pointer_position_clears_hover() {
    let (top, bottom) = sample_items();
    let mut driver = RailDriver::expanded(NavigationRailModel::new(top, bottom, PaneId::Files));

    assert!(
        !driver.hover(Point::new(50.0, 50.0)),
        "a plain hover move must not be claimed"
    );
    assert_eq!(driver.state().hovered_dest, Some(PaneId::Files));

    // An event delivered while the pointer is outside the window.
    driver.drive(
        &Event::Mouse(mouse::Event::CursorMoved {
            position: Point::new(50.0, 50.0),
        }),
        mouse::Cursor::Unavailable,
    );
    assert_eq!(
        driver.state().hovered_dest,
        None,
        "a pointer-less event must clear the hovered destination"
    );
    assert!(
        !driver.state().hover_controller.is_active(),
        "a pointer-less event must disarm the tooltip"
    );
}

#[test]
fn test_compact_hover_shown_tooltip_marks_the_warm_session() {
    let (top, bottom) = sample_items();
    let mut driver = RailDriver::new(
        NavigationRailModel::new(top, bottom, PaneId::Files).with_expanded(false),
        Rectangle::new(Point::new(0.0, 0.0), Size::new(COMPACT_WIDTH, 400.0)),
    );

    // A zero hover delay is a supported accessibility configuration: the
    // tooltip is shown on the first tick instead of after 400ms.
    {
        let state = driver
            .tree
            .state
            .downcast_mut::<NavigationRailState<PaneId>>();
        state.hover_controller = HoverController::with_delay(Duration::ZERO);
    }

    assert!(
        !driver.hover(Point::new(24.0, 50.0)),
        "a plain hover move must not be claimed"
    );
    assert_eq!(driver.state().hovered_dest, Some(PaneId::Files));
    assert!(
        driver.state().warm_session.has_memory(),
        "a shown compact tooltip must be noted so the next arrival is warm"
    );
    assert!(
        driver.state().warm_session.is_warm(TimeMs::now()),
        "the warm handoff window must be open right after the tooltip showed"
    );
}

#[test]
fn test_hover_does_not_track_while_resize_is_open() {
    let (top, bottom) = sample_items();
    let mut driver = RailDriver::expanded(NavigationRailModel::new(top, bottom, PaneId::Files));

    assert!(driver.press(Point::new(220.0, 200.0)));
    assert!(driver.state().is_hovered_grab);

    // Dragging across the list must not light up destinations.
    assert!(driver.hover(Point::new(50.0, 50.0)));
    assert_eq!(
        driver.state().hovered_dest,
        None,
        "hover tracking is frozen while a resize gesture is open"
    );
    assert!(
        !driver.state().is_hovered_toggle,
        "the toggle hover must not be tracked mid-drag"
    );
}

// ---------------------------------------------------------------------------
// 8. Mouse Interaction
// ---------------------------------------------------------------------------

#[test]
fn test_mouse_interaction_per_region() {
    let (top, bottom) = sample_items();
    let model = NavigationRailModel::new(top, bottom, PaneId::Files);
    let bounds = Rectangle::new(Point::new(0.0, 0.0), Size::new(220.0, 400.0));
    let driver = RailDriver::new(model, bounds);

    // (case label, cursor, expected interaction)
    let cases = [
        (
            "grab band",
            Point::new(220.0, 200.0),
            mouse::Interaction::ResizingHorizontally,
        ),
        (
            "toggle button",
            Point::new(24.0, 18.0),
            mouse::Interaction::Pointer,
        ),
        (
            "enabled destination",
            Point::new(50.0, 50.0),
            mouse::Interaction::Pointer,
        ),
        (
            "disabled destination",
            Point::new(50.0, 150.0),
            mouse::Interaction::Idle,
        ),
        (
            "divider band",
            Point::new(50.0, 327.0),
            mouse::Interaction::Idle,
        ),
        (
            "outside the list",
            Point::new(50.0, 300.0),
            mouse::Interaction::Idle,
        ),
    ];

    for (label, at, expected) in cases {
        assert_eq!(
            driver.interaction(at),
            expected,
            "[{label}] reported the wrong cursor at {at:?}"
        );
    }
}

#[test]
fn test_mouse_interaction_grab_only_exists_when_expanded() {
    let (top, bottom) = sample_items();
    let compact =
        NavigationRailModel::new(top.clone(), bottom.clone(), PaneId::Files).with_expanded(false);
    let driver = RailDriver::new(
        compact,
        Rectangle::new(Point::new(0.0, 0.0), Size::new(COMPACT_WIDTH, 400.0)),
    );

    assert_eq!(
        driver.interaction(Point::new(COMPACT_WIDTH - 1.0, 200.0)),
        mouse::Interaction::Idle,
        "a compact rail has no resize edge"
    );

    // The same right-edge point is a resize handle once expanded.
    let expanded = RailDriver::new(
        NavigationRailModel::new(top, bottom, PaneId::Files),
        Rectangle::new(Point::new(0.0, 0.0), Size::new(220.0, 400.0)),
    );
    assert_eq!(
        expanded.interaction(Point::new(219.0, 200.0)),
        mouse::Interaction::ResizingHorizontally,
        "the expanded rail must offer a resize cursor on its right edge"
    );
}

#[test]
fn test_mouse_interaction_keeps_resize_cursor_while_dragging() {
    let (top, bottom) = sample_items();
    let mut driver = RailDriver::expanded(NavigationRailModel::new(top, bottom, PaneId::Files));

    assert!(driver.press(Point::new(220.0, 200.0)));

    assert_eq!(
        driver.interaction(Point::new(50.0, 50.0)),
        mouse::Interaction::ResizingHorizontally,
        "an open resize gesture must keep the resize cursor everywhere"
    );
}

// ---------------------------------------------------------------------------
// 9. Layout Node
// ---------------------------------------------------------------------------

#[test]
fn test_widget_layout_uses_model_width_and_offered_height() {
    let renderer = headless_renderer();
    let limits = layout::Limits::new(Size::ZERO, Size::new(800.0, 600.0));

    // (case label, expanded, expected node width)
    let cases = [
        ("expanded", true, EXPANDED_DEFAULT_WIDTH),
        ("compact", false, COMPACT_WIDTH),
    ];

    for (label, expanded, expected_width) in cases {
        let (top, bottom) = sample_items();
        let model = NavigationRailModel::new(top, bottom, PaneId::Files).with_expanded(expanded);
        let mut rail: NavigationRail<'_, (), PaneId> = navigation_rail(model);
        let mut tree = Tree::new(&rail as &dyn Widget<(), iced::Theme, iced::Renderer>);

        let node = rail.layout(&mut tree, &renderer, &limits);
        assert_eq!(
            node.size().width,
            expected_width,
            "[{label}] the layout node must be as wide as the model reports"
        );
        assert_eq!(
            node.size().height,
            600.0,
            "[{label}] the rail fills the height it is offered"
        );
    }
}
