//! Unit tests for the shared iced logo mark.
//!
//! Pure assertions ([RUST-006]): glyph/colour/font identity against the
//! shared tokens, and element-construction smoke tests. No renderer, no
//! window, no filesystem access.

use crate::ui_iced::about::ABOUT_BANNER_SIZE;

use super::*;

#[test]
fn logo_glyph_is_the_shared_phosphor_bolt() {
    assert_eq!(
        logo_glyph(),
        strings::ICON_LIGHTNING,
        "the iced stand-in must be the same Phosphor codepoint the legacy UI \
         stand-in uses"
    );
}

#[test]
fn logo_color_is_the_accent_text_token() {
    assert_eq!(
        logo_color(),
        Palette::fastmd_dark().accent_text,
        "the mark must resolve through the palette, never a literal colour \
         [GUD-003]"
    );
}

#[test]
fn logo_font_is_the_registered_phosphor_family() {
    assert_eq!(
        logo_font(),
        Font::with_name(PHOSPHOR_FONT_NAME),
        "the mark must use the family `ui_iced::fonts` registers at boot"
    );
}

#[test]
fn logo_mark_size_matches_the_top_bar_slot() {
    assert_eq!(
        LOGO_MARK_SIZE, 20.0,
        "top-bar parity: the legacy UI toolbar allocates vec2(20.0, 20.0) for \
         the badge (ui/panels/top.rs:142)"
    );
}

#[test]
fn logo_mark_builds_at_every_caller_size() {
    // Smoke: both real call sites (top bar, About banner) must produce a
    // usable `'static` element without panicking.
    let top: Element<'static, Message> = logo_mark(LOGO_MARK_SIZE);
    let banner: Element<'static, Message> = logo_mark(ABOUT_BANNER_SIZE);
    let _ = (top, banner);
}

#[test]
fn about_banner_glyph_routes_through_the_shared_mark() {
    // [RUST-051]: the About banner must not own its own glyph choice.
    assert_eq!(
        crate::ui_iced::about::banner_glyph(),
        logo_glyph(),
        "the About banner must delegate to `logo::logo_glyph`"
    );
}
