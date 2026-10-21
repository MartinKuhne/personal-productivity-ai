//! Sizing, hit targets and control geometry (`UIF-TOK-002`).
//!
//! Two independent scales live here, and they answer different questions.
//! The control rungs ([`CONTROL_SM`], [`CONTROL_MD`], [`CONTROL_LG`]) say
//! how tall a given class of control is; the icon rungs ([`ICON_SM`],
//! [`ICON_MD`], [`ICON_LG`]) say how large a glyph rendered inside one is.
//! [`HIT_TARGET_MIN`] is the floor both must clear — it is not a size to
//! pick, it is the rule every pick is checked against.
//!
//! Values are logical pixels.
//!
//! Unit tests live in the sibling `size_tests.rs` sidecar.

/// `size.hit_target.min` — the mandatory minimum pointer hit area of
/// `24 × 24` for any interactive control: a close `×`, a clear `×`, an
/// icon button, a tree or TOC row.
///
/// The floor applies to **both** dimensions; a control that is 40px wide
/// and 20px tall still violates it.
pub const HIT_TARGET_MIN: f32 = 24.0;

/// `size.control.sm` — compact controls: navbar buttons, dropdown triggers
/// and inline search controls.
pub const CONTROL_SM: f32 = 28.0;

/// `size.control.md` — standard controls: dialog CTA buttons, single-line
/// text inputs and selectable menu rows.
pub const CONTROL_MD: f32 = 32.0;

/// `size.control.lg` — primary structural bars: the Top Navbar height and
/// the Tab Strip height.
pub const CONTROL_LG: f32 = 36.0;

/// `size.icon.sm` — inline status, badge and close `×` glyph optical size.
pub const ICON_SM: f32 = 14.0;

/// `size.icon.md` — standard button, menu leading slot and tree item icon
/// size.
pub const ICON_MD: f32 = 16.0;

/// `size.icon.lg` — brand mark and empty-state illustration icon size.
pub const ICON_LG: f32 = 20.0;

#[cfg(test)]
#[path = "size_tests.rs"]
mod size_tests;
