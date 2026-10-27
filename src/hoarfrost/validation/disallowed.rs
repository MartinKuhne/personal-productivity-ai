//! The characters a field refuses outright, as opposed to a value that
//! merely fails validation (`UIF-PRIN-009`, `UIF-INP-003`).
//!
//! §1.1 treats these two facts as opposites in time:
//!
//! * *"this value is invalid"* is **withheld** while the field is pristine
//!   — a duplicate name, a missing extension, a length the host dislikes.
//!   The user has not finished, and telling them now is premature.
//! * *"this character is not allowed here"* is **punished instantly**,
//!   pristine or not. A `/` typed into a basename is not a half-formed
//!   intention; it is a character that will never be legal at this
//!   position, and waiting for `blur` to say so means the user finishes
//!   typing the whole path before learning the field was never going to
//!   take it.
//!
//! A plain `invalid: bool` cannot express that difference, which is why
//! the carve-out needs its own type rather than a second flag on the
//! model.
//!
//! # What the spec actually licenses
//!
//! §1.1 names two characters — `/` and `\0` — and points at
//! `UIF-INP-003`, which asks for "valid filesystem basename characters"
//! without enumerating them. This crate already enumerates a set for
//! exactly that requirement, in `surface/input_dialog.rs`'s basename
//! rule: the eight Windows-illegal punctuation characters, plus control
//! characters. That set is reused verbatim rather than re-derived, so the
//! framework's carve-out and the dialog's validator cannot disagree.
//!
//! Windows' *reserved device names* (`CON`, `PRN`, `AUX`, `NUL`,
//! `COM1`–`COM9`, `LPT1`–`LPT9`) are deliberately **not** modelled
//! here. The spec's carve-out is about characters, and a reserved name is
//! a whole-string property: it is legal one character at a time and only
//! becomes illegal at the end, which puts it on the withheld side of the
//! line, not the instant-punish side. Recording that judgement is the
//! job of the ledger's `note:`, and it is recorded there.
//!
//! Unit tests live in the sibling `disallowed_tests.rs` sidecar.

use std::fmt;

/// The punctuation a filesystem basename cannot contain.
///
/// The eight characters Windows refuses in a name. This is the
/// authoritative copy: `surface/input_dialog.rs`'s basename validator
/// reads this constant rather than carrying its own literal, so the
/// instant-punish rule and the submit-time rule are the same rule.
pub const FILENAME_ILLEGAL_CHARS: &str = "/\\:*?\"<>|";

/// A closed class of characters a field refuses in real time.
///
/// Closed rather than open-ended on purpose. §1.1 licenses exactly one
/// carve-out — the filesystem basename — and a class list that could be
/// extended without touching the spec would let an invented "no spaces at
/// all" or "no uppercase" rule smuggle itself into the instant-punish
/// path, where the pillar means only the genuinely-never-legal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DisallowedClass {
    /// A filesystem basename, as `UIF-INP-003` requires.
    FilenameBasename,
}

/// Which rule a refused character broke.
///
/// Kept distinct from the character itself so a caller can say *why* the
/// field went red without re-testing membership. The two kinds also
/// differ in how they arrive: an illegal path character is typed, a
/// control character is often pasted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DisallowedKind {
    /// One of [`FILENAME_ILLEGAL_CHARS`].
    IllegalPathCharacter,
    /// A Unicode control character — `Cc`, NUL included.
    ControlCharacter,
}

impl DisallowedKind {
    /// Every kind, in the order the classifier tests them.
    pub const ALL: [Self; 2] = [Self::IllegalPathCharacter, Self::ControlCharacter];
}

/// The first refused character in a value.
///
/// Carries the byte offset as well as the character because the caller
/// usually needs to point at it — in a caption, a caret position, or a
/// test failure message. `char` rather than `&str` so the report is `Copy`
/// and cannot outlive the value it came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DisallowedChar {
    /// The character that is not allowed.
    pub character: char,
    /// Its byte offset into the value that was tested.
    pub offset: usize,
    /// Which rule it broke.
    pub kind: DisallowedKind,
}

impl DisallowedClass {
    /// Every class this framework models.
    pub const ALL: [Self; 1] = [Self::FilenameBasename];

    /// Classify one character, or say the class allows it.
    #[must_use]
    pub const fn classify(self, character: char) -> Option<DisallowedKind> {
        match self {
            Self::FilenameBasename => {
                if ascii_contains(FILENAME_ILLEGAL_CHARS, character) {
                    Some(DisallowedKind::IllegalPathCharacter)
                } else if is_control_char(character) {
                    Some(DisallowedKind::ControlCharacter)
                } else {
                    None
                }
            }
        }
    }

    /// Whether this class refuses `character` outright.
    #[must_use]
    pub const fn refuses(self, character: char) -> bool {
        self.classify(character).is_some()
    }

    /// The first refused character in `value`, scanning left to right.
    ///
    /// First rather than all, because the pillar's decision needs only to
    /// know *that* something is present, and reporting the leftmost one
    /// is the one the user's eyes are nearest. A caller wanting every
    /// offence can use [`offences`](Self::offences) instead.
    #[must_use]
    pub fn first_offence(self, value: &str) -> Option<DisallowedChar> {
        self.value_offences(value).next()
    }

    /// Every refused character in `value`, left to right.
    pub fn offences(self, value: &str) -> impl Iterator<Item = DisallowedChar> + '_ {
        self.value_offences(value)
    }

    /// Whether `value` contains anything this class refuses.
    #[must_use]
    pub fn contains_any(self, value: &str) -> bool {
        self.value_offences(value).next().is_some()
    }

    /// The internal iterator, spelled out so `first_offence` and
    /// `offences` share one walk rather than two that could diverge.
    fn value_offences(self, value: &str) -> impl Iterator<Item = DisallowedChar> + '_ {
        value.char_indices().filter_map(move |(offset, character)| {
            self.classify(character).map(|kind| DisallowedChar {
                character,
                offset,
                kind,
            })
        })
    }
}

impl fmt::Display for DisallowedClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FilenameBasename => f.write_str("FilenameBasename"),
        }
    }
}

impl fmt::Display for DisallowedKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IllegalPathCharacter => f.write_str("IllegalPathCharacter"),
            Self::ControlCharacter => f.write_str("ControlCharacter"),
        }
    }
}

/// Byte-level membership test for an ASCII-only needle set.
///
/// `str::contains` is not `const`, and [`DisallowedClass::classify`] is
/// worth being `const` so a caller can build a static refusal table.
/// Every character in [`FILENAME_ILLEGAL_CHARS`] is ASCII, so a
/// non-ASCII `needle` can never match and returns `false` without
/// scanning — which is the correct answer, not a silent failure.
const fn ascii_contains(haystack: &str, needle: char) -> bool {
    if !needle.is_ascii() {
        return false;
    }
    let bytes = haystack.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == needle as u8 {
            return true;
        }
        index += 1;
    }
    false
}

/// The Unicode `Cc` category, computed without a table lookup.
///
/// `Cc` is exactly `U+0000..=U+001F` and `U+007F..=U+009F`, so the
/// comparison is complete rather than an approximation. The sidecar
/// asserts agreement with `char::is_control` across the whole tested
/// range, which is what makes "this is the same rule
/// `check_basename` already applies" a fact rather than a claim.
const fn is_control_char(character: char) -> bool {
    let code = character as u32;
    code <= 0x1F || (code >= 0x7F && code <= 0x9F)
}

#[cfg(test)]
#[path = "disallowed_tests.rs"]
mod disallowed_tests;
