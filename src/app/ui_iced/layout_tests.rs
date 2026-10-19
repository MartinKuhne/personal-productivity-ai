//! Unit tests for `layout.rs` ([TASK-010]) — pane/split id mapping and the
//! pure `PanelLayout` resize reductions ([RUST-003]).
//!
//! Widget rendering itself is not unit-testable here; only state
//! reductions, id discovery, and gate wiring are asserted ([TASK-010]
//! testing rule).

use super::*;
use crate::ui::panel_layout::PanelLayout;

/// Reference viewport width used throughout the width-clamping cases.
const VIEWPORT: f32 = 1000.0;

// ---------------------------------------------------------------------------
// Ids & shape contract ([TASK-010]: pane id → region mapping)
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
// Gate reuse ([TASK-010]: `should_show_panel`)
// ---------------------------------------------------------------------------

#[test]
fn test_should_show_panel_gates_via_layout_module() {
    // Re-exported from the iced ToC module (`ui_iced/panels/right.rs`) for
    // the framework-free migration ([TASK-029]); assert the gate contract at the
    // [TASK-010] import path.
    assert!(should_show_panel(true, true));
    assert!(!should_show_panel(true, false));
    assert!(!should_show_panel(false, true));
    assert!(!should_show_panel(false, false));
}

// ---------------------------------------------------------------------------
// Resize → PanelLayout reductions ([TASK-010]; UI-016 clamping)
// ---------------------------------------------------------------------------

fn resize_event(split: Split, ratio: f32) -> pane_grid::ResizeEvent {
    pane_grid::ResizeEvent { split, ratio }
}

#[test]
fn test_left_resize_grows_and_shrinks_within_clamps() {
    let (_state, band_panes) = middle_band_state();
    let mut layout = PanelLayout::new();

    // Drag 25% → 250px, clamped by the 20% cap (200px).
    let replay = apply_pane_resize(
        &band_panes,
        resize_event(band_panes.left_split, 0.25),
        VIEWPORT,
        &mut layout,
    );
    assert_eq!(layout.left_panel_width, Some(200.0));
    // The replayed ratio matches the clamped width, not the raw drag.
    assert_eq!(replay, Some((band_panes.left_split, 0.2)));

    // Drag 18% → 180px exactly the minimum (UI-016 MIN).
    layout = PanelLayout::new();
    let replay = apply_pane_resize(
        &band_panes,
        resize_event(band_panes.left_split, 0.18),
        VIEWPORT,
        &mut layout,
    );
    assert_eq!(layout.left_panel_width, Some(180.0));
    assert_eq!(replay, Some((band_panes.left_split, 0.18)));

    // Drag 10% → 100px, clamped below the minimum to 180px.
    layout = PanelLayout::new();
    let replay = apply_pane_resize(
        &band_panes,
        resize_event(band_panes.left_split, 0.10),
        VIEWPORT,
        &mut layout,
    );
    assert_eq!(layout.left_panel_width, Some(180.0));
    assert_eq!(replay, Some((band_panes.left_split, 0.18)));
}

#[test]
fn test_left_resize_grow_from_existing_width() {
    let (_state, band_panes) = middle_band_state();
    let mut layout = PanelLayout::new();
    layout.set_width(Some(184.0));

    // On a 2500px viewport the 20% cap is 500px, so dragging to 30%
    // (750px) is clamped down to the cap (184 → 500).
    let wide = 2500.0;
    let replay = apply_pane_resize(
        &band_panes,
        resize_event(band_panes.left_split, 0.3),
        wide,
        &mut layout,
    );
    assert_eq!(layout.left_panel_width, Some(500.0));
    assert_eq!(replay, Some((band_panes.left_split, 0.2)));

    // Drag back to 10% of 2500 = 250px; that is ABOVE the 180px minimum,
    // so the value stays 250 and the replayed ratio stays 0.1.
    let replay = apply_pane_resize(
        &band_panes,
        resize_event(band_panes.left_split, 0.1),
        wide,
        &mut layout,
    );
    assert_eq!(layout.left_panel_width, Some(250.0));
    assert_eq!(replay, Some((band_panes.left_split, 0.1)));
}

#[test]
fn test_right_resize_clamped_by_fraction_of_viewport() {
    let (_state, band_panes) = middle_band_state();
    let mut layout = PanelLayout::new();
    layout.set_width(Some(250.0));

    // available = 1000 - 250 = 750; drag ratio 0.5 → right = 375 > cap 300
    // (30% of 1000) → clamped to 300.
    let replay = apply_pane_resize(
        &band_panes,
        resize_event(band_panes.right_split, 0.5),
        VIEWPORT,
        &mut layout,
    );
    assert_eq!(layout.right_panel_width, Some(300.0));
    // Replay feeds 1 - 300/750 = 0.6 so the widget matches the clamped px.
    assert_eq!(replay, Some((band_panes.right_split, 0.6)));
}

#[test]
fn test_right_resize_min_clamp() {
    let (_state, band_panes) = middle_band_state();
    let mut layout = PanelLayout::new();
    layout.set_width(Some(250.0));

    // available = 750; drag ratio 0.9 → right = 75 < min 150 → 150.
    let replay = apply_pane_resize(
        &band_panes,
        resize_event(band_panes.right_split, 0.9),
        VIEWPORT,
        &mut layout,
    );
    assert_eq!(layout.right_panel_width, Some(150.0));
    assert_eq!(replay, Some((band_panes.right_split, 0.8)));
}

#[test]
fn test_right_resize_without_persisted_left_width_uses_default() {
    let (_state, band_panes) = middle_band_state();
    let mut layout = PanelLayout::new();
    assert!(layout.left_panel_width.is_none());

    // available = 1000 - default 280 = 720; ratio 0.5 → right = 360 →
    // clamped to 300 (30% cap).
    let replay = apply_pane_resize(
        &band_panes,
        resize_event(band_panes.right_split, 0.5),
        VIEWPORT,
        &mut layout,
    );
    assert_eq!(layout.right_panel_width, Some(300.0));
    assert_eq!(replay, Some((band_panes.right_split, 1.0 - 300.0 / 720.0)));
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
    // side panel stays draggable ([TASK-010] clamping semantics).
    assert_eq!(clamped_width(50.0, 180.0, 20.0), 180.0);
    assert_eq!(clamped_width(500.0, 180.0, 20.0), 180.0);
}
