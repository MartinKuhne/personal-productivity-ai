//! Unit tests for the framework-free font inventory.
//!
//! Pure inventory/discovery tests only ([RUST-006]): no renderer, no
//! windowing. Glyph-shape coverage belongs to the rendering layer — the
//! iced runtime registers these bytes via `crate::ui_iced::fonts`.

use super::*;

#[test]
fn test_candidate_fallback_font_paths_non_empty() {
    let paths = candidate_fallback_font_paths();
    assert!(
        !paths.is_empty(),
        "Candidate fallback font paths list must not be empty"
    );
}

#[test]
fn test_phosphor_font_inventory_is_a_valid_ttf() {
    // The embedded byte collection must be non-empty and carry the TrueType
    // magic number (0x00010000) at the head.
    assert!(
        PHOSPHOR_REGULAR_FONT.len() > 4,
        "Phosphor font bytes must be non-trivial, got {} bytes",
        PHOSPHOR_REGULAR_FONT.len()
    );
    assert_eq!(
        &PHOSPHOR_REGULAR_FONT[..4],
        &[0x00, 0x01, 0x00, 0x00],
        "Phosphor font must start with the TrueType magic number"
    );
}

#[test]
fn test_load_fallback_font_returns_bytes_or_none() {
    // The loader either finds a readable candidate or reports None — it
    // must never panic on missing files.
    let result = load_fallback_font();
    if let Some(bytes) = &result {
        assert!(
            !bytes.is_empty(),
            "a loaded fallback font must not be empty"
        );
    }
}
