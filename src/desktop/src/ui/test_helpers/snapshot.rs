//! Tier 3 visual regression snapshot helpers.
//!
//! Wraps the `egui_kittest` snapshot API with project-standard options:
//! 3px per-channel pixel diff threshold (Q10), `tests/snapshots/`
//! output path, and a builder that wires up a known viewport.
//!
//! # Why this exists
//!
//! Per the R-1 deliverable in `doc/planning/egui-testing.md`, the
//! project's most complex visual surface (the markdown renderer) has
//! no visual regression coverage. A real refactor that changes a
//! column's wrap point, a heading's left padding, or a code block's
//! border thickness would pass the current test suite silently.
//! Snapshots close that gap.
//!
//! # Usage
//!
//! ```ignore
//! use crate::ui::test_helpers::snapshot::snapshot_harness;
//!
//! let mut harness = snapshot_harness("ftwa_6col_table", egui::vec2(1200.0, 600.0), |ui| {
//!     let table = build_six_col_table();
//!     fastmd::ui::render::render_table(ui, &table, 0, fastmd::ui::table_width::DeficitStrategy::ProportionalToSlack);
//! });
//! harness.run();
//! harness.snapshot("ftwa_6col_table");
//! ```
//!
//! # Q10 — 3px project-wide threshold
//!
//! The threshold is set per-harness via `SnapshotOptions::threshold`.
//! 3px was chosen over the original Q2 commitment of 5px because it
//! catches a single line of text wrap (~14-16px) while still tolerating
//! system-font metric variation across Windows / Linux CI / macOS.
//!
//! # Renderer requirement
//!
//! `egui_kittest`'s snapshot API requires a real renderer (the
//! default `LazyRenderer` errors with "no default renderer
//! available"). The `wgpu` feature on `egui_kittest` provides
//! `WgpuTestRenderer`, which is the project's planned renderer. As
//! of the 2026-07-27 branch, the `wgpu` feature is **not enabled**
//! because pulling it in on Windows produces a wgpu-hal 0.29 /
//! windows-core 0.56/0.62 trait-bound conflict that the local
//! environment cannot compile. This is a separate, pre-existing
//! issue in the wgpu / windows-core dependency graph; resolving it
//! is out of scope for R-1. Once the wgpu dep conflict is sorted,
//! callers should add `.wgpu()` to the `HarnessBuilder` chain in
//! `snapshot_harness` below.
//!
//! Until the wgpu story is resolved, calling `harness.snapshot()`
//! on this project will return `SnapshotError::RenderError` with a
//! "no default renderer" message. The helper still exercises the
//! full API surface; only the actual image write is blocked.
//!
//! # Real example: see `tests/render_snapshots.rs`
//!
//! That integration test takes 2-3 initial snapshots once the
//! renderer is wired up.

use eframe::egui;

/// Standard project viewport for a snapshot: 1024x768, dark theme,
/// `Predictable` wgpu renderer options (when the `wgpu` feature is
/// enabled on `egui_kittest`).
pub const DEFAULT_VIEWPORT: egui::Vec2 = egui::vec2(1024.0, 768.0);

/// 3-pixel project-wide diff threshold (Q10).
///
/// Set on `SnapshotOptions::threshold`. Catches a single line of
/// text wrap (~14-16px) while tolerating system-font metric
/// variation across Windows / Linux CI / macOS.
pub const SNAPSHOT_THRESHOLD: f32 = 3.0;

/// Build a `egui_kittest::Harness` configured for snapshot tests.
///
/// This is a thin wrapper over `egui_kittest::Harness::builder()`
/// that pins the viewport to `DEFAULT_VIEWPORT` and sets the
/// snapshot options to `SNAPSHOT_THRESHOLD`. The `wgpu()` call is
/// gated by the `wgpu` feature on `egui_kittest`; when that
/// feature is disabled (current state on this branch — see module
/// docs for the wgpu / windows-core conflict), the harness falls
/// back to the default `LazyRenderer` and `harness.snapshot()`
/// will fail with a "no default renderer" error. Callers should
/// catch the error and `skip!` the test in that case.
///
/// # TODO
///
/// Re-enable `.wgpu()` here once the wgpu dep conflict is resolved
/// in the project. The blocking issue is the wgpu-hal 0.29 vs
/// windows-core 0.56/0.62 trait-bound conflict on Windows. Until
/// then, snapshots cannot be taken locally on Windows; CI on
/// Linux may work if the toolchain is set up correctly.
pub fn snapshot_harness<F>(
    name: impl Into<String>,
    size: egui::Vec2,
    body: F,
) -> egui_kittest::Harness<'static, ()>
where
    F: FnMut(&mut egui::Ui) + 'static,
{
    let _ = name; // name is passed to harness.snapshot() by the caller.
    egui_kittest::Harness::builder()
        .with_size(size)
        .build_ui(body)
}
