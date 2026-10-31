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
//! The fourth group is **part geometry**: the checkbox box
//! ([`CHECKBOX_BOX`]), the switch track ([`SWITCH_TRACK_WIDTH`],
//! [`SWITCH_TRACK_HEIGHT`]) and thumb ([`SWITCH_THUMB`]), the slider rail
//! ([`SLIDER_TRACK_HEIGHT`]) and thumb ([`SLIDER_THUMB`]) and the progress
//! bar ([`PROGRESS_BAR_HEIGHT`]) each size a thing that is *inside* a
//! control rather than the control's own height, which comes from the
//! control rungs. [`SPINNER_SM`] and [`SPINNER_MD`] are the spinner
//! rungs; the small one is the slot a busy button reserves, which is why
//! [`crate::widget::button_model::SPINNER_SLOT`] names it.
//!
//! Several of these have no consumer in this crate yet — the slider and
//! progress rungs and [`SPINNER_MD`]. They are declared anyway, on the
//! [`SPLITTER_GRAB`] precedent: `UIF-TOK-002` defines them, and the
//! alternative to declaring a defined token is an application inventing
//! its own number for the same measurement, which puts a hole straight
//! through **UIF-PRIN-001**. Each such constant says so in its own doc.
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

// ---------------------------------------------------------------------------
//  Part geometry (§2.2) — the box inside a control, not the control's height
// ---------------------------------------------------------------------------

/// `size.checkbox.box` — the `16 × 16` visual glyph box for a `Checkbox`
/// or `Radio` indicator.
///
/// This sizes the *indicator*, not the control: the box sits centred
/// inside a [`HIT_TARGET_MIN`] target that extends across the inline
/// label, so a `16` glyph never means a `16` click area
/// (`UIF-ATOM-003`, §3.3). Both atoms use the same box — the `Radio` is
/// the same square drawn as a circle, which is what lets the two share
/// one geometry function and one hit test.
pub const CHECKBOX_BOX: f32 = 16.0;

/// `size.switch.track` — outer width of a `ToggleSwitch` pill track.
pub const SWITCH_TRACK_WIDTH: f32 = 36.0;

/// `size.switch.track` — outer height of a `ToggleSwitch` pill track.
pub const SWITCH_TRACK_HEIGHT: f32 = 20.0;

/// `size.switch.thumb` — the `14 × 14` sliding thumb of a `ToggleSwitch`.
///
/// The spec fixes the thumb's inset in prose rather than as a token of its
/// own; [`SWITCH_THUMB_INSET`] is that sentence made arithmetic.
pub const SWITCH_THUMB: f32 = 14.0;

/// The gap between a `ToggleSwitch` thumb and the track edge — `3px`, per
/// the "`3px` inset from track edges" note on `size.switch.thumb`.
///
/// **Derived, not declared.** §2.2 gives no `size.switch.inset`; it gives
/// the track and the thumb and states the inset in the thumb's own row.
/// Deriving it here rather than writing `3.0` means the three numbers
/// cannot drift apart: retune the track or the thumb and the inset moves
/// with them, instead of leaving a `3` that no longer fits.
pub const SWITCH_THUMB_INSET: f32 = (SWITCH_TRACK_HEIGHT - SWITCH_THUMB) / 2.0;

/// `size.slider.track_height` — horizontal rail thickness for a `Slider`.
///
/// Consumed by [`crate::widget::slider::TRACK_HEIGHT`], which is the rail
/// the thumb rides on, and by nothing else. The tick marks that hang off
/// the same rail are **not** priced here: §2.1's role column assigns the
/// slider tick width to `space.2xs` and the slider tick height to
/// `space.xs`, so a tick is bought on the spacing grid
/// ([`crate::widget::slider::TICK_WIDTH`],
/// [`crate::widget::slider::TICK_HEIGHT`]) rather than by borrowing the
/// rail's own thickness. The two happen to measure the same `4`, which is
/// a coincidence the sidecar pins rather than a definition.
pub const SLIDER_TRACK_HEIGHT: f32 = 4.0;

/// `size.slider.thumb` — the `16 × 16` visual thumb of a `Slider`.
///
/// Consumed by [`crate::widget::slider::THUMB_SIZE`]. Note that the visual
/// thumb is `16` while the pointer target §2.2 asks for is
/// [`HIT_TARGET_MIN`] (`24`): the same indicator-versus-target split
/// [`CHECKBOX_BOX`] carries. The `4` left over per side is not slack — it is
/// what the focus ring is drawn inside, which is why the ring sits `2px` off
/// the thumb rather than touching it.
pub const SLIDER_THUMB: f32 = 16.0;

/// `size.progress.bar_height` — linear `ProgressBar` track thickness.
///
/// **No consumer exists in this crate yet** — see
/// [`SLIDER_TRACK_HEIGHT`].
pub const PROGRESS_BAR_HEIGHT: f32 = 4.0;

/// `size.spinner.sm` — inline busy `Spinner` diameter, for the slot a
/// button or input reserves while its action is in flight.
///
/// The value is the same as [`ICON_SM`], and that is the point: the busy
/// indicator occupies exactly the small-icon box, which is why
/// [`crate::widget::button_model::SPINNER_SLOT`] is this rung rather
/// than a borrowed icon constant. The two names are kept apart because
/// they answer different questions — "how big is a glyph" and "how big
/// is the spinner" — and a spinner that later needed a different diameter
/// must not have to take the glyph scale with it.
pub const SPINNER_SM: f32 = 14.0;

/// `size.spinner.md` — panel or dialog loading `Spinner` diameter.
///
/// **No consumer exists in this crate yet** — see
/// [`SLIDER_TRACK_HEIGHT`].
pub const SPINNER_MD: f32 = 20.0;

/// Count badge height — §3.8's `18px h` pill.
///
/// A separate rung from [`SPINNER_SM`] even though both are small-circle
/// sized, because they answer different requirements: one is a control's
/// busy glyph, the other is a count's container. Binding them together
/// would let a change to the spinner silently resize a badge.
///
/// The `18.0` is the *whole* pill height. The `12px` label §3.8 puts
/// inside it is what makes the pill read as a badge rather than as a
/// `12`-pixel box with padding, so the height is not derived from the
/// type rung — it is the number the spec prints.
pub const BADGE_COUNT_HEIGHT: f32 = 18.0;

/// Status chip height — §3.8's `24px h` pill.
///
/// The chip is the only badge that carries a *word*, so it is the only
/// one sized for a reading line rather than a mark. It happens to equal
/// [`HIT_TARGET_MIN`], but not by rule: the chip is **non-interactive**
/// (§3.8), so it owes nothing to the hit-target floor.
pub const BADGE_CHIP_HEIGHT: f32 = 24.0;

// ---------------------------------------------------------------------------
//  Avatar geometry (§3.13, `UIF-ATOM-013`) — the container diameter rungs
// ---------------------------------------------------------------------------

/// `size.avatar.sm` — small avatar diameter: `24 × 24px` (`UIF-ATOM-013`, §3.13).
///
/// Meets the [`HIT_TARGET_MIN`] floor exactly: even the smallest avatar
/// provides a compliant touch target when interactive.
pub const AVATAR_SM: f32 = 24.0;

/// `size.avatar.md` — medium / standard avatar diameter: `32 × 32px` (`UIF-ATOM-013`, §3.13).
pub const AVATAR_MD: f32 = 32.0;

/// `size.avatar.lg` — large avatar diameter: `36 × 36px` (`UIF-ATOM-013`, §3.13).
pub const AVATAR_LG: f32 = 36.0;

#[cfg(test)]
#[path = "size_tests.rs"]
mod size_tests;
