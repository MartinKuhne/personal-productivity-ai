//! Heading mapping and widget.
//!
//! A heading renders as a wrapped row of rich-text runs at a level-dependent
//! size. The size ladder lives in the named constants below and the runs
//! themselves come from [`crate::ui_iced::render::inline`], so a heading's
//! inline styling is produced by exactly one mapping ([RUST-051]).
//!
//! Unit tests live in the sibling `heading_tests.rs` sidecar.

use super::inline::{SpanModel, spans_from_inline};
use crate::markdown::{InlineElem, heading_plain_text};
use crate::ui::theme::Palette;
use crate::ui_iced::message::Message;
use crate::ui_iced::render::inline::rich_text_element;
use iced::Element;
use iced_pro::token::space;
use iced_pro::token::typography;
use std::path::Path;

/// Point size of an H1 heading — `type.title_lg` (`UIF-TOK-004`).
///
/// **Movement: 32 → 24 (−8).** `32` is not a rung: the scale tops out at
/// `TITLE_LG` = 24, so the old ladder's top step was 8 pt above the largest
/// size the interface is allowed to render. §2.4 names `type.title_lg` as
/// the *document H1* rung outright, so this is the spec's own number rather
/// than a compromise. The heading ladder is now the type scale read from the
/// top down, with no invented steps between the rungs.
pub const H1_SIZE: f32 = typography::TITLE_LG.size;

/// Point size of an H2 heading — `type.title_md` (`UIF-TOK-004`).
///
/// **Movement: 24 → 20 (−4).** §2.4 assigns `type.title_md` to "Document
/// H2 headings" by name. The old `24` collided with the *H1* rung, so an H2
/// used to be drawn at the size the scale reserves for H1 and the ladder
/// compressed everything below it by one step.
pub const H2_SIZE: f32 = typography::TITLE_MD.size;

/// Point size of an H3 heading — `type.title_sm` (`UIF-TOK-004`).
///
/// **Movement: 18 → 16 (−2).** `18` is not a rung — the same off-scale
/// number INT-7c removed from [`DOCUMENT_TITLE_SIZE`] and
/// [`AGENT_HEADER_SIZE`] in the center panel. `TITLE_SM` is the smallest
/// *title* rung, which is what an H3 needs to stay above body prose.
///
/// [`DOCUMENT_TITLE_SIZE`]: crate::ui_iced::panels::center::DOCUMENT_TITLE_SIZE
/// [`AGENT_HEADER_SIZE`]: crate::ui_iced::panels::center::AGENT_HEADER_SIZE
pub const H3_SIZE: f32 = typography::TITLE_SM.size;

/// Point size of an H4 heading — `type.body` (`UIF-TOK-004`).
///
/// Value unchanged at `14`. An H4 sits on the body rung and is distinguished
/// from prose by weight and position rather than by size, which is what the
/// scale leaves it: there is no rung between 14 and 16.
pub const H4_SIZE: f32 = typography::BODY.size;

/// Point size of an H5/H6 heading (the ladder's floor) — `type.caption`
/// (`UIF-TOK-004`).
///
/// Value unchanged at `12`, which is also [`typography::MIN_SIZE`] — the
/// interface-wide legibility floor. The ladder therefore lands exactly on the
/// floor and never steps below it, which is the property the floor test
/// checks.
pub const H5_SIZE: f32 = typography::CAPTION.size;

/// Space left under a rendered heading — `space.xs` (`UIF-TOK-001`).
/// Value unchanged at `4`.
pub const HEADING_BOTTOM_SPACING: f32 = space::XS;

/// The point size a heading level renders at.
///
/// Levels outside 1..=6 collapse to [`H5_SIZE`].
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
/// A heading whose plain text trims to empty renders nothing, which also keeps
/// it out of the scroll-id bookkeeping.
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
