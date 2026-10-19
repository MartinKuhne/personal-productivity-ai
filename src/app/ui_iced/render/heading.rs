//! Heading mapping and widget ([TASK-015]).
//!
//! The legacy UI renderer (`src/app/ui/render/heading.rs`) painted a heading as
//! a wrapped row of `RichText` runs at a level-dependent size. The iced
//! port keeps the same size ladder as named constants and reuses
//! [`crate::ui_iced::render::inline`] for the runs themselves, so a
//! heading's inline styling is produced by exactly one mapping
//! ([RUST-051]).
//!
//! Unit tests live in the sibling `heading_tests.rs` sidecar.

use super::inline::{SpanModel, spans_from_inline};
use crate::markdown::{InlineElem, heading_plain_text};
use crate::ui::theme::Palette;
use crate::ui_iced::message::Message;
use crate::ui_iced::render::inline::rich_text_element;
use iced::Element;
use std::path::Path;

/// Point size of an H1 heading.
///
/// Ported from `src/app/ui/render/heading.rs:41-47`.
pub const H1_SIZE: f32 = 32.0;

/// Point size of an H2 heading.
pub const H2_SIZE: f32 = 24.0;

/// Point size of an H3 heading.
pub const H3_SIZE: f32 = 18.0;

/// Point size of an H4 heading.
pub const H4_SIZE: f32 = 14.0;

/// Point size of an H5/H6 heading (the ladder's floor).
pub const H5_SIZE: f32 = 12.0;

/// Space left under a rendered heading.
///
/// Ported from the `ui.add_space(4.0)` that closed `render_heading`
/// (`src/app/ui/render/heading.rs:79`).
pub const HEADING_BOTTOM_SPACING: f32 = 4.0;

/// The point size a heading level renders at.
///
/// Levels outside 1..=6 collapse to [`H5_SIZE`], matching the `_ => 12.0`
/// arm of the legacy UI ladder.
#[must_use]
pub fn heading_size(level: u32) -> f32 {
    match level {
        1 => H1_SIZE,
        2 => H2_SIZE,
        3 => H3_SIZE,
        4 => H4_SIZE,
        _ => H5_SIZE,
    }
}

/// Whether a heading renders nothing.
///
/// the legacy UI skipped a heading whose plain text trimmed to empty
/// (`src/app/ui/render/heading.rs:37-40`), which also keeps it out of the
/// scroll-id bookkeeping.
#[must_use]
pub fn is_blank_heading(elems: &[InlineElem]) -> bool {
    heading_plain_text(elems).trim().is_empty()
}

/// Maps a heading's inline elements onto span models at the level's size.
#[must_use]
pub fn heading_spans(elems: &[InlineElem], level: u32, palette: Palette) -> Vec<SpanModel> {
    spans_from_inline(elems, heading_size(level), palette)
}

/// Renders a heading block from already-mapped spans.
#[must_use]
pub fn heading_block(spans: &[SpanModel], source_file: Option<&Path>) -> Element<'static, Message> {
    rich_text_element(spans, source_file)
}

#[cfg(test)]
#[path = "heading_tests.rs"]
mod tests;
