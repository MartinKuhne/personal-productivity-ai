//! Microcopy, capitalisation and truncation discipline
//! (`UIF-PRIN-007`).
//!
//! §1.1 pillar 7 is a style guide, and a style guide written in a
//! document is advice. This subsystem is the same rules as *checks*:
//! pure functions over strings and small declarative models, so a host
//! can run them over its labels before it renders them and a test can
//! fail on `SAVE CHANGES` rather than a user noticing it.
//!
//! # Why a `microcopy/` subsystem
//!
//! None of these rules belongs to a widget. A
//! [`ButtonModel`](crate::widget::button_model::ButtonModel) can hold the string
//! `"SAVE CHANGES"` and be perfectly well-formed as far as its own type
//! is concerned, because capitalisation is a property of the *text*,
//! not of the control holding it. The same is true of the ellipsis
//! `iff` — whether `Rename` opens a prompt is a fact about the handler,
//! not the button. Following the bounded-subsystem rule that a directory
//! should fully contain one cohesive concern, the five clauses of
//! pillar 7 live here together and read their vocabulary from the
//! tokens rather than restating it.
//!
//! # The five clauses
//!
//! * [`pins`] — the closed **Host `SPEC.md` Literal Pin Table**. Nine
//!   literals pinned verbatim by `UI-009`..`UI-034` / `UI-062`,
//!   matched exactly, each carrying its own set of exemptions.
//! * [`case_rule`] — the **sentence-case mandate**, with acronyms
//!   licensed by data rather than guessed by heuristic, and the pin
//!   table consulted before anything is called a violation.
//! * [`action_label`] — **verb-first CTAs** and the **ASCII `...`
//!   versus Unicode `…`** biconditional.
//! * [`truncate`](mod@crate::microcopy::truncate) — **end** versus mandatory **middle** truncation,
//!   and the **truncation ⇒ tooltip** contract.
//! * [`tabular`] — the **tabular numerals** flag, and an honest account
//!   of what the framework cannot reach.
//!
//! # What none of this is
//!
//! A runtime enforcement point. Every check here is a pure function a
//! host calls against a plan or a label it is about to show. Nothing
//! in the crate calls them, so an unruly host stays unruly until it
//! opts in — which is why the coverage ledger records the residual
//! rather than claiming the style guide is enforced.
//!
//! * [`pins`] — the pin table, exact-match, closed.
//! * [`case_rule`] — sentence case, ALL-CAPS and Title Case detection.
//! * [`action_label`] — verb-first and the ellipsis `iff`.
//! * [`truncate`](mod@crate::microcopy::truncate) — end / middle truncation and tooltip pairing.
//! * [`tabular`] — the tabular-numerals flag and its boundary.

pub mod action_label;
pub mod case_rule;
pub mod pins;
pub mod tabular;
pub mod truncate;

pub use action_label::{
    ASCII_ACTION_ELLIPSIS, ActionLabel, EllipsisVerdict, EllipsisViolation, PromptBehaviour,
    UNICODE_ELLIPSIS, VerbFirstVerdict, check_action, check_action_ellipsis, check_verb_first,
    is_generic_cta,
};
pub use case_rule::{
    Capitalisation, CaseReport, DEFAULT_LICENSED, SPEC_NAMED_LICENSED, WordCase, check_label,
    check_label_with, classify, classify_with,
};
pub use pins::{
    HOST_PIN_REQUIREMENTS, HOST_PIN_TABLE, PinExemption, PinnedLabel, is_exempt, is_pinned, pin_for,
};
pub use tabular::{
    NumericReadout, ReadoutKind, TabularNums, TabularViolation, check_numeric_readout,
};
pub use truncate::{
    ELLIPSIS, LabelSlot, SEGMENT_SEPARATORS, Truncated, TruncationMode, TruncationTooltipViolation,
    check_truncation_tooltip, truncate, truncate_end, truncate_middle,
};
