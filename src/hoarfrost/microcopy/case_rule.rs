//! The sentence-case mandate and its acronym licence (`UIF-PRIN-007`).
//!
//! §1.1: *"All UI labels … use **Sentence case** (capitalizing only the
//! first word and proper nouns/acronyms such as `Markdown`, `PDF`,
//! `MCP`, `JSON`). ALL-CAPS and Title Case on multi-word chrome labels
//! are prohibited."*
//!
//! # A checker that cries wolf is worse than no checker
//!
//! Capitalisation lint fails one way: it flags a legitimate label, the
//! host adds an override, and the next person learns to distrust the
//! rule rather than read it. Three decisions here exist to keep the false
//! positive rate near zero.
//!
//! **Acronyms are data, not a heuristic.** There is no "all-caps word of
//! length ≤ 4 is probably an acronym" rule. A word is licensed only if it
//! appears in the licensed set, and the set is a named constant a host
//! can read and extend through [`classify_with`]. §1.1 names four
//! (`Markdown`, `PDF`, `MCP`, `JSON`); the rest of the framework's
//! default set is framework-supplied and recorded as such in the
//! coverage ledger.
//!
//! **A label that does not start with a letter is not first-word-lowercase.**
//! `"3 items selected"` begins with a digit, so its first *cased* word
//! is `"items"` — and capitalising that to `"3 Items selected"` would be
//! strictly worse copy than what the rule is trying to prevent. The
//! check only fires when the label's first character is an alphabetic
//! lowercase.
//!
//! **Pinned literals are exempt before they are reported.** The Host
//! `SPEC.md` Literal Pin Table is consulted through
//! [`pins::is_exempt`](crate::microcopy::pins::is_exempt) so `'Copy Path'` passes while `'Copy path'` —
//! the unpinned tree-target variant the spec explicitly puts *under*
//! `UIF-PRIN-007` — does not.
//!
//! # What the check cannot see
//!
//! It reads a string, not a rendered label. It cannot tell a proper noun
//! it has never met from a typo, which is exactly why the licensed set is
//! a list rather than a model. A host that renames a feature to something
//! capitalised has to add the word; nothing here can infer it.
//!
//! Unit tests live in the sibling `case_rule_tests.rs` sidecar.

use crate::microcopy::pins::{PinExemption, is_exempt, pin_for};

/// The proper nouns and acronyms §1.1 names in the mandate itself.
///
/// Transcribed from the sentence *"such as `Markdown`, `PDF`, `MCP`,
/// `JSON`"*. Everything else in [`DEFAULT_LICENSED`] is a framework
/// addition, not a spec transcription.
pub const SPEC_NAMED_LICENSED: [&str; 4] = ["Markdown", "PDF", "MCP", "JSON"];

/// The framework's default licensed set.
///
/// The four spec-named words plus the acronyms and proper nouns this
/// product's chrome actually contains. This is a **framework-supplied
/// default**, not a spec list — the spec gives four examples and stops.
/// A host whose labels use another proper noun must pass it through
/// [`classify_with`] rather than expect the checker to guess.
pub const DEFAULT_LICENSED: &[&str] = &[
    "AI", "API", "CSV", "DAV", "FastMD", "Git", "HTML", "ID", "JMAP", "JSON", "MCP", "Markdown",
    "OAuth", "PDF", "TOML", "UI", "URI", "URL", "Windows", "XML", "YAML",
];

/// How one whitespace-delimited word is capitalised
/// (`UIF-PRIN-007`).
///
/// Only alphabetic characters are considered, so trailing punctuation —
/// the `...` of `Rename...`, the `?` of a question — does not change a
/// word's classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WordCase {
    /// No alphabetic characters at all: `"3"`, `"..."`, `"—"`.
    Uncased,
    /// First letter lowercase, all the rest lowercase: `"save"`.
    Lower,
    /// First letter uppercase, all the rest lowercase: `"Changes"`.
    Title,
    /// Every letter uppercase: `"PDF"`.
    Upper,
    /// Anything else — `"FastMD"`, `"OAuth"`, `"myLabel"`.
    Mixed,
}

impl WordCase {
    /// Classify one whitespace-delimited word.
    #[must_use]
    pub fn of(word: &str) -> Self {
        let letters: Vec<char> = word.chars().filter(|c| c.is_alphabetic()).collect();
        let Some((first, rest)) = letters.split_first() else {
            return Self::Uncased;
        };
        let first_upper = first.is_uppercase();
        // A single capital letter is a Title-cased word, not an all-caps
        // shout. Without the length guard `check_label("A")` and every
        // label containing the pronoun "I" would come back as ALL-CAPS —
        // exactly the false positive that would make the rule ignorable.
        let all_upper = letters.len() > 1 && letters.iter().all(|c| c.is_uppercase());
        let rest_lower = rest.iter().all(|c| c.is_lowercase());

        if all_upper {
            Self::Upper
        } else if first_upper && rest_lower {
            Self::Title
        } else if !first_upper && rest_lower {
            Self::Lower
        } else {
            Self::Mixed
        }
    }
}

/// How a whole label is capitalised (`UIF-PRIN-007`).
///
/// The classification *is* the verdict: [`Capitalisation::Sentence`] and
/// [`Capitalisation::Uncasable`] are the two compliant shapes and
/// everything else is a named violation. Collapsing classification and
/// verdict into one type is deliberate — a report that said "this is
/// Title Case, and by the way that's a violation" gives the reader two
/// chances to disagree with itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Capitalisation {
    /// No cased characters — nothing to check.
    Uncasable,
    /// Sentence case — compliant.
    Sentence,
    /// Every cased word fully uppercase, at least one of them unlicensed.
    AllCaps,
    /// A capital appears inside a word rather than at its head.
    MidWordCapital,
    /// The label begins with a lowercase letter.
    FirstWordLowercase,
    /// More than one unlicensed word begins with a capital.
    TitleCase,
}

impl Capitalisation {
    /// Whether this classification satisfies the mandate.
    #[must_use]
    pub const fn is_compliant(self) -> bool {
        matches!(self, Self::Uncasable | Self::Sentence)
    }

    /// Stable machine-readable name for reports.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Uncasable => "uncasable",
            Self::Sentence => "sentence",
            Self::AllCaps => "all-caps",
            Self::MidWordCapital => "mid-word-capital",
            Self::FirstWordLowercase => "first-word-lowercase",
            Self::TitleCase => "title-case",
        }
    }
}

/// The verdict for one label (`UIF-PRIN-007`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaseReport<'a> {
    /// The label that was checked.
    pub label: &'a str,
    /// How it classified.
    pub capitalisation: Capitalisation,
    /// Whether a host pin exempted it rather than the label being clean.
    pub exempt: bool,
    /// What pins the literal, when it is pinned.
    pub pinned_by: Option<&'static str>,
}

impl CaseReport<'_> {
    /// Whether the label may ship.
    ///
    /// True for a compliant classification, and also for a violating
    /// classification the Host Pin Table exempts — the pin is the whole
    /// reason a Title Case row like `'Close Others'` is legal.
    #[must_use]
    pub fn is_compliant(&self) -> bool {
        self.capitalisation.is_compliant() || self.exempt
    }
}

impl std::fmt::Display for CaseReport<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.capitalisation.is_compliant() {
            write!(f, "'{}' is {}", self.label, self.capitalisation.slug())
        } else if self.exempt {
            write!(
                f,
                "'{}' is {} but pinned by {}",
                self.label,
                self.capitalisation.slug(),
                self.pinned_by.unwrap_or("the host pin table"),
            )
        } else {
            write!(
                f,
                "'{}' is {}; §1.1 requires sentence case",
                self.label, self.capitalisation
            )
        }
    }
}

impl std::fmt::Display for Capitalisation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.slug())
    }
}

/// Whether `word` is licensed to break sentence case.
fn is_licensed(word: &str, licensed: &[&str]) -> bool {
    licensed.contains(&word)
}

/// How many alphabetic characters a word carries.
fn letter_count(word: &str) -> usize {
    word.chars().filter(|c| c.is_alphabetic()).count()
}

/// Classify `label` against a caller-supplied licensed set
/// (`UIF-PRIN-007`).
///
/// The general form. Evaluation order is deliberate and each step is
/// more specific than the last:
///
/// 1. no cased words → [`Capitalisation::Uncasable`]
/// 2. every cased word fully uppercase (two letters or more), at least
///    one unlicensed → [`Capitalisation::AllCaps`]
/// 3. any unlicensed word with a capital inside it →
///    [`Capitalisation::MidWordCapital`]
/// 4. the label's first character is a lowercase letter →
///    [`Capitalisation::FirstWordLowercase`]
/// 5. any unlicensed multi-letter word past the first begins with a
///    capital → [`Capitalisation::TitleCase`]
/// 6. otherwise → [`Capitalisation::Sentence`]
///
/// Steps 2 and 5 both ignore single-letter words. A one-letter run
/// cannot be a shout and cannot be a Title-Cased choice, so counting `"A"`
/// or `"I"` would report correct copy as a violation.
#[must_use]
pub fn classify_with(label: &str, licensed: &[&str]) -> Capitalisation {
    let words: Vec<&str> = label.split_whitespace().collect();
    let cased: Vec<(&str, WordCase)> = words
        .iter()
        .map(|word| (*word, WordCase::of(word)))
        .filter(|(_, case)| *case != WordCase::Uncased)
        .collect();

    if cased.is_empty() {
        return Capitalisation::Uncasable;
    }

    if cased.iter().all(|(_, case)| *case == WordCase::Upper)
        && cased.iter().any(|(word, _)| !is_licensed(word, licensed))
    {
        return Capitalisation::AllCaps;
    }

    if cased
        .iter()
        .any(|(word, case)| *case == WordCase::Mixed && !is_licensed(word, licensed))
    {
        return Capitalisation::MidWordCapital;
    }

    if label
        .chars()
        .next()
        .is_some_and(|first| first.is_alphabetic() && first.is_lowercase())
    {
        return Capitalisation::FirstWordLowercase;
    }

    let unlicensed_capitals = cased
        .iter()
        .skip(1)
        .filter(|(word, case)| {
            matches!(case, WordCase::Title | WordCase::Upper)
                && !is_licensed(word, licensed)
                // A one-letter word is not "Title Cased" in any sense the
                // prohibition reaches. Without this the pronoun "I" and
                // the article "A" each read as an offending capital, so
                // "Add to I list" would be reported as Title Case.
                && letter_count(word) > 1
        })
        .count();

    if unlicensed_capitals > 0 {
        return Capitalisation::TitleCase;
    }

    Capitalisation::Sentence
}

/// Classify `label` against [`DEFAULT_LICENSED`] (`UIF-PRIN-007`).
#[must_use]
pub fn classify(label: &str) -> Capitalisation {
    classify_with(label, DEFAULT_LICENSED)
}

/// Check one chrome label against the sentence-case mandate
/// (`UIF-PRIN-007`).
///
/// Classifies the label, then consults the Host `SPEC.md` Literal Pin
/// Table before calling anything a violation. A pinned literal is
/// reported as exempt rather than compliant so a reader can see *why*
/// `'Close Others'` passed — the two are not the same fact.
#[must_use]
pub fn check_label(label: &str) -> CaseReport<'_> {
    let capitalisation = classify(label);
    let pin = pin_for(label);
    let exempt = !capitalisation.is_compliant() && is_exempt(label, PinExemption::SentenceCase);
    CaseReport {
        label,
        capitalisation,
        exempt,
        pinned_by: pin.map(|pinned| pinned.pinned_by),
    }
}

/// Check a label against a caller-supplied licensed set (`UIF-PRIN-007`).
///
/// The extension point for a host whose chrome contains proper nouns the
/// framework has never met.
#[must_use]
pub fn check_label_with<'a>(label: &'a str, licensed: &[&str]) -> CaseReport<'a> {
    let capitalisation = classify_with(label, licensed);
    let pin = pin_for(label);
    let exempt = !capitalisation.is_compliant() && is_exempt(label, PinExemption::SentenceCase);
    CaseReport {
        label,
        capitalisation,
        exempt,
        pinned_by: pin.map(|pinned| pinned.pinned_by),
    }
}

#[cfg(test)]
#[path = "case_rule_tests.rs"]
mod case_rule_tests;
