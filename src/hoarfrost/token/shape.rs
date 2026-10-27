//! Corner radii and stroke weights (`UIF-TOK-003`).
//!
//! The radii form a nesting: the larger the surface, the rounder its
//! corners, so a widget's radius tells you which layer of the interface it
//! belongs to before you read its contents. Strokes are deliberately
//! coarser — one resting weight and one attention weight — so a focus ring
//! is never mistakable for a border.
//!
//! Values are logical pixels.
//!
//! Unit tests live in the sibling `shape_tests.rs` sidecar.

/// `radius.window` — the outer application window frame radius.
///
/// The largest radius in the set; nothing inside the frame may be rounder
/// than the frame containing it.
pub const RADIUS_WINDOW: f32 = 8.0;

/// `radius.card` — floating modal dialogs, input dialogs, movable overlay
/// windows and menu cards.
pub const RADIUS_CARD: f32 = 6.0;

/// `radius.widget` — interactive controls: buttons, text inputs,
/// dropdowns, checkboxes and code blocks.
pub const RADIUS_WIDGET: f32 = 4.0;

/// `radius.pill` — status chips and numeric count badges.
///
/// A deliberately absurd radius rather than a measured one: any value this
/// far past the height of the chip renders as a fully rounded capsule at
/// every size the chip can take.
pub const RADIUS_PILL: f32 = 999.0;

/// `border.hairline` — the default structural stroke: dividers, card
/// perimeters, resting input borders and table borders.
pub const BORDER_HAIRLINE: f32 = 1.0;

/// `border.focus` — the attention stroke: the keyboard focus ring, the
/// active tab accent bar, and the focused or invalid input border.
///
/// Twice [`BORDER_HAIRLINE`], so an element signalling focus never reads
/// as one that simply has a heavier border.
pub const BORDER_FOCUS: f32 = 2.0;

#[cfg(test)]
#[path = "shape_tests.rs"]
mod shape_tests;
