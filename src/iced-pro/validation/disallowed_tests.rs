//! Tests for the disallowed-character carve-out — the one thing §1.1 lets
//! the framework punish while a field is still pristine
//! (`UIF-PRIN-009`, `UIF-INP-003`).

use super::DisallowedChar;
use super::DisallowedClass;
use super::DisallowedKind;
use super::FILENAME_ILLEGAL_CHARS;
use super::ascii_contains;
use super::is_control_char;

// -- the class itself --------------------------------------------------------

#[test]
fn only_the_basename_class_exists() {
    assert_eq!(DisallowedClass::ALL, [DisallowedClass::FilenameBasename]);
}

#[test]
fn the_class_renders_as_its_own_name() {
    assert_eq!(
        DisallowedClass::FilenameBasename.to_string(),
        "FilenameBasename"
    );
    assert_eq!(
        DisallowedKind::IllegalPathCharacter.to_string(),
        "IllegalPathCharacter"
    );
    assert_eq!(
        DisallowedKind::ControlCharacter.to_string(),
        "ControlCharacter"
    );
}

#[test]
fn the_illegal_set_is_the_eight_windows_basename_refusals() {
    let chars: Vec<char> = FILENAME_ILLEGAL_CHARS.chars().collect();
    assert_eq!(
        chars,
        vec!['/', '\\', ':', '*', '?', '"', '<', '>', '|'],
        "the canonical set drifted; surface/input_dialog.rs reads this exact \
         constant for its own validator"
    );
}

// -- the two characters the spec names by hand -------------------------------

#[test]
fn a_slash_is_refused_as_an_illegal_path_character() {
    let offence = DisallowedClass::FilenameBasename
        .first_offence("notes/2024")
        .expect("a slash in a basename must be caught");
    assert_eq!(offence.character, '/');
    assert_eq!(offence.offset, 5);
    assert_eq!(offence.kind, DisallowedKind::IllegalPathCharacter);
}

#[test]
fn a_nul_is_refused_as_a_control_character() {
    let offence = DisallowedClass::FilenameBasename
        .first_offence("name\0")
        .expect("NUL in a basename must be caught");
    assert_eq!(offence.character, '\0');
    assert_eq!(offence.offset, 4);
    assert_eq!(offence.kind, DisallowedKind::ControlCharacter);
}

// -- membership ------------------------------------------------------------

#[test]
fn every_character_in_the_illegal_set_is_refused() {
    for character in FILENAME_ILLEGAL_CHARS.chars() {
        assert!(
            DisallowedClass::FilenameBasename.refuses(character),
            "{character:?} is in FILENAME_ILLEGAL_CHARS but the class allows it"
        );
        assert_eq!(
            DisallowedClass::FilenameBasename.classify(character),
            Some(DisallowedKind::IllegalPathCharacter)
        );
    }
}

#[test]
fn every_control_character_is_refused() {
    for code in 0u32..0x20 {
        let character = char::from_u32(code).expect("C0 is all valid");
        assert!(
            DisallowedClass::FilenameBasename.refuses(character),
            "U+{code:04X} is a control character and must be refused"
        );
    }
    for code in 0x7Fu32..0xa0 {
        let character = char::from_u32(code).expect("C1/DEL is all valid");
        assert!(
            DisallowedClass::FilenameBasename.refuses(character),
            "U+{code:04X} is a control character and must be refused"
        );
    }
}

#[test]
fn ordinary_basename_characters_are_allowed() {
    for character in "notes-2024_v1.0 final ر ü é".chars() {
        assert!(
            !DisallowedClass::FilenameBasename.refuses(character),
            "{character:?} is a legal basename character the class refused"
        );
    }
}

#[test]
fn the_illegal_path_rule_is_checked_before_the_control_rule() {
    // DEL is a control character and not a path character; a slash is a
    // path character and not a control. Neither can be misclassified, but
    // the ordering is what makes the kinds disjoint rather than
    // overlapping guesses.
    assert_eq!(
        DisallowedClass::FilenameBasename.classify('\u{7F}'),
        Some(DisallowedKind::ControlCharacter)
    );
    assert_eq!(
        DisallowedClass::FilenameBasename.classify('/'),
        Some(DisallowedKind::IllegalPathCharacter)
    );
}

// -- scanning --------------------------------------------------------------

#[test]
fn first_offence_returns_the_leftmost_character() {
    let offence = DisallowedClass::FilenameBasename
        .first_offence("ab:cd/ef")
        .expect("a colon is present");
    assert_eq!(offence.character, ':');
    assert_eq!(offence.offset, 2);
}

#[test]
fn offences_reports_every_refusal_in_order() {
    let found: Vec<(char, usize, DisallowedKind)> = DisallowedClass::FilenameBasename
        .offences("a/b:c\0d")
        .map(|offence: DisallowedChar| (offence.character, offence.offset, offence.kind))
        .collect();
    assert_eq!(
        found,
        vec![
            ('/', 1, DisallowedKind::IllegalPathCharacter),
            (':', 3, DisallowedKind::IllegalPathCharacter),
            ('\0', 5, DisallowedKind::ControlCharacter),
        ]
    );
}

#[test]
fn a_clean_value_has_no_offence() {
    assert_eq!(
        DisallowedClass::FilenameBasename.first_offence("readme.md"),
        None
    );
    assert!(!DisallowedClass::FilenameBasename.contains_any("readme.md"));
}

#[test]
fn an_empty_value_has_no_offence() {
    assert!(!DisallowedClass::FilenameBasename.contains_any(""));
    assert_eq!(
        DisallowedClass::FilenameBasename.offences("count").count(),
        0
    );
}

#[test]
fn contains_any_agrees_with_first_offence() {
    for value in [
        "clean",
        "",
        "a/b",
        "a\\b",
        "tail\0",
        "a<b>c",
        "quote\"d",
        "tab\there",
        "newline\nhere",
        "ünïcode ✓",
    ] {
        assert_eq!(
            DisallowedClass::FilenameBasename.contains_any(value),
            DisallowedClass::FilenameBasename
                .first_offence(value)
                .is_some(),
            "contains_any and first_offence disagree about {value:?}"
        );
    }
}

// -- the const helpers ------------------------------------------------------

#[test]
fn the_control_test_agrees_with_the_stdlib_across_the_bmp() {
    for code in 0u32..0x1_0000 {
        let Some(character) = char::from_u32(code) else {
            continue;
        };
        assert_eq!(
            is_control_char(character),
            character.is_control(),
            "U+{code:04X}: the const test and char::is_control disagree, so \
             this class is not the same rule check_basename already applies"
        );
    }
}

#[test]
fn the_control_test_agrees_with_the_stdlib_beyond_the_bmp() {
    // No supplementary character is in `Cc`, but the claim should hold for
    // the whole char range rather than only where it was checked.
    for code in [0x1_0000u32, 0x1_FFFF, 0x4_E000, 0xF_FFFD, 0x10_FFFF] {
        let character = char::from_u32(code).expect("a valid scalar value");
        assert_eq!(is_control_char(character), character.is_control());
    }
}

#[test]
fn ascii_contains_finds_every_illegal_character() {
    for character in FILENAME_ILLEGAL_CHARS.chars() {
        assert!(ascii_contains(FILENAME_ILLEGAL_CHARS, character));
    }
    assert!(!ascii_contains(FILENAME_ILLEGAL_CHARS, 'a'));
}

#[test]
fn ascii_contains_never_matches_a_non_ascii_character() {
    for character in ['é', '中', '✓', '\u{2028}'] {
        assert!(
            !ascii_contains(FILENAME_ILLEGAL_CHARS, character),
            "{character:?} is not ASCII and cannot be in the set"
        );
    }
}

// -- the set is shared, not duplicated --------------------------------------

#[test]
fn the_basename_validator_reads_the_canonical_set_rather_than_its_own_copy() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/surface/input_dialog.rs");
    let text = std::fs::read_to_string(path).expect("input_dialog.rs is readable");
    assert!(
        text.contains("crate::validation::disallowed::FILENAME_ILLEGAL_CHARS"),
        "input_dialog.rs no longer aliases the canonical set"
    );
    assert!(
        !text.contains(r##""/\\:*?\"<>|"##),
        "input_dialog.rs carries its own copy of the illegal-character literal, \
         so the two sets can now drift apart"
    );
}
