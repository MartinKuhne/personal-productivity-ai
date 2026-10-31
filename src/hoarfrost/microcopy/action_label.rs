//! Action-label rules: verb-first CTAs and the ellipsis `iff`
//! (`UIF-PRIN-007`).
//!
//! Two of pillar 7's clauses live here because both are facts about an
//! *action* label rather than about prose, and both are checkable only if
//! the action carries information the string alone does not contain.
//!
//! # The ellipsis is a biconditional, so it needs a second input
//!
//! §1.1 says the ASCII three-dot suffix is appended **if and only if**
//! activating the control opens a secondary dialog or prompt requiring
//! further input. A string cannot answer that question: `Rename...` and
//! `Save` are both short imperative labels, and whether one of them opens
//! a dialog is a fact about the handler behind it. So
//! [`ActionLabel::behaviour`] carries the answer as data, and
//! [`check_action_ellipsis`] can then test both directions of the `iff`
//! rather than one. All four quadrants are pinned by test.
//!
//! # Two ellipses that must never be confused
//!
//! * `...` (U+002E × 3) — the **action** suffix. Appended *unspaced*:
//!   `Rename...`, never `Rename ...`. `'Create Directory ...'` is the
//!   one pinned exception (`UI-015`).
//! * `…` (U+2026) — the **truncation** mark, reserved for
//!   layout-time elision, placeholders and the `[…]` breadcrumb
//!   trigger. Finding it as an action suffix means someone typed the
//!   wrong character for the wrong job, which is exactly the confusion
//!   the spec splits them to prevent.
//!
//! # Verb-first, narrowly
//!
//! §1.1 forbids generic `OK` and `Yes`/`No` where a specific imperative
//! verb is called for. That set is small on purpose: the spec names
//! those and no others, so [`GENERIC_CTA_LABELS`] holds four strings
//! (`ok`, `okay`, `yes`, `no` — `okay` being the other spelling of the
//! first) and nothing else. Widening it to `Cancel`, `Done` or `Submit`
//! would be inventing a style rule the spec did not write, and `Cancel`
//! in particular *is* a specific imperative verb.
//!
//! The check matches the whole trimmed label rather than its first word,
//! because it is called on CTAs. Running it over arbitrary prose would
//! flag `"no results found"` for a rule that was never about that.
//!
//! Unit tests live in the sibling `action_label_tests.rs` sidecar.

use crate::microcopy::pins::{PinExemption, is_exempt};

/// The ASCII three-dot action suffix (`UIF-PRIN-007`).
///
/// Three U+002E FULL STOP characters — the *action* marker, distinct
/// from [`UNICODE_ELLIPSIS`].
pub const ASCII_ACTION_ELLIPSIS: &str = "...";

/// The Unicode horizontal ellipsis (`UIF-PRIN-007`).
///
/// `U+2026`, reserved exclusively for layout-time truncation, input
/// placeholders and the `[…]` breadcrumb elision trigger. Its presence
/// on an action label is a violation of the reservation.
pub const UNICODE_ELLIPSIS: char = '\u{2026}';

/// What happens when a control is activated (`UIF-PRIN-007`).
///
/// The second input the ellipsis `iff` needs. The label alone cannot
/// say whether a prompt follows, so the host says it here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PromptBehaviour {
    /// Activation runs immediately — `Save`, `Format Markdown`,
    /// `Close`. §1.1's view toggles (`System logs`) are immediate in
    /// exactly the sense this rule cares about, so they fall under this
    /// arm rather than needing a fourth variant that would behave
    /// identically.
    Immediate,
    /// Activation opens a secondary dialog or prompt requiring further
    /// user input before anything executes — `Rename...`, `Move...`,
    /// `Delete...`.
    LaunchesPrompt,
}

impl PromptBehaviour {
    /// Whether the action suffix is required on this behaviour's label.
    #[must_use]
    pub const fn requires_action_ellipsis(self) -> bool {
        matches!(self, Self::LaunchesPrompt)
    }

    /// Stable machine-readable name for reports.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Immediate => "immediate",
            Self::LaunchesPrompt => "launches-prompt",
        }
    }
}

/// An action label plus what the action actually does
/// (`UIF-PRIN-007`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionLabel<'a> {
    /// The label as it will be shown.
    pub label: &'a str,
    /// Whether activating it opens a further prompt.
    pub behaviour: PromptBehaviour,
}

/// One way an action label breaks the ellipsis rules
/// (`UIF-PRIN-007`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EllipsisViolation<'a> {
    /// The action opens a prompt but its label carries no `...`.
    MissingActionEllipsis {
        /// The offending label.
        label: &'a str,
    },
    /// The action runs immediately but its label carries `...`,
    /// promising a prompt that will never appear.
    SpuriousActionEllipsis {
        /// The offending label.
        label: &'a str,
    },
    /// The `...` is there but preceded by a space; §1.1 requires it
    /// appended unspaced.
    SpacedActionEllipsis {
        /// The offending label.
        label: &'a str,
    },
    /// The label ends in `…` (U+2026), which is reserved for
    /// truncation, placeholders and `[…]` elision — never an action
    /// suffix.
    UnicodeEllipsisAsActionSuffix {
        /// The offending label.
        label: &'a str,
    },
}

impl std::fmt::Display for EllipsisViolation<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingActionEllipsis { label } => write!(
                f,
                "'{label}' opens a secondary prompt but carries no '{ASCII_ACTION_ELLIPSIS}' suffix"
            ),
            Self::SpuriousActionEllipsis { label } => write!(
                f,
                "'{label}' runs immediately but carries '{ASCII_ACTION_ELLIPSIS}', promising a prompt that never appears"
            ),
            Self::SpacedActionEllipsis { label } => write!(
                f,
                "'{label}' carries a spaced '{ASCII_ACTION_ELLIPSIS}'; §1.1 requires the suffix appended unspaced"
            ),
            Self::UnicodeEllipsisAsActionSuffix { label } => {
                let code_point = UNICODE_ELLIPSIS as u32;
                write!(
                    f,
                    "'{label}' ends in the truncation ellipsis (U+{code_point:04X}); that character is reserved for truncation, placeholders and […] elision"
                )
            }
        }
    }
}

/// The ellipsis verdict for one action label (`UIF-PRIN-007`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EllipsisVerdict<'a> {
    /// The label that was checked.
    pub label: &'a str,
    /// The behaviour it was checked against.
    pub behaviour: PromptBehaviour,
    /// What broke, if anything.
    pub violation: Option<EllipsisViolation<'a>>,
    /// Whether a host pin exempted the label.
    pub exempt: bool,
}

impl EllipsisVerdict<'_> {
    /// Whether the label may ship.
    #[must_use]
    pub fn is_compliant(&self) -> bool {
        self.violation.is_none() || self.exempt
    }
}

/// Whether `label` ends with the ASCII three-dot suffix, spacing aside.
#[must_use]
pub fn ends_with_ascii_ellipsis(label: &str) -> bool {
    label.ends_with(ASCII_ACTION_ELLIPSIS)
}

/// Whether `label` ends with the Unicode truncation ellipsis.
#[must_use]
pub fn ends_with_unicode_ellipsis(label: &str) -> bool {
    label.ends_with(UNICODE_ELLIPSIS)
}

/// Whether the trailing ASCII suffix is preceded by whitespace.
///
/// `false` when there is no suffix at all, or when the label is nothing
/// but the suffix.
#[must_use]
pub fn is_spaced_ellipsis(label: &str) -> bool {
    if !ends_with_ascii_ellipsis(label) {
        return false;
    }
    let before = &label[..label.len() - ASCII_ACTION_ELLIPSIS.len()];
    before.chars().next_back().is_some_and(char::is_whitespace)
}

/// Check one action label against the ellipsis `iff` (`UIF-PRIN-007`).
///
/// The Unicode reservation is tested first: a `…` on an action label is
/// the wrong *character*, and reporting it as a merely-missing or
/// merely-spurious ASCII suffix would send the reader to fix the wrong
/// thing.
#[must_use]
pub fn check_action_ellipsis<'a>(action: &ActionLabel<'a>) -> EllipsisVerdict<'a> {
    let violation = if ends_with_unicode_ellipsis(action.label) {
        Some(EllipsisViolation::UnicodeEllipsisAsActionSuffix {
            label: action.label,
        })
    } else if action.behaviour.requires_action_ellipsis() {
        if !ends_with_ascii_ellipsis(action.label) {
            Some(EllipsisViolation::MissingActionEllipsis {
                label: action.label,
            })
        } else if is_spaced_ellipsis(action.label) {
            Some(EllipsisViolation::SpacedActionEllipsis {
                label: action.label,
            })
        } else {
            None
        }
    } else if ends_with_ascii_ellipsis(action.label) {
        Some(EllipsisViolation::SpuriousActionEllipsis {
            label: action.label,
        })
    } else {
        None
    };

    let exempt = violation.is_some() && is_exempt(action.label, PinExemption::Ellipsis);
    EllipsisVerdict {
        label: action.label,
        behaviour: action.behaviour,
        violation,
        exempt,
    }
}

/// The generic CTA labels §1.1 forbids (`UIF-PRIN-007`).
///
/// Stored lowercase; comparison lowercases the candidate. The spec names
/// `OK` and `Yes`/`No` and nothing else, so this list is four strings
/// and no more — `okay` is included only because it is the same word
/// spelled another way.
pub const GENERIC_CTA_LABELS: [&str; 4] = ["ok", "okay", "yes", "no"];

/// Whether `label` is a generic CTA rather than a specific imperative.
///
/// Matches the whole trimmed label, case-insensitively. Called on CTAs;
/// running it over arbitrary prose would flag sentences the rule was
/// never about.
#[must_use]
pub fn is_generic_cta(label: &str) -> bool {
    let folded = label.trim().to_lowercase();
    GENERIC_CTA_LABELS.contains(&folded.as_str())
}

/// The verb-first verdict for one CTA label (`UIF-PRIN-007`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerbFirstVerdict<'a> {
    /// The label that was checked.
    pub label: &'a str,
    /// Whether it is generic.
    pub generic: bool,
    /// Whether a host pin exempts it.
    pub exempt: bool,
}

impl VerbFirstVerdict<'_> {
    /// Whether the CTA may ship.
    #[must_use]
    pub fn is_compliant(&self) -> bool {
        !self.generic || self.exempt
    }
}

/// Check a dialog or form CTA against the verb-first mandate
/// (`UIF-PRIN-007`).
///
/// `'Ok'` is generic and exempt by pin (`UI-009`); `'OK'` is generic
/// and **not** pinned, so it fails. That asymmetry is the pin table
/// doing exactly the job it exists for.
#[must_use]
pub fn check_verb_first(label: &str) -> VerbFirstVerdict<'_> {
    let generic = is_generic_cta(label);
    let exempt = generic && is_exempt(label, PinExemption::VerbFirst);
    VerbFirstVerdict {
        label,
        generic,
        exempt,
    }
}

/// Check an action label against both action-label clauses at once
/// (`UIF-PRIN-007`).
///
/// Convenience over [`check_action_ellipsis`] and [`check_verb_first`]:
/// a host validating a whole dialog calls this once per CTA and gets
/// both verdicts.
#[must_use]
pub fn check_action<'a>(action: &ActionLabel<'a>) -> (EllipsisVerdict<'a>, VerbFirstVerdict<'a>) {
    (
        check_action_ellipsis(action),
        check_verb_first(action.label),
    )
}

#[cfg(test)]
#[path = "action_label_tests.rs"]
mod action_label_tests;
