//! The closed Host `SPEC.md` Literal Pin Table (`UIF-PRIN-007`).
//!
//! §1.1 pillar 7 mandates sentence case and verb-first CTAs, and then
//! carves out a **closed** set of literals that the host's `SPEC.md`
//! pins verbatim: `'Ok'`, `'Cancel'`, `'Open in Editor'`,
//! `'Show in File Explorer'`, `'Create Directory ...'`, `'Close Others'`,
//! `'Close All'`, `'Copy Path'` and `'Windows...'`. Those strings are
//! owned by `UI-009`..`UI-034` and `UI-062` under `[RUST-043]`, which
//! means this crate may *read* them but must never reshape them.
//!
//! # Closed means exact-match, not pattern-match
//!
//! The table is a set of exact strings and nothing else. There is no
//! prefix rule, no "looks like a pinned row" heuristic, and no
//! case-insensitive lookup. That narrowness is the whole point: the
//! spec's own §4.9 block says `'Copy Path'` is pinned while
//! `'Copy path'` on tree targets is **unpinned and follows
//! `UIF-PRIN-007`**. A checker that fuzzy-matched would silently exempt
//! the unpinned variant and the difference between those two strings
//! would be invisible. The sidecar pins both halves of that pair.
//!
//! # The exemptions are not uniform
//!
//! The two spec blocks that define this table do not exempt every
//! literal from the same rules, and pretending otherwise would be a
//! transcription error:
//!
//! * §4.5 exempts `'Ok'` and `'Cancel'` from the **verb-first CTA**
//!   rule.
//! * §4.8 / §4.9 exempt the seven chrome rows from the **sentence-case**
//!   and **ellipsis** rules.
//!
//! So each entry carries its own [`PinExemption`] list rather than the
//! table granting a blanket pardon.
//!
//! Unit tests live in the sibling `pins_tests.rs` sidecar.

/// One `UIF-PRIN-007` rule a pinned literal escapes
/// (`UIF-PRIN-007`).
///
/// Three separate variants rather than one boolean because the spec's
/// two pin blocks exempt different rules, and a host needs to be able to
/// ask "is this string exempt from *this* check" without getting a
/// yes that means something else.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PinExemption {
    /// Exempt from the sentence-case mandate.
    SentenceCase,
    /// Exempt from the verb-first CTA rule.
    VerbFirst,
    /// Exempt from the ASCII / Unicode ellipsis rules.
    Ellipsis,
}

/// The requirement range the spec attributes the pin exception to as a
/// whole.
///
/// Used for the literals the spec does not tie to a single id. Naming
/// the range rather than guessing an id keeps the citation something a
/// reader can go and check.
pub const HOST_PIN_REQUIREMENTS: &str = "UI-009..UI-034, UI-062";

/// The §4.5 Input Dialog pin block, which names `UI-009` explicitly.
pub const PIN_INPUT_DIALOG: &str = "§4.5 pin block (UI-009)";

/// The §4.9 Context Menu pin block.
pub const PIN_CONTEXT_MENU: &str = "§4.9 pin block (UI-009..UI-034)";

/// The `Create Directory ...` pin, named explicitly by §1.1.
pub const PIN_CREATE_DIRECTORY: &str = "UI-015";

/// The §4.8 `Windows...` pin.
pub const PIN_WINDOWS: &str = "§4.8 pin block (UI-009..UI-034, UI-062)";

/// One literal the host pins verbatim (`UIF-PRIN-007`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PinnedLabel {
    /// The literal exactly as pinned. Matched by exact string equality —
    /// see the module docs on why this is not a pattern.
    pub literal: &'static str,
    /// The requirement the spec attributes this pin to.
    pub pinned_by: &'static str,
    /// The rules this literal is exempt from. Not every pinned literal
    /// escapes every rule; the two spec blocks differ.
    pub exempts: &'static [PinExemption],
}

/// The sentence-case plus ellipsis pair the §4.8 / §4.9 rows carry.
const CHROME_EXEMPTIONS: [PinExemption; 2] = [PinExemption::SentenceCase, PinExemption::Ellipsis];

/// The closed Host `SPEC.md` Literal Pin Table (`UIF-PRIN-007`).
///
/// Nine literals, transcribed verbatim from §1.1 line 34. The table is
/// closed: a label not listed here is not pinned, however much it
/// resembles something that is.
pub const HOST_PIN_TABLE: &[PinnedLabel] = &[
    PinnedLabel {
        literal: "Ok",
        pinned_by: PIN_INPUT_DIALOG,
        exempts: &[PinExemption::VerbFirst],
    },
    PinnedLabel {
        literal: "Cancel",
        pinned_by: PIN_INPUT_DIALOG,
        exempts: &[PinExemption::VerbFirst],
    },
    PinnedLabel {
        literal: "Open in Editor",
        pinned_by: PIN_CONTEXT_MENU,
        exempts: &CHROME_EXEMPTIONS,
    },
    PinnedLabel {
        literal: "Show in File Explorer",
        pinned_by: PIN_CONTEXT_MENU,
        exempts: &CHROME_EXEMPTIONS,
    },
    PinnedLabel {
        literal: "Create Directory ...",
        pinned_by: PIN_CREATE_DIRECTORY,
        exempts: &CHROME_EXEMPTIONS,
    },
    PinnedLabel {
        literal: "Close Others",
        pinned_by: PIN_CONTEXT_MENU,
        exempts: &CHROME_EXEMPTIONS,
    },
    PinnedLabel {
        literal: "Close All",
        pinned_by: PIN_CONTEXT_MENU,
        exempts: &CHROME_EXEMPTIONS,
    },
    PinnedLabel {
        literal: "Copy Path",
        pinned_by: PIN_CONTEXT_MENU,
        exempts: &CHROME_EXEMPTIONS,
    },
    PinnedLabel {
        literal: "Windows...",
        pinned_by: PIN_WINDOWS,
        exempts: &CHROME_EXEMPTIONS,
    },
];

/// The pin entry for `label`, if it is one of the nine.
///
/// Exact match, case-sensitive. `'Copy path'` returns `None` even
/// though `'Copy Path'` is pinned.
#[must_use]
pub fn pin_for(label: &str) -> Option<&'static PinnedLabel> {
    HOST_PIN_TABLE.iter().find(|pin| pin.literal == label)
}

/// Whether `label` is a pinned literal.
#[must_use]
pub fn is_pinned(label: &str) -> bool {
    pin_for(label).is_some()
}

/// Whether `label` is exempt from one specific `UIF-PRIN-007` rule.
///
/// A pinned literal is only exempt from the rules its own entry lists,
/// so `'Ok'` is exempt from [`PinExemption::VerbFirst`] but not from
/// [`PinExemption::Ellipsis`] — it has to earn its way past the ellipsis
/// rule on its own merits, as it does.
#[must_use]
pub fn is_exempt(label: &str, exemption: PinExemption) -> bool {
    pin_for(label).is_some_and(|pin| pin.exempts.contains(&exemption))
}

#[cfg(test)]
#[path = "pins_tests.rs"]
mod pins_tests;
