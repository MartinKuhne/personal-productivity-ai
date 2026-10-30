//! The "Reward Early, Punish Late" validation choreography
//! (`UIF-PRIN-009`).
//!
//! §1.1's ninth pillar is three clauses about *timing*, and timing is
//! where validation usually goes wrong. Deciding **whether** a value is
//! valid is a host concern and this crate already lets a host say so.
//! Deciding **when the user is told** is a framework concern, and until
//! now the framework had no say in it at all.
//!
//! * [`stance`] — [`EditStance`], whether the user has committed to the
//!   field yet. The fact the whole pillar turns on, and the one no model
//!   in this crate used to carry.
//! * [`disallowed`] — [`DisallowedClass`], the characters a field can
//!   never accept, kept separate from "this value is invalid" because the
//!   pillar treats the two as opposites in time.
//! * [`punish`](mod@crate::validation::punish) — [`punish()`](fn@crate::validation::punish::punish),
//!   the single pure function that turns
//!   `(stance, invalid, disallowed)` into what gets painted and what the
//!   app must still know.
//! * [`recovery`] — [`EditSnapshot`] and [`RecoverySession`], the
//!   buffer-plus-selection-plus-scroll revert target a rejected operation
//!   owes the user.
//!
//! # Why a subsystem rather than a field on each widget
//!
//! The alternative was a `pristine: bool` on every validation-bearing
//! model, and a `show_error: bool` next to it. That is state that can
//! only ever be wrong: two copies of "has the user finished?" on one
//! model, written from different places, with nothing forcing them to
//! agree — and a renderer free to read whichever one it happened to get
//! to first.
//!
//! A stance plus a pure decision cannot drift, because the decision is
//! recomputed from the stance every frame and there is exactly one place
//! that computes it. The renderer's whole question becomes
//! `punish(model.stance(), model.is_invalid(), disallowed).painted()`,
//! and the answer cannot be arrived at incorrectly by asking somewhere
//! else.
//!
//! # What this does *not* do
//!
//! It does not validate anything. It does not run on a clock — there is
//! no `Instant` anywhere in this module, and none is needed, because the
//! decision is a function of state the caller already has rather than of
//! elapsed time. And it does not force a widget to consult it: a renderer
//! that draws a red border from `is_invalid()` directly still compiles,
//! which is the residual the ledger names.
//!
//! Unit tests live in the sibling `*_tests.rs` sidecars of each file.

pub mod disallowed;
pub mod punish;
pub mod recovery;
pub mod stance;

pub use disallowed::DisallowedChar;
pub use disallowed::DisallowedClass;
pub use disallowed::DisallowedKind;
pub use disallowed::FILENAME_ILLEGAL_CHARS;
pub use punish::PunishReason;
pub use punish::Punishment;
pub use punish::punish;
pub use recovery::EditSnapshot;
pub use recovery::RecoverySession;
pub use recovery::RestoredFields;
pub use recovery::ScrollOffset;
pub use stance::EditStance;
