//! Tests for `ui/theme.rs` (UI-002 dark palette, UI-046 inverted editor).
//!
//! Since INT-1 the palette is a *derived* value: the fifteen overlapping
//! tokens come from `hoarfrost::token::color::Palette::dark()`
//! (`UIF-TOK-005`) and only the six app-owned content tokens are
//! literals (decision **D-2**). The tests therefore assert **parity with
//! the framework** — the app value IS the framework value — so a future
//! divergence fails loudly here rather than drifting silently.
//!
//! Pure token-value tests only ([RUST-006]): no window, no process spawn,
//! no filesystem access.

use super::*;
use hoarfrost::token::color::Palette as ProPalette;

/// Every colour token the framework palette carries, as a flat list.
///
/// Used to prove the six app-owned content tokens are genuinely *not*
/// framework values — a content token that quietly became equal to a
/// framework token would mean the D-2 boundary had moved without anyone
/// deciding it.
fn pro_token_values() -> [iced::Color; 23] {
    let pro = ProPalette::dark();
    [
        pro.surface_base,
        pro.surface_sunken,
        pro.surface_raised,
        pro.surface_code,
        pro.surface_overlay,
        pro.surface_hover,
        pro.surface_active,
        pro.surface_scrim,
        pro.border_subtle,
        pro.border_strong,
        pro.accent_fill,
        pro.accent_fill_active,
        pro.accent_border,
        pro.accent_text,
        pro.text_primary,
        pro.text_strong,
        pro.text_muted,
        pro.text_disabled,
        pro.status_success,
        pro.status_warning,
        pro.status_danger_text,
        pro.status_danger_fill,
        pro.status_danger_fill_active,
    ]
}

/// The accessor must hand back the framework's own dark palette, unchanged.
#[test]
fn test_pro_accessor_is_the_framework_dark_palette() {
    assert_eq!(Palette::pro(), ProPalette::dark());
}

/// INT-1 core assertion: every overlapping app token IS the framework
/// token it maps to (`UIF-TOK-005`). Not "equals the same literal" —
/// the same value read from the same source.
#[test]
fn test_fastmd_dark_overlapping_tokens_are_the_framework_tokens() {
    let app = Palette::fastmd_dark();
    let pro = ProPalette::dark();

    let mapped: [(&str, iced::Color, iced::Color); 15] = [
        ("surface", app.surface, pro.surface_base),
        ("surface_variant", app.surface_variant, pro.surface_raised),
        ("code_surface", app.code_surface, pro.surface_code),
        ("table_surface", app.table_surface, pro.surface_overlay),
        ("border", app.border, pro.border_subtle),
        ("border_strong", app.border_strong, pro.border_strong),
        ("text", app.text, pro.text_primary),
        ("text_strong", app.text_strong, pro.text_strong),
        ("text_muted", app.text_muted, pro.text_muted),
        ("text_faint", app.text_faint, pro.text_disabled),
        ("accent", app.accent, pro.accent_fill),
        ("accent_text", app.accent_text, pro.accent_text),
        ("success", app.success, pro.status_success),
        ("warning", app.warning, pro.status_warning),
        ("error", app.error, pro.status_danger_text),
    ];

    for (name, app_value, pro_value) in mapped {
        assert_eq!(
            app_value, pro_value,
            "app token `{name}` must equal its hoarfrost source token, got \
             {app_value:?} vs {pro_value:?}"
        );
    }
}

/// The six app-owned content tokens (decision **D-2**) must still be
/// present, keep their declared FastMD values, and be *unreachable* from
/// the framework palette — proving they were not quietly promoted into
/// framework tokens.
#[test]
fn test_content_tokens_are_app_owned_and_not_framework_values() {
    let app = Palette::fastmd_dark();
    let framework = pro_token_values();

    let owned: [(&str, iced::Color, (u8, u8, u8)); 6] = [
        ("pdf_surface", app.pdf_surface, (35, 30, 20)),
        ("text_value", app.text_value, (220, 220, 220)),
        ("accent_text_soft", app.accent_text_soft, (150, 200, 255)),
        (
            "accent_text_subtle",
            app.accent_text_subtle,
            (160, 190, 220),
        ),
        ("log_timestamp", app.log_timestamp, (96, 96, 96)),
        ("log_category", app.log_category, (173, 216, 230)),
    ];

    for (name, token, (r, g, b)) in owned {
        assert_eq!(
            token,
            iced::Color::from_rgb8(r, g, b),
            "app-owned content token `{name}` must keep its FastMD value"
        );
        for pro_value in framework {
            assert_ne!(
                token, pro_value,
                "app-owned content token `{name}` must not collide with a \
                 framework token ({pro_value:?}); if it is now a framework \
                 role, decision D-2 has changed and this needs a decision, \
                 not a silent rewire"
            );
        }
    }
}

/// UI-002: the canonical dark palette carries the FastMD surface token
/// and the `UIF-TOK-005` accent fill.
#[test]
fn test_fastmd_dark_brand_tokens() {
    let palette = Palette::fastmd_dark();
    assert_eq!(palette.surface, Color::from_rgb8(9, 9, 11));
    // UIF-TOK-005 `color.accent.fill`. Changed from RGB(99, 102, 241)
    // by INT-1; see `test_legacy_pre_int1_colours_are_gone`.
    assert_eq!(palette.accent, Color::from_rgb8(93, 96, 238));
}

/// Every token must be distinct from its neighbours where the original inline
/// literals differed, so collapsing the palette can't silently merge two
/// shades.
#[test]
fn test_fastmd_dark_preserves_distinct_shades() {
    let palette = Palette::fastmd_dark();
    assert_ne!(palette.code_surface, palette.table_surface);
    assert_ne!(palette.text_muted, palette.text_faint);
    assert_ne!(palette.accent_text_soft, palette.accent_text_subtle);
    assert_ne!(palette.log_timestamp, palette.log_category);
    assert_eq!(palette.log_timestamp, Color::from_rgb8(96, 96, 96));
    assert_eq!(palette.log_category, Color::from_rgb8(173, 216, 230));
}

/// Every palette token is fully opaque (α = 1.0) — the theme mapping
/// relies on this: no palette token carries translucency.
///
/// This is why the app palette cannot use the framework's
/// `surface_scrim` token directly; the scrim is composed separately in
/// `ui_iced::theme::scrim_color`.
#[test]
fn test_fastmd_dark_tokens_are_opaque() {
    let palette = Palette::fastmd_dark();
    let tokens = [
        palette.surface,
        palette.surface_variant,
        palette.code_surface,
        palette.table_surface,
        palette.pdf_surface,
        palette.border,
        palette.border_strong,
        palette.text,
        palette.text_strong,
        palette.text_value,
        palette.text_muted,
        palette.text_faint,
        palette.accent,
        palette.accent_text,
        palette.accent_text_soft,
        palette.accent_text_subtle,
        palette.success,
        palette.warning,
        palette.error,
        palette.log_timestamp,
        palette.log_category,
    ];
    for token in tokens {
        assert_eq!(token.a, 1.0, "palette token {token:?} must be opaque");
    }
}

/// The u8 round-trip is exact for every declared brand value: the re-typed
/// `iced::Color` tokens normalise 0..255 bytes to f32 / 255.0 without
/// drift.
#[test]
fn test_brand_values_round_trip_through_rgba8() {
    let palette = Palette::fastmd_dark();
    let checks: [(Color, u8, u8, u8); 6] = [
        (palette.surface, 9, 9, 11),
        (palette.text, 210, 210, 210),
        // UIF-TOK-005 accent.fill (was 99, 102, 241 pre-INT-1).
        (palette.accent, 93, 96, 238),
        (palette.warning, 220, 130, 0),
        // UIF-TOK-005 status.danger.text (was 244, 15, 2 pre-INT-1).
        (palette.error, 255, 92, 92),
        (palette.accent_text, 100, 200, 255),
    ];
    for (token, r, g, b) in checks {
        assert_eq!(token, Color::from_rgb8(r, g, b));
        let [tr, tg, tb, ta] = token.into_rgba8();
        assert_eq!((tr, tg, tb, ta), (r, g, b, 255));
    }
}

/// The three pre-INT-1 values must be gone. Pinned explicitly so a
/// rebase that resurrects one of them fails here rather than quietly
/// re-introducing a colour the `UIF-TOK-005` palette rejected.
#[test]
fn test_legacy_pre_int1_colours_are_gone() {
    let palette = Palette::fastmd_dark();
    assert_ne!(
        palette.accent,
        Color::from_rgb8(99, 102, 241),
        "accent must be UIF-TOK-005 accent.fill, not the pre-INT-1 indigo"
    );
    assert_ne!(
        palette.error,
        Color::from_rgb8(244, 15, 2),
        "error must be UIF-TOK-005 status.danger.text, not the pre-INT-1 red"
    );
    assert_ne!(
        EditorColors::inverted().error,
        Color::from_rgb8(255, 0, 0),
        "the inverted editor error must be UIF-TOK-005 editor.inverted.err, \
         not the pre-INT-1 pure red (gap G-6: 3.9985:1 on white fails the \
         4.5:1 normal-text rule)"
    );
}

/// UI-046 in one assertion set: the inverted editor palette has black text on
/// a white background with a black border, the default equals the inverted
/// one (so callers don't have to opt in), and per-channel contrast is maximal.
#[test]
fn test_editor_colors_inverted_satisfies_ui046() {
    let colors = EditorColors::inverted();

    assert_eq!(colors.background, Color::WHITE);
    assert_eq!(colors.text, Color::BLACK);
    assert_eq!(colors.border, Color::BLACK);
    // UIF-TOK-005 `color.editor.inverted.err` (was 255, 0, 0 pre-INT-1).
    assert_eq!(colors.error, Color::from_rgb8(180, 0, 0));

    assert_eq!(EditorColors::default(), EditorColors::inverted());

    let bg = colors.background;
    let fg = colors.text;
    assert_eq!(bg.r + fg.r, 1.0);
    assert_eq!(bg.g + fg.g, 1.0);
    assert_eq!(bg.b + fg.b, 1.0);
}

/// The editor palette is a *mapping*, not a copy: the three framework-
/// modelled fields must be the framework's own values, read from
/// `EditorPalette::inverted()` (decision **D-4**).
#[test]
fn test_editor_colors_delegate_to_the_framework_editor_palette() {
    let app = EditorColors::inverted();
    let pro = hoarfrost::token::color::EditorPalette::inverted();

    assert_eq!(app.background, pro.background);
    assert_eq!(app.text, pro.text);
    assert_eq!(app.error, pro.error);
}

/// The app's four-field public shape is unchanged by INT-1 — the
/// framework `EditorPalette` has three fields, so `border` is the one
/// field with no framework source. Pinned so nobody "fixes" the shape by
/// dropping it or by silently borrowing the text token.
#[test]
fn test_editor_colors_keep_four_field_shape() {
    let app = EditorColors::inverted();
    let pro = hoarfrost::token::color::EditorPalette::inverted();

    // Three mapped + one app-side constant = four distinct assignments.
    assert_eq!(app.border, Color::BLACK);
    assert_eq!(
        app.border, app.text,
        "the editor border and text are both black today, but `border` is \
         NOT delegated: hoarfrost's EditorPalette carries no border token \
         (UIF-TOK-005 models bg/fg/err only)"
    );
    assert_ne!(
        app.error, pro.text,
        "error must come from the framework error token, never the fg token"
    );
}
