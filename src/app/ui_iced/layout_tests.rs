//! Unit tests for `layout.rs` — pane/split id mapping, the pure
//! `PanelLayout` resize reductions ([RUST-003]), the spacing-aware
//! pixel↔ratio conversions introduced by defect #12, the content-sized
//! right (ToC) pane of defect #14, and headless
//! (`iced_test::Simulator`) measurements of the real `middle_band` widget's
//! gutters.

use super::*;
use crate::bus::events::typed::{BackgroundEvent, ProcessEvent};
use crate::config::AppConfig;
use crate::ui::PersistedUiState;
use crate::ui::panel_layout::PanelLayout;
use crate::ui_iced::app::AppState;
use crate::ui_iced::message::{InboundMessage, LayoutMessage, Message};
use crate::ui_iced::test_helpers::test_app_state_with;
use crate::ui_iced::update::update;
use iced::Size;
use iced::widget::{container, text};
use iced_test::selector::Candidate;
use std::path::PathBuf;
use std::sync::Arc;

/// Reference viewport width used throughout the width-clamping cases.
const VIEWPORT: f32 = 1000.0;

/// Sub-pixel tolerance for comparing measured layout rectangles.
const LAYOUT_TOLERANCE: f32 = 0.001;

/// A headless iced driver over the app's `Message` (tier 2).
type Sim<'a> = iced_test::Simulator<'a, Message, iced::Theme, iced::Renderer>;

/// Test-only ids for the three pane bodies, so the simulator can address them.
const LEFT_BODY_ID: &str = "layout_test_pane_left";
/// See [`LEFT_BODY_ID`].
const CENTER_BODY_ID: &str = "layout_test_pane_center";
/// See [`LEFT_BODY_ID`].
const RIGHT_BODY_ID: &str = "layout_test_pane_right";

/// A fill-sized labelled container carrying `id`.
fn labelled<'a>(id: &'static str, label: &'static str) -> iced::Element<'a, Message> {
    container(text(label).size(1.0))
        .id(id)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Lays the real [`middle_band`] widget out headless in a window `width` wide.
fn laid_out_middle_band(panes: &pane_grid::State<PaneKind>, width: f32) -> Sim<'_> {
    let band = middle_band(
        panes,
        || labelled(LEFT_BODY_ID, "L"),
        || labelled(CENTER_BODY_ID, "C"),
        || labelled(RIGHT_BODY_ID, "R"),
        true,
        true,
        |_| {
            Message::Layout(LayoutMessage::CursorMoved {
                point: iced::Point::ORIGIN,
            })
        },
    );
    iced_test::Simulator::with_size(
        iced_test::core::Settings::default(),
        Size::new(width, 600.0),
        band,
    )
}

/// Lays the bare pane grid out headless at exactly `width`, and returns the
/// laid-out width of each pane (left, centre, right).
///
/// This is what proves a replay reached the widget: the numbers come from the
/// panes' own rectangles, not from the ratios stored on the state.
fn laid_out_grid_widths(panes: &pane_grid::State<PaneKind>, width: f32) -> Vec<f32> {
    let grid: iced::Element<'_, Message> =
        pane_grid::PaneGrid::new(panes, |_pane, kind, _is_maximized| {
            pane_grid::Content::new(match kind {
                PaneKind::Left => labelled(LEFT_BODY_ID, "L"),
                PaneKind::Center => labelled(CENTER_BODY_ID, "C"),
                PaneKind::Right => labelled(RIGHT_BODY_ID, "R"),
            })
        })
        .spacing(PANE_SPACING_PIXELS)
        .into();
    let surface: iced::Element<'_, Message> = container(grid)
        .width(Length::Fixed(width))
        .height(Length::Fixed(600.0))
        .into();
    let mut ui = iced_test::Simulator::with_size(
        iced_test::core::Settings::default(),
        Size::new(width, 600.0),
        surface,
    );

    vec![
        bounds_of(&mut ui, LEFT_BODY_ID).width,
        bounds_of(&mut ui, CENTER_BODY_ID).width,
        bounds_of(&mut ui, RIGHT_BODY_ID).width,
    ]
}

/// The laid-out bounds of the container carrying `id`.
fn bounds_of(ui: &mut Sim<'_>, id: &'static str) -> iced::Rectangle {
    let wanted = iced::widget::Id::from(id);
    ui.find(move |candidate: Candidate<'_>| match candidate {
        Candidate::Container {
            id: Some(found),
            bounds,
            ..
        } if *found == wanted => Some(bounds),
        _ => None,
    })
    .unwrap_or_else(|err| panic!("container {id:?} should be reachable: {err:?}"))
}

// ---------------------------------------------------------------------------
// Ids & shape contract (pane id → region mapping)
// ---------------------------------------------------------------------------

#[test]
fn test_middle_band_state_contains_three_panes() {
    let (state, _panes) = middle_band_state();
    assert_eq!(state.panes.len(), 3);
}

#[test]
fn test_middle_band_pane_ids_map_to_regions() {
    let (state, band_panes) = middle_band_state();

    // The BTreeMap ascending order is left/center/right by construction;
    // this pins the id → region mapping end-to-end.
    let left_pane = band_panes.left;
    let center_pane = band_panes.center;
    let right_pane = band_panes.right;

    assert_eq!(state.get(left_pane), Some(&PaneKind::Left));
    assert_eq!(state.get(center_pane), Some(&PaneKind::Center));
    assert_eq!(state.get(right_pane), Some(&PaneKind::Right));

    // The three ids are distinct.
    assert_ne!(left_pane, center_pane);
    assert_ne!(center_pane, right_pane);
    assert_ne!(left_pane, right_pane);
}

#[test]
fn test_middle_band_split_ids_are_two_and_distinct() {
    let (_state, band_panes) = middle_band_state();
    assert_ne!(band_panes.left_split, band_panes.right_split);
}

#[test]
fn test_split_side_mapping_covers_the_pilot_splits() {
    let (_state, band_panes) = middle_band_state();
    assert_eq!(
        split_side(&band_panes, band_panes.left_split),
        Some(PaneSide::Left)
    );
    assert_eq!(
        split_side(&band_panes, band_panes.right_split),
        Some(PaneSide::Right)
    );
}
#[test]
fn test_split_side_unknown_split_returns_none() {
    // `Split` cannot be constructed from outside iced (its inner field is
    // `pub(super)`, `split.rs:5`), so a genuinely foreign split id is
    // synthesized through a separate single-pane state. `split_node`
    // allocates the new PANE before the SPLIT (`state.rs:191-201`), so the
    // probe's first split is `Split(3)` — which COLLIDES with the band's
    // inner split — and only its second split is `Split(5)`, outside the
    // band's id set {3, 4} (panes 0..2, inner split 3, outer split 4).
    let (_state, band_panes) = middle_band_state();
    let (mut probe, probe_pane) = pane_grid::State::new(PaneKind::Center);
    let (probe_pane2, _probe_split1) = probe
        .split(pane_grid::Axis::Horizontal, probe_pane, PaneKind::Center)
        .expect("probe split always succeeds");
    let (_probe_pane3, foreign_split) = probe
        .split(pane_grid::Axis::Horizontal, probe_pane2, PaneKind::Center)
        .expect("second probe split always succeeds");
    assert_eq!(split_side(&band_panes, foreign_split), None);
}

#[test]
fn test_middle_band_configuration_is_two_level_vertical() {
    let config = middle_band_configuration();
    match config {
        Configuration::Split {
            axis,
            a,
            b,
            ratio: _,
        } => {
            assert_eq!(axis, pane_grid::Axis::Vertical);
            assert!(matches!(*a, Configuration::Pane(PaneKind::Left)));
            match *b {
                Configuration::Split {
                    axis,
                    a,
                    b,
                    ratio: _,
                } => {
                    assert_eq!(axis, pane_grid::Axis::Vertical);
                    assert!(matches!(*a, Configuration::Pane(PaneKind::Center)));
                    assert!(matches!(*b, Configuration::Pane(PaneKind::Right)));
                }
                _ => panic!("inner child must be the center|right split"),
            }
        }
        _ => panic!("middle band must split, not be a single pane"),
    }
}

// ---------------------------------------------------------------------------
// Gate reuse (`should_show_panel`)
// ---------------------------------------------------------------------------

#[test]
fn test_should_show_panel_gates_via_layout_module() {
    // Re-exported from the iced ToC module (`ui_iced/panels/right.rs`);
    // assert the gate contract at the `layout` import path.
    assert!(should_show_panel(true, true));
    assert!(!should_show_panel(true, false));
    assert!(!should_show_panel(false, true));
    assert!(!should_show_panel(false, false));
}

// ---------------------------------------------------------------------------
// Resize → PanelLayout reductions (UI-016 clamping)
// ---------------------------------------------------------------------------

fn resize_event(split: Split, ratio: f32) -> pane_grid::ResizeEvent {
    pane_grid::ResizeEvent { split, ratio }
}

#[test]
fn test_left_resize_grows_and_shrinks_within_clamps() {
    let (_state, band_panes) = middle_band_state();
    let mut layout = PanelLayout::new();

    // Drag 25% → the split line sits at 250px, so the left pane claims
    // `250 - spacing/2` = 249px; the 20% cap (200px) wins. The replay puts the
    // line back at `(200 + spacing/2) / 1000` = 0.201, i.e. 1px past the plain
    // `width / viewport` a zero-spacing grid would have asked for.
    let replay = apply_pane_resize(
        &band_panes,
        resize_event(band_panes.left_split, 0.25),
        VIEWPORT,
        &mut layout,
    );
    assert_eq!(layout.left_panel_width, Some(200.0));
    assert_eq!(replay, Some((band_panes.left_split, 0.201)));

    // Drag 18% → `180 - spacing/2` = 179px, exactly the UI-016 MIN once the
    // minimum takes over; replay `(180 + 1) / 1000` = 0.181.
    layout = PanelLayout::new();
    let replay = apply_pane_resize(
        &band_panes,
        resize_event(band_panes.left_split, 0.18),
        VIEWPORT,
        &mut layout,
    );
    assert_eq!(layout.left_panel_width, Some(180.0));
    assert_eq!(replay, Some((band_panes.left_split, 0.181)));

    // Drag 10% → 99px, clamped back up to the 180px minimum → same 0.181.
    layout = PanelLayout::new();
    let replay = apply_pane_resize(
        &band_panes,
        resize_event(band_panes.left_split, 0.10),
        VIEWPORT,
        &mut layout,
    );
    assert_eq!(layout.left_panel_width, Some(180.0));
    assert_eq!(replay, Some((band_panes.left_split, 0.181)));
}

#[test]
fn test_left_resize_grow_from_existing_width() {
    let (_state, band_panes) = middle_band_state();
    let mut layout = PanelLayout::new();
    layout.set_width(Some(184.0));

    // On a 2500px viewport the 20% cap is 500px, so dragging the line to 30%
    // (750px, i.e. 749px of pane) is clamped down to the cap.
    let wide = 2500.0;
    let replay = apply_pane_resize(
        &band_panes,
        resize_event(band_panes.left_split, 0.3),
        wide,
        &mut layout,
    );
    assert_eq!(layout.left_panel_width, Some(500.0));
    // (500 + 1) / 2500 = 0.2004 — again half a gutter past the naive 0.2.
    assert_eq!(replay, Some((band_panes.left_split, 0.2004)));

    // Drag back to 10% of 2500 = 250px line position → 249px of pane. That is
    // ABOVE the 180px minimum, so 249 stands, and the replay lands exactly on
    // the 0.1 the drag asked for: the conversion round-trips when nothing
    // clamps.
    let replay = apply_pane_resize(
        &band_panes,
        resize_event(band_panes.left_split, 0.1),
        wide,
        &mut layout,
    );
    assert_eq!(layout.left_panel_width, Some(249.0));
    assert_eq!(replay, Some((band_panes.left_split, 0.1)));
}

#[test]
fn test_right_resize_clamped_by_fraction_of_viewport() {
    let (_state, band_panes) = middle_band_state();
    let mut layout = PanelLayout::new();
    layout.set_width(Some(250.0));

    // The right split's region is NOT `1000 - 250`: the left split already ate
    // a gutter, so the nested region is `1000 - 250 - spacing` = 748px. Drag
    // ratio 0.5 → the trailing side asks for `0.5 * 748 - spacing/2` = 373px,
    // above the 300px cap (30% of 1000) → clamped to 300.
    let region = VIEWPORT - 250.0 - PANE_SPACING_PIXELS;
    let replay = apply_pane_resize(
        &band_panes,
        resize_event(band_panes.right_split, 0.5),
        VIEWPORT,
        &mut layout,
    );
    assert_eq!(layout.right_panel_width, Some(300.0));
    // Replay feeds `1 - (300 + spacing/2) / 748` so the widget matches the
    // clamped px, measured from the region iced will actually hand the split.
    assert_eq!(
        replay,
        Some((
            band_panes.right_split,
            1.0 - split_ratio_for_width(300.0, region, PANE_SPACING_PIXELS)
        ))
    );
    assert_eq!(region, 748.0, "the nested region deducts one gutter");
}

#[test]
fn test_right_resize_min_clamp() {
    let (_state, band_panes) = middle_band_state();
    let mut layout = PanelLayout::new();
    layout.set_width(Some(250.0));

    // region = 748; drag ratio 0.9 → `0.1 * 748 - spacing/2` = 73.8 < min 150
    // → 150, replayed as `1 - (150 + spacing/2) / 748`.
    let region = VIEWPORT - 250.0 - PANE_SPACING_PIXELS;
    let replay = apply_pane_resize(
        &band_panes,
        resize_event(band_panes.right_split, 0.9),
        VIEWPORT,
        &mut layout,
    );
    assert_eq!(layout.right_panel_width, Some(150.0));
    assert_eq!(
        replay,
        Some((
            band_panes.right_split,
            1.0 - split_ratio_for_width(150.0, region, PANE_SPACING_PIXELS)
        ))
    );
}

#[test]
fn test_right_resize_without_persisted_left_width_uses_the_seeded_ratio() {
    let (_state, band_panes) = middle_band_state();
    let mut layout = PanelLayout::new();
    assert!(layout.left_panel_width.is_none());

    // Nothing is remembered, so the grid holds DEFAULT_ROOT_SPLIT_RATIO's width,
    // not LEFT_PANEL_DEFAULT_WIDTH. At a 1000px viewport that is 249px, not 280,
    // so region = 1000 - 249 - 2 = 749; ratio 0.5 -> 373.5px -> clamped to the
    // 300px (30%) cap.
    let region = nested_region_width(
        VIEWPORT,
        current_left_width(&layout, VIEWPORT),
        PANE_SPACING_PIXELS,
    );
    let replay = apply_pane_resize(
        &band_panes,
        resize_event(band_panes.right_split, 0.5),
        VIEWPORT,
        &mut layout,
    );
    assert_eq!(layout.right_panel_width, Some(300.0));
    assert_eq!(
        replay,
        Some((
            band_panes.right_split,
            1.0 - split_ratio_for_width(300.0, region, PANE_SPACING_PIXELS)
        ))
    );
    assert_eq!(region, 749.0);
}

/// Defect #12 follow-on: a right-split drag must round-trip even before any
/// left width has been remembered.
///
/// `apply_pane_resize` solves the right split against the nested region iced
/// actually hands it. While `PanelLayout.left_panel_width` is `None` the grid is
/// sitting at [`DEFAULT_ROOT_SPLIT_RATIO`], so that region has to be derived from
/// the seeded ratio's width. Deriving it from `LEFT_PANEL_DEFAULT_WIDTH` instead
/// sizes the split against a region iced never receives, and the replayed ratio
/// lands the pane somewhere other than the width the reducer just stored.
///
/// A drag replay is a ratio replay, so it reads the same
/// [`current_left_width`] rule as [`apply_pane_widths`] and the ToC auto-fit.
#[test]
fn defect_12_a_right_drag_round_trips_before_any_left_width_is_remembered() {
    let (mut panes, band_panes) = middle_band_state();
    let mut layout = PanelLayout::new();
    assert!(layout.left_panel_width.is_none());

    // Ratio 0.7 at a 1000px viewport lands the right pane inside the
    // [150, 300] clamp, so nothing is clamped and the round trip is observable.
    let replay = apply_pane_resize(
        &band_panes,
        resize_event(band_panes.right_split, 0.7),
        VIEWPORT,
        &mut layout,
    )
    .expect("a right-split resize is accepted");
    let stored = layout
        .right_panel_width
        .expect("the reducer stored a right width");

    panes.resize(replay.0, replay.1);
    let laid_out = laid_out_grid_widths(&panes, VIEWPORT)[2];

    assert!(
        (laid_out - stored).abs() <= 1.0,
        "defect #12: the reducer stored a right width of {stored}px but the grid \
         laid it out at {laid_out}px — the nested region was derived from a left \
         width the grid does not hold"
    );
}

#[test]
fn test_right_resize_on_zero_available_rejects() {
    let (_state, band_panes) = middle_band_state();
    let mut layout = PanelLayout::new();
    layout.set_width(Some(VIEWPORT));

    assert_eq!(
        apply_pane_resize(
            &band_panes,
            resize_event(band_panes.right_split, 0.5),
            VIEWPORT,
            &mut layout
        ),
        None,
        "no room left for the right pane → event rejected"
    );
    assert!(
        layout.right_panel_width.is_none(),
        "rejected events must not touch the layout"
    );
}

#[test]
fn test_resize_on_zero_viewport_rejected() {
    let (_state, band_panes) = middle_band_state();
    let mut layout = PanelLayout::new();

    assert_eq!(
        apply_pane_resize(
            &band_panes,
            resize_event(band_panes.left_split, 0.5),
            0.0,
            &mut layout
        ),
        None
    );
    assert_eq!(
        apply_pane_resize(
            &band_panes,
            resize_event(band_panes.left_split, 0.5),
            -1.0,
            &mut layout
        ),
        None
    );
}

#[test]
fn test_clamped_width_min_wins_over_degenerate_cap() {
    // Desired below both bounds.
    assert_eq!(clamped_width(50.0, 180.0, 200.0), 180.0);
    // Desired inside the band.
    assert_eq!(clamped_width(190.0, 180.0, 200.0), 190.0);
    // Desired above the cap.
    assert_eq!(clamped_width(500.0, 180.0, 200.0), 200.0);
    // Degenerate cap below the minimum: the minimum wins so the
    // side panel stays draggable.
    assert_eq!(clamped_width(50.0, 180.0, 20.0), 180.0);
    assert_eq!(clamped_width(500.0, 180.0, 20.0), 180.0);
}

// ---------------------------------------------------------------------------
// Splitter grab band + pane gutter (framework token binding, defect #12)
// ---------------------------------------------------------------------------

/// [RUST-021] / `UIF-TOK-002`: the pane-grid resize leeway is derived from the
/// framework's `size.splitter.grab` token rather than owning its own number,
/// and it is reduced by exactly the gutter the grid paints — iced's grab area
/// is `spacing + leeway`, so the two must sum to the mandated band.
#[test]
fn test_resize_leeway_plus_spacing_is_the_framework_splitter_grab_token() {
    assert_eq!(
        RESIZE_LEEWAY_PIXELS + PANE_SPACING_PIXELS,
        hoarfrost::token::size::SPLITTER_GRAB,
        "UIF-PANE-001: the grab band is `spacing + leeway` = size.splitter.grab"
    );
    assert_eq!(
        RESIZE_LEEWAY_PIXELS + PANE_SPACING_PIXELS,
        10.0,
        "the grab band stays 10px wide, reaching 5px to each side of the line"
    );
    // Read through a local: clippy folds a bare `CONST > 0.0` comparison away as
    // a constant assertion, and this guard is here precisely because the value is
    // derived from another token.
    let leeway = RESIZE_LEEWAY_PIXELS;
    assert!(
        leeway > 0.0,
        "the gutter must not eat the whole grab band (leeway would go negative)"
    );
}

/// Defect #12: the gutter is a hoarfrost spacing rung, not a literal, and it
/// stays small enough to read as a divider rather than a margin.
#[test]
fn test_pane_spacing_is_the_framework_space_2xs_token() {
    assert_eq!(
        PANE_SPACING_PIXELS,
        hoarfrost::token::space::XXS,
        "PANE_SPACING_PIXELS must be hoarfrost::token::space::XXS"
    );
    // See [`test_resize_leeway_plus_spacing_is_the_framework_splitter_grab_token`]:
    // the rung comparison is a guard, so it is read through locals.
    let spacing = PANE_SPACING_PIXELS;
    let next_rung = hoarfrost::token::space::XS;
    assert!(
        spacing < next_rung,
        "the seam must stay a divider rung, not grow into a margin"
    );
}

/// `UIF-PRIN-006`: adjacent containers must differ in surface token or carry a
/// visible divider. The gutter is that divider, so its fill must not be the
/// canvas the panes sit on — an identical fill is exactly defect #12.
#[test]
fn test_pane_gutter_fill_differs_from_the_canvas_it_divides() {
    let gutter = pane_gutter_fill();
    let canvas = Palette::fastmd_dark().surface;
    assert_eq!(
        gutter,
        Palette::pro().border_subtle,
        "the gutter carries the framework's pane-split-line colour"
    );
    assert_ne!(
        gutter, canvas,
        "defect #12: a gutter painted in the panes' own surface colour is invisible"
    );
}

// ---------------------------------------------------------------------------
// Spacing-aware ratio conversions (defect #12)
// ---------------------------------------------------------------------------

/// The two conversion helpers are exact inverses: a width replayed as a ratio
/// comes back as the same width, at every viewport the app can be resized to.
#[test]
fn test_ratio_helpers_are_exact_inverses() {
    for viewport in [640.0f32, 1000.0, 1920.0, 2560.0] {
        for width in [150.0f32, 180.0, 249.0, 320.0, 512.5] {
            let ratio = split_ratio_for_width(width, viewport, PANE_SPACING_PIXELS);
            let back = pane_width_from_ratio(ratio, viewport, PANE_SPACING_PIXELS);
            assert!(
                (back - width).abs() < LAYOUT_TOLERANCE,
                "{width}px -> {ratio} -> {back}px at viewport {viewport}"
            );
        }
    }
}

/// The naive zero-spacing conversion `ratio * viewport` overstates the leading
/// pane by half a gutter, and `viewport - left` overstates the nested region by
/// a whole one — the two errors this change removes.
#[test]
fn test_spacing_terms_are_present_in_both_conversions() {
    let half = PANE_SPACING_PIXELS / 2.0;
    assert_eq!(
        pane_width_from_ratio(0.25, 1000.0, PANE_SPACING_PIXELS),
        250.0 - half,
        "the leading pane gives half the gutter to the seam"
    );
    assert_eq!(
        nested_region_width(1000.0, 250.0, PANE_SPACING_PIXELS),
        748.0,
        "the nested region is the viewport minus the left pane minus one gutter"
    );
    assert_eq!(
        nested_region_width(1000.0, 250.0, 0.0),
        750.0,
        "with no gutter the region is the retired `viewport - left`"
    );
}

/// Round-trip through the reducer itself: drag to a width no clamp touches,
/// replay the returned ratio, and the width the grid lays out is the width the
/// reducer stored — at two very different viewports.
#[test]
fn test_unclamped_left_drag_round_trips_at_two_viewports() {
    for viewport in [1200.0f32, 2400.0] {
        let (_state, band_panes) = middle_band_state();
        let mut layout = PanelLayout::new();
        // A drag well inside [MIN, 20% cap] so nothing clamps.
        let wanted = viewport * 0.15;
        let ratio = split_ratio_for_width(wanted, viewport, PANE_SPACING_PIXELS);

        let replay = apply_pane_resize(
            &band_panes,
            resize_event(band_panes.left_split, ratio),
            viewport,
            &mut layout,
        )
        .expect("a positive viewport is never rejected");

        let stored = layout.left_panel_width.unwrap();
        assert!(
            (stored - wanted).abs() < LAYOUT_TOLERANCE,
            "the reducer should have stored the dragged width at viewport {viewport}"
        );
        let laid_out = pane_width_from_ratio(replay.1, viewport, PANE_SPACING_PIXELS);
        assert!(
            (laid_out - stored).abs() < LAYOUT_TOLERANCE,
            "defect #12: replay ratio {} laid out {laid_out}px against the stored \
             {stored}px at viewport {viewport}",
            replay.1,
        );
    }
}

/// The same round-trip for the right split, whose region is the nested one.
#[test]
fn test_unclamped_right_drag_round_trips_at_two_viewports() {
    for viewport in [1200.0f32, 2400.0] {
        let (_state, band_panes) = middle_band_state();
        let mut layout = PanelLayout::new();
        let left = viewport * 0.2;
        layout.set_width(Some(left));
        let region = nested_region_width(viewport, left, PANE_SPACING_PIXELS);

        // A right width inside [MIN, 30% cap] so nothing clamps.
        let wanted = viewport * 0.2;
        let ratio = 1.0 - split_ratio_for_width(wanted, region, PANE_SPACING_PIXELS);

        let replay = apply_pane_resize(
            &band_panes,
            resize_event(band_panes.right_split, ratio),
            viewport,
            &mut layout,
        )
        .expect("a positive viewport is never rejected");

        let stored = layout.right_panel_width.unwrap();
        let laid_out = pane_width_from_ratio(1.0 - replay.1, region, PANE_SPACING_PIXELS);
        assert!(
            (laid_out - stored).abs() < LAYOUT_TOLERANCE,
            "defect #12: replay ratio {} laid out {laid_out}px against the stored \
             {stored}px at viewport {viewport}",
            replay.1,
        );
    }
}

// ---------------------------------------------------------------------------
// F-2 — the startup replay of remembered widths
// ---------------------------------------------------------------------------

/// [`apply_pane_widths`] must leave the grid holding the remembered widths,
/// re-clamped to the current viewport, and must not invent widths that were
/// never remembered.
#[test]
fn test_apply_pane_widths_replays_only_remembered_widths() {
    let (_state, band) = middle_band_state();
    let mut panes = pane_grid::State::with_configuration(middle_band_configuration());
    let viewport = 2000.0;

    // Nothing remembered: the seeded ratios stand and no width is invented.
    let layout = PanelLayout::new();
    apply_pane_widths(&mut panes, &band, &layout, viewport);
    assert!(layout.left_panel_width.is_none());
    assert!(layout.right_panel_width.is_none());
    let seeded = laid_out_grid_widths(&panes, viewport);

    // A non-positive viewport is a no-op rather than a divide-by-zero.
    let mut remembered = PanelLayout::new();
    remembered.set_width(Some(320.0));
    apply_pane_widths(&mut panes, &band, &remembered, 0.0);
    assert_eq!(remembered.left_panel_width, Some(320.0));
    assert_eq!(
        laid_out_grid_widths(&panes, viewport),
        seeded,
        "a rejected replay must leave the grid exactly as it was"
    );
}

/// F-2 — a remembered width must reach the *grid*, not just `PanelLayout`, and
/// it must reach it unchanged: `PanelLayout` owns the number, so the grid has to
/// lay the pane out at exactly that width.
#[test]
fn test_apply_pane_widths_replays_remembered_widths_verbatim_into_the_grid() {
    let (_state, band) = middle_band_state();
    let mut panes = pane_grid::State::with_configuration(middle_band_configuration());
    let viewport = 2000.0;

    let mut layout = PanelLayout::new();
    layout.set_width(Some(320.0));
    layout.set_right_width(Some(220.0));
    apply_pane_widths(&mut panes, &band, &layout, viewport);

    assert_eq!(
        layout.left_panel_width,
        Some(320.0),
        "the replay must not rewrite `PanelLayout`"
    );
    assert_eq!(
        layout.right_panel_width,
        Some(220.0),
        "the replay must not rewrite `PanelLayout`"
    );

    let widths = laid_out_grid_widths(&panes, viewport);
    assert!(
        (widths[0] - 320.0).abs() <= 1.0,
        "the remembered left width must be the laid-out left width, got {}",
        widths[0]
    );
    assert!(
        (widths[2] - 220.0).abs() <= 1.0,
        "the remembered right width must be the laid-out right width, got {}",
        widths[2]
    );
}

/// The UI-016 bounds govern the *drag*, not the restore: a width remembered
/// from a larger window still takes effect instead of being silently resized.
#[test]
fn test_apply_pane_widths_does_not_reclamp_a_width_from_a_larger_window() {
    let (_state, band) = middle_band_state();
    let mut panes = pane_grid::State::with_configuration(middle_band_configuration());
    let viewport = 2000.0;

    // 900px is far above the UI-016 left cap for this viewport
    // (`viewport * LEFT_PANEL_MAX_WIDTH_FRACTION` = 600px).
    let mut layout = PanelLayout::new();
    layout.set_width(Some(900.0));
    apply_pane_widths(&mut panes, &band, &layout, viewport);

    assert_eq!(layout.left_panel_width, Some(900.0));
    let widths = laid_out_grid_widths(&panes, viewport);
    assert!(
        (widths[0] - 900.0).abs() <= 1.0,
        "the restore must not be re-clamped to the UI-016 cap, got {}",
        widths[0]
    );
}

/// [RUST-021]: the seeded split ratios are named constants, so the startup
/// replay can reason about the region a fresh grid hands the nested split.
#[test]
fn test_seeded_split_ratios_are_named() {
    let config = middle_band_configuration();
    match config {
        Configuration::Split { ratio, b, .. } => {
            assert_eq!(ratio, DEFAULT_ROOT_SPLIT_RATIO);
            match *b {
                Configuration::Split { ratio, .. } => {
                    assert_eq!(ratio, DEFAULT_NESTED_SPLIT_RATIO);
                }
                _ => panic!("inner child must be the center|right split"),
            }
        }
        _ => panic!("middle band must split, not be a single pane"),
    }
}

// ---------------------------------------------------------------------------
// Defect #12 — "there is no visual separation between the three window panes"
//
// The tests in this section measure the real `middle_band` widget headless
// (`iced_test::Simulator`) rather than reading a constant back out of the
// module, because the defect is a *geometry* claim. The first one is the RED
// that was proven against the unmodified grid: with iced's default `0.0`
// spacing the measured gutter came out at exactly `0.0px`.
// ---------------------------------------------------------------------------

/// Defect #12 — the three panes must not butt up against each other: a real
/// gutter of window canvas has to separate each adjacent pair.
#[test]
fn defect_12_the_pane_grid_separates_each_pair_of_panes_by_a_gutter() {
    let (panes, _band) = middle_band_state();
    let mut ui = laid_out_middle_band(&panes, 1200.0);

    let left = bounds_of(&mut ui, LEFT_BODY_ID);
    let center = bounds_of(&mut ui, CENTER_BODY_ID);
    let right = bounds_of(&mut ui, RIGHT_BODY_ID);

    let first_gutter = center.x - (left.x + left.width);
    let second_gutter = right.x - (center.x + center.width);

    assert!(
        first_gutter > 0.0,
        "defect #12: the centre pane starts at x={:.1}, the left pane ends at \
         x={:.1} — the gutter between them is {:.1}px, so nothing separates them",
        center.x,
        left.x + left.width,
        first_gutter,
    );
    assert!(
        second_gutter > 0.0,
        "defect #12: the right pane starts at x={:.1}, the centre pane ends at \
         x={:.1} — the gutter between them is {:.1}px, so nothing separates them",
        right.x,
        center.x + center.width,
        second_gutter,
    );
}

/// Defect #12 — the gutter is exactly one [`PANE_SPACING_PIXELS`] wide at both
/// edges, at more than one viewport, and no pane overlaps its neighbour. The
/// measurement is the panes' own laid-out rectangles.
#[test]
fn defect_12_the_gutter_is_exactly_one_spacing_wide_at_every_viewport() {
    for viewport in [800.0f32, 1200.0, 2000.0] {
        let (panes, _band) = middle_band_state();
        let mut ui = laid_out_middle_band(&panes, viewport);

        let left = bounds_of(&mut ui, LEFT_BODY_ID);
        let center = bounds_of(&mut ui, CENTER_BODY_ID);
        let right = bounds_of(&mut ui, RIGHT_BODY_ID);

        let first = center.x - (left.x + left.width);
        let second = right.x - (center.x + center.width);

        assert!(
            (first - PANE_SPACING_PIXELS).abs() < LAYOUT_TOLERANCE,
            "defect #12: at viewport {viewport} the left|centre gutter measured {first}px, \
             not the {PANE_SPACING_PIXELS}px the grid is built with"
        );
        assert!(
            (second - PANE_SPACING_PIXELS).abs() < LAYOUT_TOLERANCE,
            "defect #12: at viewport {viewport} the centre|right gutter measured {second}px, \
             not the {PANE_SPACING_PIXELS}px the grid is built with"
        );

        // Nothing may overlap, and the three panes plus the two gutters must
        // account for the whole viewport.
        assert!(center.x >= left.x + left.width - LAYOUT_TOLERANCE);
        assert!(right.x >= center.x + center.width - LAYOUT_TOLERANCE);
        let covered = left.width + center.width + right.width + 2.0 * PANE_SPACING_PIXELS;
        assert!(
            (covered - viewport).abs() < 1.0,
            "defect #12: panes {left:?} {center:?} {right:?} plus two gutters cover \
             {covered}px of a {viewport}px viewport"
        );
    }
}

/// F-2 — a width remembered from the previous session must actually reach the
/// pane grid at startup, not just `PanelLayout`.
#[test]
fn defect_12_f2_restored_pane_widths_take_effect_at_startup() {
    let viewport_width = 2000.0;
    let remembered_left = 320.0;
    let remembered_right = 220.0;

    let persisted = PersistedUiState {
        window_width: Some(viewport_width),
        window_height: Some(1000.0),
        left_panel_width: Some(remembered_left),
        right_panel_width: Some(remembered_right),
        ..Default::default()
    };

    let state = test_app_state_with(AppConfig::default(), persisted);
    assert_eq!(
        state.app.layout.left_panel_width,
        Some(remembered_left),
        "the fixture assumes the restore into PanelLayout already works"
    );

    let mut ui = laid_out_middle_band(&state.panes, state.viewport.width);
    let left = bounds_of(&mut ui, LEFT_BODY_ID);
    let right = bounds_of(&mut ui, RIGHT_BODY_ID);

    assert!(
        (left.width - remembered_left).abs() < 1.0,
        "defect #12 / F-2: the remembered left width is {remembered_left}px but the \
         grid laid the left pane out at {:.1}px — the restore never reaches the grid",
        left.width,
    );
    assert!(
        (right.width - remembered_right).abs() < 1.0,
        "defect #12 / F-2: the remembered right width is {remembered_right}px but the \
         grid laid the right pane out at {:.1}px — the restore never reaches the grid",
        right.width,
    );
}

/// [RUST-021] / `UIF-TOK-002`: the stub band height follows the
/// framework's `size.control.lg` rung — the same rung the live navbar
/// band is built at — rather than keeping its own off-scale `28`.
///
/// **Movement: 28 → 36**, tracking
/// [`crate::ui_iced::panels::top::BAR_HEIGHT_POINTS`]. The stubs are
/// unmounted dev scaffolding; see [`STUB_BAND_HEIGHT_POINTS`] for why one
/// fixed height cannot represent both real bands.
#[test]
fn test_stub_band_height_is_the_framework_control_lg_token() {
    assert_eq!(
        STUB_BAND_HEIGHT_POINTS,
        hoarfrost::token::size::CONTROL_LG,
        "STUB_BAND_HEIGHT_POINTS must be hoarfrost::token::size::CONTROL_LG"
    );
    assert_eq!(STUB_BAND_HEIGHT_POINTS, 36.0);
    assert_ne!(
        STUB_BAND_HEIGHT_POINTS, 28.0,
        "the retired off-token 28 must not come back"
    );
}

// ---------------------------------------------------------------------------
// Defect #14 — "the table of contents takes up more horizontal space than it
// needs to display its contents"
//
// The right split was a bare `ratio: 0.5` with no content awareness, so the
// ToC opened at roughly 37.5% of the viewport whether its longest entry was
// "Intro" or a 90-character heading. The owner's ruling is a **content-sized
// right panel**, so the tests below drive a real document load through the
// real reducer and measure the pane's own laid-out rectangle — the same
// gutter-aware measurement defect #12 introduced.
// ---------------------------------------------------------------------------

/// A markdown document whose only heading is as short as a heading gets.
const SHORT_TOC_DOC: &str = "# Intro\n";

/// A markdown document with a genuinely long heading that the UI-016 30% cap
/// still leaves room for at the viewports it is measured at.
const LONG_TOC_DOC: &str = "# Getting Started With The FastMD Workspace\n";

/// A markdown document whose heading runs to ~90 characters — far more than
/// the UI-016 30% cap can ever hold at a normal window width.
const OVERLONG_TOC_DOC: &str =
    "# Implementing the Fair Table Width Algorithm across every renderer in the workspace\n";

/// A booted state at `viewport` with **no** remembered panel widths, so the
/// seeded split ratios are the baseline being measured against.
fn booted_at(viewport: f32) -> AppState {
    let mut state = test_app_state_with(AppConfig::default(), PersistedUiState::default());
    state.viewport = Size::new(viewport, 800.0);
    state
}

/// Loads `markdown` the way the application does: select the file, then land
/// the background `FileLoaded` result through the real reducer, so the ToC the
/// pane is fitted from is the one `AppOrchestrator` built.
fn load_document(state: &mut AppState, markdown: &str) {
    let path = PathBuf::from("/notes/defect14.md");
    state.app.orchestrator.selection.select_file(path.clone());
    let event = BackgroundEvent::Process(ProcessEvent::FileLoaded {
        path,
        content: Ok(markdown.to_string()),
    });
    let _ = update(
        state,
        Message::Inbound(InboundMessage::Background(Arc::new(event))),
    );
}

/// Defect #14 — a ToC whose longest entry is `Intro` must not be handed more
/// width than the entry needs. The floor it lands on is the UI-016 right-panel
/// minimum, which is the smallest width the pane is allowed to have at all.
#[test]
fn defect_14_a_short_toc_is_not_given_more_width_than_it_needs() {
    for viewport in [800.0f32, 1200.0, 2000.0] {
        let mut state = booted_at(viewport);
        load_document(&mut state, SHORT_TOC_DOC);

        let widths = laid_out_grid_widths(&state.panes, viewport);
        assert!(
            widths[2] <= RIGHT_PANEL_MIN_WIDTH + 1.0,
            "defect #14: at viewport {viewport} a ToC whose only heading is \"Intro\" was \
             laid out {:.1}px wide; the entry needs at most the {RIGHT_PANEL_MIN_WIDTH}px \
             panel minimum, so the pane is taking space it does not display",
            widths[2],
        );
    }
}

/// The width the widest row of `entries` actually needs, measured with the
/// production measurer at the rung the pane paints at.
fn needed_toc_width(entries: &[crate::markdown::ToCEntry]) -> f32 {
    crate::ui_iced::panels::right::longest_entry_width(entries, |text| {
        crate::ui_iced::table_width::fira_sans_width(
            text,
            iced::Font::DEFAULT,
            crate::ui_iced::view::TOC_TEXT_RUNG.size,
        )
    })
}

/// Defect #14 — the fit must not just be *narrower*, a long heading still has
/// to fit inside the pane it is given. Measured at two viewports against the
/// row's own required width, computed with the same real metrics the solver
/// uses.
#[test]
fn defect_14_a_long_heading_still_fits_inside_the_pane() {
    for viewport in [1200.0f32, 2000.0] {
        let mut state = booted_at(viewport);
        load_document(&mut state, LONG_TOC_DOC);

        let entries = &state.app.orchestrator.tabs.toc;
        assert!(
            !entries.is_empty(),
            "the fixture document must produce a ToC at all"
        );
        let needed = needed_toc_width(entries);
        let cap = viewport * RIGHT_PANEL_MAX_WIDTH_FRACTION;
        assert!(
            needed < cap,
            "fixture: this heading needs {needed:.1}px, which is above the {cap:.1}px \
             UI-016 cap — the case is unmeasurable here",
        );

        let widths = laid_out_grid_widths(&state.panes, viewport);
        assert!(
            widths[2] + LAYOUT_TOLERANCE >= needed,
            "defect #14: at viewport {viewport} the widest ToC row needs {needed:.1}px but \
             the pane was laid out at {:.1}px — the heading would be truncated",
            widths[2],
        );
    }
}

/// Defect #14 — an honest statement of what the UI-016 clamp does to a heading
/// it cannot hold: the pane stops at `viewport × 30%` and the heading is
/// clipped. The clamp is the contract, so the solver does not fight it. This
/// test pins the ceiling — `laid out == min(needed, cap)` — at a viewport where
/// the cap genuinely binds and one where it does not.
#[test]
fn defect_14_a_heading_beyond_the_ui_016_cap_is_clamped_not_overflowed() {
    for viewport in [1200.0f32, 2000.0] {
        let mut state = booted_at(viewport);
        load_document(&mut state, OVERLONG_TOC_DOC);

        let needed = needed_toc_width(&state.app.orchestrator.tabs.toc);
        let cap = viewport * RIGHT_PANEL_MAX_WIDTH_FRACTION;
        let widths = laid_out_grid_widths(&state.panes, viewport);

        assert!(
            (widths[2] - needed.min(cap)).abs() <= 1.0,
            "defect #14: at viewport {viewport} a heading needing {needed:.1}px should land \
             at min(needed, {cap:.1}px) = {:.1}px, but the pane measured {:.1}px",
            needed.min(cap),
            widths[2],
        );

        if needed > cap {
            // The clamp is fighting real content: record it rather than hide it.
            assert!(
                widths[2] + LAYOUT_TOLERANCE < needed,
                "fixture: at viewport {viewport} the cap should bind ({needed:.1}px needed, \
                 {cap:.1}px allowed), so the pane must be short of the content"
            );
        }
    }
}

/// Defect #14 — the fit is per document: loading a short-heading document
/// after a long one shrinks the pane again instead of leaving the widest
/// width latched for the session.
#[test]
fn defect_14_the_fit_follows_the_document_being_loaded() {
    let mut state = booted_at(1200.0);
    load_document(&mut state, OVERLONG_TOC_DOC);
    let wide = laid_out_grid_widths(&state.panes, 1200.0)[2];

    load_document(&mut state, SHORT_TOC_DOC);
    let narrow = laid_out_grid_widths(&state.panes, 1200.0)[2];

    assert!(
        narrow < wide,
        "the pane must re-fit per document, not latch the widest heading \
         (long {wide:.1}px, short {narrow:.1}px)"
    );
}
