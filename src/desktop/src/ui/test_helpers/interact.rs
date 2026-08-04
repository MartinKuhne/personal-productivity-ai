//! Tier 4 interaction test helpers — the state-capture pattern.
//!
//! # Why this exists
//!
//! egui 0.35 replaced `PlatformOutput::copied_text` / `open_url` with
//! `PlatformOutput::commands: Vec<OutputCommand>`. Worse, `PlatformOutput`
//! is now per-frame — every new pass starts a fresh `PlatformOutput`.
//! A click that emits `OutputCommand::CopyText(text)` on frame N has that
//! command **overwritten on frame N+1**, before the test can read it.
//!
//! This is the opposite of what `Harness::output()` would suggest. The
//! naive pattern — "render, click, render, assert on
//! `harness.output().platform_output.commands`" — silently fails because
//! the `run()`-and-settle cycle has already started a new pass and erased
//! the click's output.
//!
//! # The fix
//!
//! Capture the click's side effect into the harness's **persistent state**
//! at the moment it fires. The harness's `state()` is preserved across
//! frames; the closure's `T` argument is the same `T` for every frame.
//! Push the side effect onto a `Vec` (or set a `bool`, or write a
//! `String`) inside that state, then read `harness.state()` after
//! `harness.step()` to see what fired.
//!
//! # Canonical example
//!
//! ```ignore
//! use crate::ui::test_helpers::interact::stateful_harness;
//!
//! let mut harness = stateful_harness(Vec::<String>::new(), |ui, captured| {
//!     if ui.button("Copy").clicked() {
//!         ui.copy_text("let x = 1;".to_string());
//!         captured.push("let x = 1;".to_string());
//!     }
//! });
//! harness.fit_contents();
//! harness.run();
//! harness.get_by_label("Copy").click();
//! // Two runs after the click: the first processes pointer events
//! // (hover + press + release = three steps), the second settles
//! // any post-click repaint.
//! harness.run();
//! harness.run();
//! let captured = harness.state();
//! assert_eq!(captured, vec!["let x = 1;".to_string()]);
//! ```
//!
//! Real example: `src/ui/render.rs::test_copy_code_button_click_copies_to_output`.
//!
//! # When *not* to use state-capture
//!
//! When the side effect lives only in `PlatformOutput::commands` and
//! cannot be intercepted at the closure level (e.g. `OpenUrl` from a
//! `Link` widget), the alternative is to use `harness.step()` (not
//! `run()`) to drive the click and read `harness.output()` in the narrow
//! window before the next frame overwrites the command. See
//! `src/ui/render.rs::test_hyperlink_click_opens_url` for the pattern.

/// Build a stateful egui_kittest `Harness` for a Tier 4 click test.
///
/// The closure receives `&mut egui::Ui` and `&mut T`. `T` is the
/// persistent state and is re-used across frames (measure, paint, click,
/// repaint). The closure is re-invoked for each pass — push the click's
/// side effect into `T`, then assert on `harness.state()` after the
/// click has settled.
///
/// This is a thin alias for `egui_kittest::Harness::new_ui_state`. It
/// exists to give the pattern a project-specific name and a doc-comment
/// anchor; the harness API itself is unchanged.
///
/// The lifetime parameter `'a` ties the harness to the closure's borrow
/// lifetime. In practice, this is the lifetime of the test function —
/// the closure is non-`'static` and the harness borrows from it.
pub fn stateful_harness<'a, T, F>(initial_state: T, body: F) -> egui_kittest::Harness<'a, T>
where
    T: 'static,
    F: FnMut(&mut eframe::egui::Ui, &mut T) + 'a,
{
    egui_kittest::Harness::new_ui_state(body, initial_state)
}
