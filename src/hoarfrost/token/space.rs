//! Spatial grid and spacing scale (`UIF-TOK-001`).
//!
//! All padding, gaps, margins and alignment offsets adhere to this base-4px
//! spatial grid, with [`XXS`] as the single 2px half-step reserved for
//! dense inline micro-gaps. A spacing value that is not one of these rungs
//! is not a spacing token and must not appear in layout code.
//!
//! Values are logical pixels. Read a rung by the role it plays rather than
//! by the number itself: [`MD`] is "the horizontal padding inside a
//! button", [`LG`] is "the inner padding of a card or dialog".
//!
//! Unit tests live in the sibling `space_tests.rs` sidecar.

/// `space.2xs` — micro gap between an anchor point and a floating menu,
/// and the tight intra-list row gap.
///
/// The only half-step on the base-4 grid; it exists so dense inline
/// content can breathe without a widget having to leave the scale.
pub const XXS: f32 = 2.0;

/// `space.xs` — compact inner padding for tags, badges, inline code, the
/// menu container inset and the tab close inset.
pub const XS: f32 = 4.0;

/// `space.sm` — standard sibling control gap, toolbar button spacing and
/// form row spacing.
pub const SM: f32 = 8.0;

/// `space.md` — standard horizontal padding inside buttons, inputs, tabs
/// and menu rows.
pub const MD: f32 = 12.0;

/// `space.lg` — card and dialog inner padding, and section separation.
pub const LG: f32 = 16.0;

/// `space.xl` — major layout section spacing and empty-state container
/// padding.
pub const XL: f32 = 24.0;

/// `space.2xl` — minimum viewport margin around large modal cards.
pub const XXL: f32 = 32.0;

#[cfg(test)]
#[path = "space_tests.rs"]
mod space_tests;
