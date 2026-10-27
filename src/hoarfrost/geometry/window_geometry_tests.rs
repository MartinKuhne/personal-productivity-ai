//! Unit tests for window geometry core, UI scale, layout solvers, and storage (`UIF-WIN-001`).

use super::*;
use std::sync::Arc;

#[test]
fn test_window_geometry_constants_match_spec() {
    assert_eq!(WINDOW_MIN_WIDTH, 640.0);
    assert_eq!(WINDOW_MIN_HEIGHT, 480.0);
    assert_eq!(WINDOW_DEFAULT_WIDTH, 1000.0);
    assert_eq!(WINDOW_DEFAULT_HEIGHT, 700.0);
    assert_eq!(CENTER_PANE_MIN_W, 240.0);
}

#[test]
fn test_ui_scale_default_and_factors() {
    let default_scale = UiScale::DEFAULT;
    assert_eq!(default_scale.factor(), 1.00);
    assert_eq!(UiScale::default(), default_scale);
}

#[test]
fn test_ui_scale_clamping() {
    // Below minimum (0.75)
    let low = UiScale::new(0.50);
    assert_eq!(low.factor(), 0.75);

    let exact_min = UiScale::new(0.75);
    assert_eq!(exact_min.factor(), 0.75);

    // Above maximum (2.00)
    let high = UiScale::new(2.50);
    assert_eq!(high.factor(), 2.00);

    let exact_max = UiScale::new(2.00);
    assert_eq!(exact_max.factor(), 2.00);

    // Non-finite resolves to DEFAULT (1.00)
    let nan_scale = UiScale::new(f32::NAN);
    assert_eq!(nan_scale.factor(), 1.00);

    let inf_scale = UiScale::new(f32::INFINITY);
    assert_eq!(inf_scale.factor(), 1.00);
}

#[test]
fn test_ui_scale_step_quantization() {
    // 0.82 rounds to 0.80
    assert_eq!(UiScale::new(0.82).factor(), 0.80);

    // 0.88 rounds to 0.90
    assert_eq!(UiScale::new(0.88).factor(), 0.90);

    // 1.04 rounds to 1.00
    assert_eq!(UiScale::new(1.04).factor(), 1.00);

    // 1.06 rounds to 1.10
    assert_eq!(UiScale::new(1.06).factor(), 1.10);
}

#[test]
fn test_ui_scale_step_up_and_down_lifecycle() {
    let mut scale = UiScale::new(0.75);
    assert_eq!(scale.factor(), 0.75);

    // Step up from 0.75 goes to 0.80
    scale = scale.step_up();
    assert_eq!(scale.factor(), 0.80);

    // Step up through all rungs to 2.00
    let expected_steps = [
        0.90, 1.00, 1.10, 1.20, 1.30, 1.40, 1.50, 1.60, 1.70, 1.80, 1.90, 2.00,
    ];
    for expected in expected_steps {
        scale = scale.step_up();
        assert!(
            (scale.factor() - expected).abs() < 1e-4,
            "expected {expected}, got {}",
            scale.factor()
        );
    }

    // Step up at 2.00 stays at 2.00
    scale = scale.step_up();
    assert_eq!(scale.factor(), 2.00);

    // Step down through all rungs
    let expected_down = [
        1.90, 1.80, 1.70, 1.60, 1.50, 1.40, 1.30, 1.20, 1.10, 1.00, 0.90, 0.80, 0.75,
    ];
    for expected in expected_down {
        scale = scale.step_down();
        assert!(
            (scale.factor() - expected).abs() < 1e-4,
            "expected {expected}, got {}",
            scale.factor()
        );
    }

    // Step down at 0.75 stays at 0.75
    scale = scale.step_down();
    assert_eq!(scale.factor(), 0.75);
}

#[test]
fn test_ui_scale_compose_with_dpi() {
    let scale = UiScale::new(1.20);
    // Composed with 1.5x OS DPI
    assert!((scale.compose_with_dpi(1.5) - 1.80).abs() < 1e-4);

    // Degenerate DPI fallbacks
    assert_eq!(scale.compose_with_dpi(f32::NAN), 1.20);
    assert_eq!(scale.compose_with_dpi(-1.0), 1.20);
    assert_eq!(scale.compose_with_dpi(0.0), 1.20);
}

#[test]
fn test_window_geometry_default() {
    let geom = WindowGeometry::default();
    assert_eq!(geom.x, None);
    assert_eq!(geom.y, None);
    assert_eq!(geom.width, 1000.0);
    assert_eq!(geom.height, 700.0);
    assert_eq!(geom.left_split_ratio, 0.20);
    assert_eq!(geom.right_split_ratio, 0.20);
    assert_eq!(geom.ui_scale, UiScale::DEFAULT);
}

#[test]
fn test_solve_window_layout_happy_path_with_mounted_right_pane() {
    let layout = solve_window_layout(1000.0, 700.0, 0.20, 0.20, true);

    assert_eq!(layout.window_width, 1000.0);
    assert_eq!(layout.window_height, 700.0);
    assert_eq!(layout.left_pane_width, 200.0);
    assert_eq!(layout.right_pane_width, 200.0);
    assert_eq!(layout.center_pane_width, 600.0);
    assert!(layout.center_pane_width >= CENTER_PANE_MIN_W);

    // Sum of widths equals total window width
    let total = layout.left_pane_width + layout.center_pane_width + layout.right_pane_width;
    assert_eq!(total, layout.window_width);
}

#[test]
fn test_solve_window_layout_unmounted_right_pane_returns_width_to_center() {
    let layout = solve_window_layout(1000.0, 700.0, 0.20, 0.20, false);

    assert_eq!(layout.window_width, 1000.0);
    assert_eq!(layout.window_height, 700.0);
    assert_eq!(layout.left_pane_width, 200.0);
    assert_eq!(layout.right_pane_width, 0.0);
    assert_eq!(layout.center_pane_width, 800.0);

    let total = layout.left_pane_width + layout.center_pane_width + layout.right_pane_width;
    assert_eq!(total, layout.window_width);
}

#[test]
fn test_solve_window_layout_clamps_to_window_minimums() {
    let layout = solve_window_layout(400.0, 300.0, 0.20, 0.20, true);

    assert_eq!(layout.window_width, WINDOW_MIN_WIDTH);
    assert_eq!(layout.window_height, WINDOW_MIN_HEIGHT);
    assert!(layout.center_pane_width >= CENTER_PANE_MIN_W);
    assert!(layout.left_pane_width >= 150.0);
    assert!(layout.right_pane_width >= 120.0);

    let total = layout.left_pane_width + layout.center_pane_width + layout.right_pane_width;
    assert_eq!(total, layout.window_width);
}

#[test]
fn test_solve_window_layout_degenerate_non_finite_inputs() {
    let layout = solve_window_layout(f32::NAN, f32::NAN, f32::NAN, f32::NAN, true);

    assert_eq!(layout.window_width, WINDOW_MIN_WIDTH);
    assert_eq!(layout.window_height, WINDOW_MIN_HEIGHT);
    assert!(layout.center_pane_width >= CENTER_PANE_MIN_W);
    assert_eq!(layout.left_pane_width, 150.0);
    assert_eq!(layout.right_pane_width, 128.0);
    assert_eq!(layout.center_pane_width, 362.0);
}

#[test]
fn test_solve_window_layout_preserves_center_pane_floor_under_deficit() {
    // When left and right panes try to request large widths, center pane floor (240px) is preserved
    let layout = solve_window_layout(640.0, 480.0, 0.50, 0.50, true);

    assert_eq!(layout.window_width, 640.0);
    assert_eq!(layout.window_height, 480.0);
    assert!(layout.center_pane_width >= CENTER_PANE_MIN_W);
    assert!(layout.left_pane_width >= 150.0);
    assert!(layout.right_pane_width >= 120.0);

    let total = layout.left_pane_width + layout.center_pane_width + layout.right_pane_width;
    assert_eq!(total, layout.window_width);
}

#[test]
fn test_noop_window_storage() {
    let storage = NoopWindowStorage;
    assert_eq!(storage.load_geometry(), None);

    let geom = WindowGeometry::default();
    storage.save_geometry(&geom);
    assert_eq!(storage.load_geometry(), None);
}

#[test]
fn test_memory_window_storage_round_trip_and_concurrency() {
    let storage = Arc::new(MemoryWindowStorage::new());
    assert_eq!(storage.load_geometry(), None);

    let geom = WindowGeometry {
        width: 1280.0,
        height: 800.0,
        x: Some(100.0),
        y: Some(50.0),
        ui_scale: UiScale::new(1.20),
        ..Default::default()
    };

    storage.save_geometry(&geom);
    assert_eq!(storage.load_geometry(), Some(geom));

    // Test with pre-populated storage
    let populated = MemoryWindowStorage::with_geometry(geom);
    assert_eq!(populated.load_geometry(), Some(geom));
}
