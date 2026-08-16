//! Tests for [`crate::ui::test_helpers::assert`].
//! Lives in a sidecar so the implementation file stays focused.

use super::*;
use eframe::egui;

#[test]
fn test_assert_no_id_change_warnings_pass() {
    let log_msgs: Vec<String> = vec![];
    let shapes: Vec<egui::Shape> = vec![];

    // Should not panic
    assert_no_id_change_warnings(&log_msgs, &shapes);
}

#[test]
#[should_panic(expected = "id-stability regression")]
fn test_assert_no_id_change_warnings_fail_log() {
    let log_msgs: Vec<String> =
        vec!["WARN egui::context: Widget rect changed id between passes".to_string()];
    let shapes: Vec<egui::Shape> = vec![];

    assert_no_id_change_warnings(&log_msgs, &shapes);
}

#[test]
#[should_panic(expected = "id-stability regression")]
fn test_assert_no_id_change_warnings_fail_shape() {
    let log_msgs: Vec<String> = vec![];

    let rect_shape = egui::epaint::RectShape::new(
        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::ZERO),
        0.0,
        egui::Color32::TRANSPARENT,
        egui::Stroke::new(1.0, egui::Color32::RED),
        egui::StrokeKind::Inside,
    );
    let shapes: Vec<egui::Shape> = vec![egui::Shape::Rect(rect_shape)];

    assert_no_id_change_warnings(&log_msgs, &shapes);
}

#[test]
fn test_assert_no_id_change_in_shapes_pass() {
    let shapes: Vec<egui::Shape> = vec![];
    assert_no_id_change_in_shapes(&shapes);
}

#[test]
#[should_panic(expected = "id-stability regression")]
fn test_assert_no_id_change_in_shapes_fail() {
    let rect_shape = egui::epaint::RectShape::new(
        egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::ZERO),
        0.0,
        egui::Color32::TRANSPARENT,
        egui::Stroke::new(1.0, egui::Color32::RED),
        egui::StrokeKind::Inside,
    );
    let shapes: Vec<egui::Shape> = vec![egui::Shape::Rect(rect_shape)];

    assert_no_id_change_in_shapes(&shapes);
}

#[test]
fn test_assert_no_id_change_in_log_pass() {
    let log_msgs: Vec<String> = vec![];
    assert_no_id_change_in_log(&log_msgs);
}

#[test]
#[should_panic(expected = "id-stability regression")]
fn test_assert_no_id_change_in_log_fail() {
    let log_msgs: Vec<String> =
        vec!["WARN egui::context: Widget rect changed id between passes".to_string()];
    assert_no_id_change_in_log(&log_msgs);
}
