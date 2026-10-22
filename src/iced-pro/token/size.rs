//! Sizing, hit targets and control geometry (`UIF-TOK-002`).
//!
//! Three independent scales live here, and they answer different
//! questions. The control rungs ([`CONTROL_SM`], [`CONTROL_MD`],
//! [`CONTROL_LG`]) say how tall a given class of control is; the icon
//! rungs ([`ICON_SM`], [`ICON_MD`], [`ICON_LG`]) say how large a glyph
//! rendered inside one is. [`HIT_TARGET_MIN`] is the floor both must
//! clear — it is not a size to pick, it is the rule every pick is checked
//! against.
//!
//! The third group is neither a control nor a glyph: the compact status
//! bar ([`BAR_STATUS`]), the unsaved-state dot ([`INDICATOR_DOT`]) and the
//! splitter grab band ([`SPLITTER_GRAB`]) size a bar, a marker and a pane
//! boundary. None of them is a control you press, so none is measured
//! against the hit-target floor — which is exactly why they get their own
//! names instead of borrowing a rung from a scale that was never about
//! them.
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

/// `size.bar.status` — the compact status and footer bar height: the
/// movable overlay's footer (§4.7.1) and a menu's section heading row
/// (§4.8.1).
///
/// The number is the same as [`HIT_TARGET_MIN`]; the meaning is not. That
/// constant is a *rule* an interactive control is measured against, this
/// one is the height a bar is built to. A footer is not a control, so it
/// does not "clear the floor" by being checked against it — it happens to
/// be built at the same 24px, and keeping the two names apart is what
/// makes moving one without the other a deliberate act rather than a
/// side effect of a retune.
pub const BAR_STATUS: f32 = 24.0;

/// `size.control.sm` — compact controls: navbar buttons, dropdown triggers
/// and inline search controls.
pub const CONTROL_SM: f32 = 28.0;

/// `size.control.md` — standard controls: dialog CTA buttons, single-line
/// text inputs and selectable menu rows.
pub const CONTROL_MD: f32 = 32.0;

/// `size.control.lg` — primary structural bars: the Top Navbar height and
/// the Tab Strip height.
pub const CONTROL_LG: f32 = 36.0;

/// `size.indicator.dot` — the unsaved-state dot (`●`) diameter.
///
/// A state marker, not a glyph: it is read as *presence*, not as an
/// icon with meaning of its own. That is why it sits below every icon rung
/// rather than on one — at [`ICON_SM`] an unsaved flag would be as loud
/// as a close `×`, and a marker that shouts as loudly as an action stops
/// being a marker.
pub const INDICATOR_DOT: f32 = 6.0;

/// `size.splitter.grab` — the interactive pointer grab band centred over
/// a `1px` pane split divider: `10px` wide, i.e. `5px` each side of the
/// divider's centre line.
///
/// **No consumer exists in this crate.** Decision **D-2** leaves pane
/// splitters app-owned, so nothing here builds, paints or hit-tests one.
/// The token is declared anyway, and deliberately so: `UIF-TOK-002`
/// defines it, and the alternative to declaring it is an application
/// inventing its own number for the same measurement — which would put a
/// hole straight through **UIF-PRIN-001**'s claim that every visual
/// number lives under `token/`. A splitter added later, here or in the
/// app, reads this value rather than a literal.
///
/// The band is wider than the hairline rule it is centred over on
/// purpose: the divider stays quiet, the grip stays grabbable. Note that
/// `10` is **below** [`HIT_TARGET_MIN`], and that is the spec's own
/// number, not an oversight to "fix" by borrowing a control rung — a
/// grab band is a strip you find by feel between two panes, not a control
/// that advertises itself. The sidecar pins the relationship so nobody
/// reads the shortfall as a bug and inflates it.
pub const SPLITTER_GRAB: f32 = 10.0;

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
