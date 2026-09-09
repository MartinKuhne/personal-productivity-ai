//! Id-stability test pattern helpers.
//!
//! # Why this exists
//!
//! The most common egui bug class in this project is the
//! `WARN egui::context: Widget rect ... changed id between passes`
//! warning. The AGENTS.md §"Conditional rendering" section documents the
//! root cause and the fix (always allocate, toggle visibility with
//! `add_visible` / `set_invisible`). Until a test exists for a panel,
//! the warning can be reintroduced by a well-meaning refactor that wraps
//! a panel in `if cond { Panel::right("...").show(...) }` and the `if`
//! arm's allocation shape changes between passes.
//!
//! # Two complementary detection mechanisms
//!
//! Both are needed because they can fail independently:
//!
//! 1. **Red-stroke shape detection** — egui's
//!    `Context::warn_if_rect_changes_id` draws a red `Shape::Rect` outline
//!    in the second pass when it detects the warning. The test renders
//!    the panel twice (priming the first-pass state, then observing the
//!    second pass), walks `output.shapes`, and asserts no shape has
//!    `stroke.color == Color32::RED`.
//!
//! 2. **Log capture** — egui emits the warning to the `log` crate. A test
//!    can install a `log::Log` impl, render across a bool flip, and
//!    grep the captured messages.
//!
//! The red-stroke mechanism can be suppressed by `ctx.set_theme(Light)`
//! (the warning's red debug colour is hidden against certain
//! backgrounds), and the log-capture mechanism can be lost if the test
//! process has another `log::set_logger` installed. Belt-and-suspenders.
//!
//! # Existing test sites
//!
//! - `panels/top.rs:289` — log capture, covers the `indexing_finished`
//!   bool flip on the toolbar.
//! - `panels/left.rs:451` — red-stroke, covers the file tree, empty
//!   state, transition, and width-clamping.
//! - `ui/app.rs:1352` — red-stroke, covers the full 5-panel render with
//!   TOC active.
//!
//! # Usage
//!
//! ```ignore
//! use crate::ui::test_helpers::assert::assert_no_id_change_warnings;
//!
//! // Install log capture (one-time, guarded by OnceLock<()>).
//! // ... (see panels/top.rs:289-360 for the full pattern)
//!
//! let output = ctx.run_ui(raw_input, |ui| {
//!     show_left_panel(&mut app, ui);
//! });
//! assert_no_id_change_warnings(&captured_log_msgs, &output.shapes);
//! ```
//!
//! Unit tests live in the sibling `assert_tests.rs` sidecar.

use crate::bus::core::BusReader;
use crate::bus::events::user_command::UserCommand;
use crate::ui::app::FastMdApp;
use eframe::egui;

/// Assert that no "widget rect changed id between passes" warnings were
/// emitted in either the `log` output or the rendered shapes.
///
/// Run both checks. If a test installs log capture, pass `log_msgs` with
/// the captured messages; if not, pass an empty slice and the helper will
/// only run the shape check. Same in reverse for `shapes`.
///
/// Accepts `&[Shape]` for `shapes` (the inner `Shape` of
/// `ClippedShape`); the `clip_rect` field is not needed for the
/// red-stroke detection. Callers can pass
/// `output.shapes.into_iter().map(|cs| cs.shape).collect()` to convert
/// from `Vec<ClippedShape>`.
pub fn assert_no_id_change_warnings(log_msgs: &[String], shapes: &[egui::Shape]) {
    let log_warns = log_msgs
        .iter()
        .filter(|m| m.contains("changed id between passes"))
        .count();
    let shape_warns = shapes
        .iter()
        .filter(|shape| {
            matches!(
                shape,
                egui::Shape::Rect(r) if r.stroke.color == egui::Color32::RED
            )
        })
        .count();
    assert!(
        log_warns == 0 && shape_warns == 0,
        "id-stability regression: {log_warns} log warning(s), {shape_warns} \
         red-stroke rect(s) in the rendered output. See \
         AGENTS.md §\"Conditional rendering\" for the fix pattern."
    );
}

/// Assert no "widget rect changed id between passes" warnings in the
/// rendered shapes only. Use this when the test does not install log
/// capture and only the visual side of the detection is needed.
///
/// Accepts `&[Shape]` rather than `&[ClippedShape]` so the call site
/// can pass `output.shapes.into_iter().map(|cs| cs.shape).collect()`
/// without depending on the `clip_rect` field.
pub fn assert_no_id_change_in_shapes(shapes: &[egui::Shape]) {
    let shape_warns = shapes
        .iter()
        .filter(|shape| {
            matches!(
                shape,
                egui::Shape::Rect(r) if r.stroke.color == egui::Color32::RED
            )
        })
        .count();
    assert_eq!(
        shape_warns, 0,
        "id-stability regression: {shape_warns} red-stroke rect(s) in the \
         rendered output. See AGENTS.md §\"Conditional rendering\" for the \
         fix pattern."
    );
}

/// Assert no "widget rect changed id between passes" warnings in the log
/// output only. Use this when the test only installs log capture and
/// does not need to inspect the rendered shapes.
pub fn assert_no_id_change_in_log(log_msgs: &[String]) {
    let log_warns = log_msgs
        .iter()
        .filter(|m| m.contains("changed id between passes"))
        .count();
    assert_eq!(
        log_warns, 0,
        "id-stability regression: {log_warns} log warning(s). See \
         AGENTS.md §\"Conditional rendering\" for the fix pattern."
    );
}

/// Assert that the bus contains the expected [`UserCommand`].
///
/// Drains `reader` via `try_recv_exposing_lag` and asserts the expected
/// command was published. Pure helper with no global state.
pub fn assert_bus_contains(reader: &mut BusReader<UserCommand>, expected: UserCommand) {
    let mut found = false;
    let mut seen = Vec::new();
    while let Ok(cmd) = reader.try_recv_exposing_lag() {
        seen.push(cmd.clone());
        if cmd == expected {
            found = true;
            break;
        }
    }
    assert!(
        found,
        "Expected bus to contain {expected:?}, but saw: {seen:?}"
    );
}

/// Assert that the app's user-command bus contains the expected [`UserCommand`].
///
/// Convenience wrapper for panel tests that own a `FastMdApp`. Reads from
/// `app.orchestrator.user_command_reader`.
pub fn assert_app_bus_contains(app: &mut FastMdApp, expected: UserCommand) {
    let reader = app
        .orchestrator
        .user_command_reader
        .as_mut()
        .expect("user_command_reader must be set on test app");
    assert_bus_contains(reader, expected);
}

#[cfg(test)]
#[path = "assert_tests.rs"]
mod assert_tests;
