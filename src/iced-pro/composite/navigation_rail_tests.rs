//! Unit tests for `navigation_rail` composite (`UIF-COMP-009`, `UIF-PRIN-005`, `UIF-PRIN-006`).

use super::*;
use crate::binding::GestureStart;
use crate::composite::Icon;
use crate::test_helpers::Harness;
use crate::token::color::Palette;
use crate::token::contrast::contrast_ratio;
use crate::widget::badge_model::BadgeModel;
use crate::widget::range_model::RangeKey;
use iced::advanced::Widget;

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
fn test_navigation_rail_widget_headless_render_expanded() {
    let (top, bottom) = sample_items();
    let model = NavigationRailModel::new(top, bottom, PaneId::Files);

    let mut harness = Harness::<(), _>::headless(move || navigation_rail(model.clone()).into());
    harness.render();
}

#[test]
fn test_navigation_rail_widget_headless_render_compact() {
    let (top, bottom) = sample_items();
    let model = NavigationRailModel::new(top, bottom, PaneId::Files).with_expanded(false);

    let mut harness = Harness::<(), _>::headless(move || navigation_rail(model.clone()).into());
    harness.render();
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
