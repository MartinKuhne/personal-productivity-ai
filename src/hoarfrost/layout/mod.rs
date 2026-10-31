//! Container composition discipline — the four rules of `UIF-PRIN-006`.
//!
//! §1.1 pillar 6 does not describe a widget. It describes what a
//! *container* is allowed to contain: one loud control rather than two,
//! one density per horizontal band, a footer whose buttons are in a
//! fixed order with a declared first focus, and glyphs centred by optical
//! weight rather than by box. Every one of those rules is a fact about
//! the *relationship between* controls, which is why none of them could
//! live in an atom — a
//! [`ButtonVariant`](crate::widget::button::ButtonVariant)
//! can say it is `Primary`, but it cannot know that its neighbour is
//! `Primary` too.
//!
//! # Why a `layout/` subsystem rather than `token/`
//!
//! [`ControlDensity`] could have gone in
//! [`crate::token`]: it is a pure pairing of two tokens that already
//! exist. The other three rules could not. [`token`](crate::token) owns
//! *values and pure derivations of them* and says of itself that it
//! "adapts nothing … and resolves no style"; a check that counts
//! high-emphasis fills per container and rejects a footer for focusing
//! the wrong button is a rule engine, not a token scale. Putting the
//! rules there would have widened the token subsystem's contract to
//! reach the thing it explicitly excludes.
//!
//! So the four rules live here, as one bounded concern, and read their
//! numbers from the tokens instead of restating them:
//! [`ControlDensity::height`](density::ControlDensity::height) returns
//! [`size::CONTROL_SM`](crate::token::size::CONTROL_SM) and
//! [`size::CONTROL_MD`](crate::token::size::CONTROL_MD);
//! [`footer::GROUP_GAP`] is [`space::SM`](crate::token::space::SM);
//! the glyph slots in [`optical`](optical::optical_box) are
//! [`size::ICON_MD`](crate::token::size::ICON_MD) and
//! [`size::HIT_TARGET_MIN`](crate::token::size::HIT_TARGET_MIN). No new
//! visual number is defined in this directory, which is why
//! [`token_lint`](crate::token_lint) needs no new scope entry for it.
//! The precedent is [`binding::Breakpoint`](crate::binding::Breakpoint):
//! a rung enum derived from spec rules, living outside `token/`.
//!
//! # The four rules
//!
//! * [`density`] — **Row Density Homogeneity.** A horizontal band is
//!   declared at one [`ControlDensity`] and
//!   every control placed in it must sit at that rung. Mixing 28px with
//!   32px in a `Navbar`, `FormRow`, `OverflowToolbar`, `FilterBar` or
//!   dialog footer is reported, naming the offender and both values.
//! * [`emphasis`] — **Single High-Emphasis Focal Point.** At most one
//!   control per container scope wears a solid high-emphasis fill.
//!   `Primary` and `Danger` together are two, because the rule is about
//!   one focal point rather than one of each kind.
//! * [`footer`] — **Action Row Button Ordering.** `[Utility] …
//!   [Secondary / Cancel] [Primary / Confirm]`, right-aligned at
//!   `space.sm`, with a destructive confirmation's initial keyboard
//!   focus carried as data the check can assert.
//! * [`optical`] — **Baseline & Optical Centering.** A mechanism that
//!   separates a glyph's optical box from its em box. The insets are
//!   host-supplied: this crate has no font and therefore no metrics, and
//!   that boundary is recorded rather than papered over with invented
//!   per-glyph numbers.
//!
//! Every check in this subsystem is pure. Nothing here renders, mutates a
//! widget, or reads a live layout pass; a host calls these functions on
//! the plan it is about to build and gets a report back.

pub mod density;
pub mod emphasis;
pub mod footer;
pub mod optical;

pub use density::{
    ControlDensity, DensityBand, DensityReport, DensityViolation, RowSlot, check_row_density,
};
pub use emphasis::{ContainerScope, Emphasis, EmphasisSlot, FocalPointReport, check_focal_point};
pub use footer::{
    FooterAction, FooterAlignment, FooterKind, FooterPlan, FooterSlot, FooterViolation,
};
pub use optical::{GLYPH_SLOT_MD, GLYPH_SLOT_SM, OpticalInset, OpticalShift, optical_box};
