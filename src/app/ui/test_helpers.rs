//! Shared test helpers for egui integration tests.
//!
//! Two patterns live here, both project-specific and not part of the
//! `egui_kittest` public API. They are documented in
//! `doc/planning/egui-testing.md` §"Patterns observed in practice"; this
//! file is the code-level home.
//!
//! * **State-capture pattern** — wraps the state-capture pattern
//!   for Tier 4 click tests. The naive `harness.output()` after `run()`
//!   silently fails in egui 0.35 because `PlatformOutput::commands` is
//!   per-frame. Capture the side effect into the harness's persistent
//!   `state()` at the moment the click fires. (See `interact` module
//!   when built with tests enabled.)
//!
//! * **[`assert::assert_no_id_change_warnings`]** — wraps the id-stability
//!   test pattern. The "widget rect changed id between passes" warning is
//!   the project's most common egui bug class; it can be detected via two
//!   complementary mechanisms (red-stroke shape + log capture), and the
//!   combined helper runs both.
//!
//! These helpers are `#[cfg(test)]`-gated by their declaration in
//! `crate::ui::mod` — they are not part of the production crate.

pub mod app;
pub mod assert;
#[cfg(test)]
pub mod interact;
pub mod offscreen;
#[cfg(test)]
pub mod snapshot;
pub mod text;

use eframe::egui;

/// Executes `ctx.run_ui` in unit/integration tests and clears `textures_delta`
/// on the returned `FullOutput` so it can be safely dropped under `epaint 0.36`.
pub fn run_ui_test(
    ctx: &egui::Context,
    raw_input: egui::RawInput,
    add_contents: impl FnMut(&mut egui::Ui),
) -> egui::FullOutput {
    let mut output = ctx.run_ui(raw_input, add_contents);
    output.textures_delta.clear();
    output
}
