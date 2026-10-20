//! Unit tests for the iced font bootstrap.
//!
//! Pure assertions only ([RUST-006]): byte-level inventory checks and the
//! error-formatting helpers — no `Task` execution (needs a live runtime),
//! no window/process spawn, besides the delegated system-font *discovery*
//! which may read standard font directories.

use super::*;

#[test]
fn test_phosphor_collection_is_embedded_true_type_font() {
    let (bytes, cow) = (
        crate::ui::fonts::PHOSPHOR_REGULAR_FONT,
        font_collections().remove(0),
    );
    assert!(!bytes.is_empty(), "Phosphor font bytes must not be empty");
    // TrueType sfnt magic ('00 01 00 00').
    assert!(
        bytes.len() >= 4 && &bytes[..4] == b"\x00\x01\x00\x00",
        "Phosphor asset must be a TrueType (sfnt) collection"
    );
    assert_eq!(
        cow.as_ref(),
        bytes,
        "first collection must be the Phosphor font"
    );
}

#[test]
fn test_collections_are_in_registration_order_and_nonempty() {
    let collections = font_collections();
    assert!(
        !collections.is_empty(),
        "at least the embedded Phosphor collection must be present"
    );
    assert!(
        collections.iter().all(|c| !c.is_empty()),
        "no empty font collections may be handed to iced::font::load"
    );
}

#[test]
fn test_phosphor_font_name_matches_thelegacy_registration_key() {
    assert_eq!(
        PHOSPHOR_FONT_NAME, "phosphor",
        "iced family name must mirror the legacy UI 'phosphor' registration key"
    );
}

#[test]
fn test_font_load_error_message_ok_is_none() {
    let results: [Result<(), iced::font::Error>; 1] = [Ok(())];
    assert!(
        font_load_error_message(&results[0]).is_none(),
        "a clean registration must yield no error message"
    );
}
