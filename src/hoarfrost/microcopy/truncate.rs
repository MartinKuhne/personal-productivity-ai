//! Context-aware truncation and the tooltip pairing it mandates
//! (`UIF-PRIN-007`).
//!
//! §1.1 splits truncation by *what the string is*, not by how much of
//! it fits:
//!
//! * **End truncation** (`Title…`) for prose titles, tree nodes, menu
//!   labels, form captions and table cells.
//! * **Middle truncation** (`src/app/…/mod.rs`) is *mandatory* for
//!   filesystem paths, URIs and hierarchical / git identifiers,
//!   because the last segment of a path is the part the user is looking
//!   for and end-truncating it hides exactly what identified the thing.
//!
//! # Budget is measured in characters, not pixels
//!
//! This crate has no font loaded where these rules run, so it cannot
//! measure a rendered string. The budget is a count of Unicode scalar
//! values. That is a real boundary rather than a shortcut: a host that
//! knows the pixel width has to convert, and the conversion is its
//! font's business, not the framework's. What *is* guaranteed here is
//! character-level correctness — the output never exceeds the budget,
//! and never splits a multi-byte character mid-sequence, because the
//! walk is over `char`s rather than bytes.
//!
//! # The `…` is U+2026, always
//!
//! Truncation emits [`ELLIPSIS`] (U+2026), never three full stops.
//! §1.1 reserves the Unicode ellipsis for truncation, placeholders and
//! the `[…]` breadcrumb trigger, and reserves `...` for the action
//! suffix — the two must not be interchangeable, or the distinction
//! [`crate::microcopy::action_label`] enforces dissolves.
//!
//! # Degradation is reported, not hidden
//!
//! Middle truncation is supposed to keep *both* the root context and the
//! basename. When the budget cannot hold both, something has to give,
//! and [`Truncated::head_kept`] / [`Truncated::tail_preserved`] say
//! which. A caller that promised the user the full basename can then see
//! that it was not delivered instead of finding out from a bug report.
//!
//! Unit tests live in the sibling `truncate_tests.rs` sidecar.

use crate::microcopy::action_label::UNICODE_ELLIPSIS;

/// The truncation mark: `…` (`U+2026`) (`UIF-PRIN-007`).
///
/// The same code point as [`UNICODE_ELLIPSIS`], re-exported under the
/// name this module's callers think in. One
/// definition, two readable names — not two constants that can drift.
pub const ELLIPSIS: char = UNICODE_ELLIPSIS;

/// The characters that separate segments in a truncatable identifier
/// (`UIF-PRIN-007`).
///
/// Forward and back slash cover POSIX paths, Windows paths and URI paths.
/// Git refs (`feature/x:y`) reach their basename through the `/` in the
/// path portion; a bare `HEAD` has no separator and degrades to the
/// even-split rule, which is the honest answer for a string with no
/// structure to preserve.
pub const SEGMENT_SEPARATORS: [char; 2] = ['/', '\\'];

/// Which truncation shape applies (`UIF-PRIN-007`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TruncationMode {
    /// End truncation — prose titles, tree nodes, menu labels, form
    /// captions, table cells.
    End,
    /// Middle truncation — mandatory for filesystem paths, URIs and
    /// hierarchical / git identifiers.
    Middle,
}

impl TruncationMode {
    /// Stable machine-readable name for reports.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::End => "end",
            Self::Middle => "middle",
        }
    }
}

/// The result of one truncation (`UIF-PRIN-007`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Truncated {
    /// The text to render.
    pub text: String,
    /// The mode that produced it.
    pub mode: TruncationMode,
    /// Whether anything was actually removed.
    pub was_truncated: bool,
    /// Whether the beginning of the string survived.
    ///
    /// `false` only when the budget was too small to keep any head at
    /// all, which means the root context was lost.
    pub head_kept: bool,
    /// Whether the tail — the basename or extension — survived whole.
    ///
    /// The field a path-truncating caller actually cares about: a
    /// middle-truncated path that dropped the extension failed at the
    /// job middle truncation exists to do.
    pub tail_preserved: bool,
}

impl Truncated {
    /// An untouched string.
    fn untouched(text: &str, mode: TruncationMode) -> Self {
        Self {
            text: text.to_owned(),
            mode,
            was_truncated: false,
            head_kept: true,
            tail_preserved: true,
        }
    }

    /// A string reduced to nothing because the budget could not hold the
    /// mark plus any content.
    fn emptied(mode: TruncationMode) -> Self {
        Self {
            text: String::new(),
            mode,
            was_truncated: true,
            head_kept: false,
            tail_preserved: false,
        }
    }
}

/// Truncate so the result is at most `budget` characters, keeping the
/// head (`UIF-PRIN-007`).
#[must_use]
pub fn truncate_end(text: &str, budget: usize) -> Truncated {
    let total = text.chars().count();
    if total <= budget {
        return Truncated::untouched(text, TruncationMode::End);
    }
    if budget == 0 {
        return Truncated::emptied(TruncationMode::End);
    }

    let keep = budget - 1;
    let head: String = text.chars().take(keep).collect();
    Truncated {
        text: format!("{head}{ELLIPSIS}"),
        mode: TruncationMode::End,
        was_truncated: true,
        head_kept: keep > 0,
        tail_preserved: false,
    }
}

/// Truncate in the middle so both ends survive (`UIF-PRIN-007`).
///
/// When the string has a segment separator, the tail is taken as
/// *separator plus basename* so the result reads the way §1.1 writes
/// it — `src/app/…/mod.rs` — and the tail is allocated before the head,
/// because the basename is the more valuable half under a tight budget.
/// With no separator there is no structure to preserve, so the budget is
/// split evenly rather than silently degrading to end truncation.
#[must_use]
pub fn truncate_middle(text: &str, budget: usize) -> Truncated {
    let chars: Vec<char> = text.chars().collect();
    let total = chars.len();
    if total <= budget {
        return Truncated::untouched(text, TruncationMode::Middle);
    }
    if budget == 0 {
        return Truncated::emptied(TruncationMode::Middle);
    }
    if budget == 1 {
        return Truncated {
            text: ELLIPSIS.to_string(),
            mode: TruncationMode::Middle,
            was_truncated: true,
            head_kept: false,
            tail_preserved: false,
        };
    }

    let remaining = budget - 1;
    let separator = chars.iter().rposition(|c| SEGMENT_SEPARATORS.contains(c));

    let (head_len, tail_len, basename_len) = match separator {
        Some(index) => {
            // Tail is `/basename`, so the mark sits between separators.
            let tail_full = total - index;
            let tail = tail_full.min(remaining);
            (remaining - tail, tail, total - (index + 1))
        }
        None => {
            let head = remaining.div_ceil(2);
            (head, remaining - head, total)
        }
    };

    let head: String = chars.iter().take(head_len).collect();
    let tail: String = chars[total - tail_len..].iter().collect();

    Truncated {
        text: format!("{head}{ELLIPSIS}{tail}"),
        mode: TruncationMode::Middle,
        was_truncated: true,
        head_kept: head_len > 0,
        tail_preserved: tail_len >= basename_len,
    }
}

/// Truncate `text` to `budget` characters in `mode` (`UIF-PRIN-007`).
#[must_use]
pub fn truncate(text: &str, budget: usize, mode: TruncationMode) -> Truncated {
    match mode {
        TruncationMode::End => truncate_end(text, budget),
        TruncationMode::Middle => truncate_middle(text, budget),
    }
}

/// A single-line label that may be truncated, plus the tooltip it
/// carries (`UIF-PRIN-007`).
///
/// The shape [`check_truncation_tooltip`] needs. It mirrors the
/// icon-only precedent in
/// [`Button::tooltip_violation`](crate::widget::button_widget::Button::tooltip_violation):
/// the geometry decides that the string is not fully visible, and the
/// tooltip stops being optional at exactly that moment.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LabelSlot<'a> {
    /// What to call this label in a report.
    pub id: &'a str,
    /// The full, untruncated string.
    pub text: &'a str,
    /// The tooltip the host supplied, if any.
    pub tooltip: Option<&'a str>,
}

/// One way a truncated label breaks the tooltip mandate
/// (`UIF-PRIN-007`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TruncationTooltipViolation<'a> {
    /// The label is truncated and carries no tooltip, so the full string
    /// is unreachable.
    MissingTooltip {
        /// The offending label.
        id: &'a str,
        /// The full string the tooltip should have exposed.
        full: &'a str,
    },
    /// A tooltip exists but does not expose the full untruncated string.
    TooltipNotFullText {
        /// The offending label.
        id: &'a str,
        /// The full string the tooltip was required to expose.
        full: &'a str,
        /// What the tooltip actually says.
        tooltip: &'a str,
    },
}

impl std::fmt::Display for TruncationTooltipViolation<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingTooltip { id, full } => write!(
                f,
                "'{id}' is truncated to fit and carries no tooltip; the full string '{full}' is unreachable"
            ),
            Self::TooltipNotFullText { id, full, tooltip } => write!(
                f,
                "'{id}' is truncated and its tooltip says '{tooltip}' but §1.1 requires the full untruncated string '{full}'"
            ),
        }
    }
}

/// Check that a label truncated at layout time exposes its full string
/// through a tooltip (`UIF-PRIN-007`).
///
/// Returns an empty `Vec` when the label fits, or when it is truncated
/// and carries a tooltip equal to the full string.
///
/// This is a **checkable contract, not a runtime guarantee**. Nothing
/// here registers a tooltip with a widget tree; it says whether the pair
/// the host built satisfies the rule, so the violation surfaces in a
/// test or a lint pass rather than as a user who cannot read a filename.
#[must_use]
pub fn check_truncation_tooltip<'a>(
    slot: &LabelSlot<'a>,
    mode: TruncationMode,
    budget: usize,
) -> Vec<TruncationTooltipViolation<'a>> {
    let result = truncate(slot.text, budget, mode);
    if !result.was_truncated {
        return Vec::new();
    }
    match slot.tooltip {
        None => vec![TruncationTooltipViolation::MissingTooltip {
            id: slot.id,
            full: slot.text,
        }],
        Some(tooltip) if tooltip == slot.text => Vec::new(),
        Some(tooltip) => vec![TruncationTooltipViolation::TooltipNotFullText {
            id: slot.id,
            full: slot.text,
            tooltip,
        }],
    }
}

#[cfg(test)]
#[path = "truncate_tests.rs"]
mod truncate_tests;
