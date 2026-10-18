//! Shared FastMD logo mark for the iced layer ([TASK-020]).
//!
//! The egui UI **vector-paints** the "Document + Bolt" badge with egui
//! painter primitives — `crate::ui::logo::paint_logo` draws the badge, the
//! document polygon, the folded corner, the markdown lines and the bolt
//! through `Painter::rect_filled` / `Shape::convex_polygon` /
//! `line_segment`. Those primitives have no iced equivalent under the
//! feature set DEC-011 enabled, so every iced surface keeps the Phosphor
//! bolt stand-in [TASK-011] already used by the top bar and the About
//! banner. This module exists so that stand-in is **built in exactly one
//! place** ([RUST-051]) instead of being re-typed per surface.
//!
//! # What the mark is
//!
//! [`logo_mark`] = [`logo_glyph`] (a Phosphor codepoint from
//! [`crate::ui::strings`], [GUD-002]) painted in the `accent_text`
//! palette token ([GUD-003]) at a caller-supplied point size. It emits no
//! message of its own, so the returned element is `'static` and drops into
//! any panel or card.
//!
//! # Why the real artwork is not rendered (feature trade-off)
//!
//! Verified against `iced_widget-0.14.2`/`iced_renderer-0.14.0` feature
//! tables and this workspace's `Cargo.lock`:
//!
//! | Option | iced feature | What it unlocks | Marginal cost (verified) |
//! | --- | --- | --- | --- |
//! | Raster badge | `image` → `iced_renderer/image` → `iced_graphics/image` | `iced::widget::image` rendering `crate::utils::app_icon::APP_ICON_PNG` (the real 32px mark) | `image` 0.25 is **already** in the tree (used by `utils::app_icon`); net new crate is `kamadak-exif` |
//! | Vector SVG | `svg` → `iced_wgpu/svg` + `iced_tiny_skia/svg` | `iced::widget::svg` over `assets/icon.svg` (present, 1.5 kB) | `resvg` (which brings `usvg` and its text stack) — **absent** from `Cargo.lock`; heaviest option |
//! | Re-painted vectors | `canvas` → `iced_renderer/geometry` | `iced::widget::canvas` — a `Program` re-implementing `paint_logo`'s polygons | `lyon` / `lyon_path` — **absent** from `Cargo.lock`; plus a hand port of ~140 lines of paint code |
//!
//! None of the three is enabled, and adding a cargo feature is a build-level
//! decision for the orchestrator ([TASK-020] constraint), not a view-layer
//! one. Until one is approved, [`logo_mark`] is the single source of the
//! stand-in and `crate::ui::logo::paint_logo` remains egui-only.
//!
//! Unit tests live in the sibling `logo_tests.rs` sidecar.

use iced::widget::text;
use iced::{Color, Element, Font};

use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::fonts::PHOSPHOR_FONT_NAME;
use crate::ui_iced::message::Message;
use crate::ui_iced::theme::to_iced_color;

#[cfg(test)]
#[path = "logo_tests.rs"]
mod tests;

/// Logical point size of the top-bar logo mark.
///
/// Parity with the `egui::vec2(20.0, 20.0)` box the egui toolbar
/// allocates for `crate::ui::logo::paint_logo`
/// (`src/app/ui/panels/top.rs:142`). The iced mark is a 20pt *glyph*
/// rather than a 20pt *box*, so the drawn artwork is slightly smaller
/// than the egui badge — inherent to the glyph stand-in.
pub const LOGO_MARK_SIZE: f32 = 20.0;

/// The glyph standing in for the "Document + Bolt" badge.
///
/// A single Phosphor codepoint ([`strings::ICON_LIGHTNING`]) — the mark is
/// a glyph, never a literal, so the codepoint lives in `strings.rs` like
/// every other user-facing character ([GUD-002]).
#[must_use]
pub fn logo_glyph() -> &'static str {
    strings::ICON_LIGHTNING
}

/// The colour the mark is painted in.
///
/// The `accent_text` palette token, converted once through
/// [`to_iced_color`] — no numeric colour literal appears here
/// ([GUD-003]).
#[must_use]
pub fn logo_color() -> Color {
    to_iced_color(Palette::fastmd_dark().accent_text)
}

/// The font handle the mark is drawn with.
///
/// The Phosphor family registered by [`crate::ui_iced::fonts`] at boot
/// ([TASK-007]); `PHOSPHOR_FONT_NAME` is the shared registration key, so
/// this is the same handle `panels::top::phosphor_font()` wraps.
#[must_use]
pub fn logo_font() -> Font {
    Font::with_name(PHOSPHOR_FONT_NAME)
}

/// Renders the logo mark at `size` logical points ([TASK-020]).
///
/// The one place the stand-in is constructed. The top bar passes
/// [`LOGO_MARK_SIZE`]; the About banner passes
/// [`crate::ui_iced::about::ABOUT_BANNER_SIZE`].
#[must_use]
pub fn logo_mark(size: f32) -> Element<'static, Message> {
    text(logo_glyph())
        .font(logo_font())
        .size(size)
        .color(logo_color())
        .into()
}
