//! iced theme construction from the FastMD colour tokens.
//!
//! [`crate::ui::theme::Palette`] is the single source of truth for every
//! numeric colour value. The tokens are `iced::Color`, so this module only
//! *maps* the semantic tokens onto iced structures — it adds no colour
//! conversion and no literals of its own:
//!
//! - [`iced_theme_palette`] — the semantic token → iced palette-role mapping
//!   into `iced::theme::palette::Palette` (`background`/`text`/`primary`/
//!   `success`/`warning`/`danger`),
//! - [`fastmd_dark_theme`] — the `iced::Theme::custom` instance handed to the
//!   `iced::application(...).theme(...)` builder (verified
//!   `iced-0.14.0/src/application.rs:377-392`),
//! - [`scrim_color`] — the dimmed `surface` token the blocking modal overlay
//!   paints with (`ui_iced/overlay.rs`).
//!
//! No numeric colour literal appears here: every value flows out of
//! [`crate::ui::theme::Palette::fastmd_dark`].
//!
//! Verified iced APIs (local sources): `Theme::custom` /
//! `Theme::custom_with_fn` — `iced_core-0.14.0/src/theme.rs:91-106`;
//! `iced::theme::palette::Palette` fields —
//! `iced_core-0.14.0/src/theme/palette.rs:7-22`.
//!
//! Unit tests live in the sibling `theme_tests.rs` sidecar.

use crate::ui::theme::Palette;

/// Name under which the FastMD custom iced theme is registered.
pub const FASTMD_THEME_NAME: &str = "FastMD Dark";

/// Maps the FastMD dark tokens ([`Palette`]) onto the iced palette roles:
/// tokens in, no numeric literals added.
///
/// Role assignments (semantic):
/// - `background` ← `surface` — panel/window fills,
/// - `text` ← `text` — default body text,
/// - `primary` ← `accent` — selection / brand colour,
/// - `success` ← `success`, `warning` ← `warning`, `danger` ← `error`.
#[must_use]
pub fn iced_theme_palette(palette: &Palette) -> iced::theme::palette::Palette {
    iced::theme::palette::Palette {
        background: palette.surface,
        text: palette.text,
        primary: palette.accent,
        success: palette.success,
        warning: palette.warning,
        danger: palette.error,
    }
}

/// Alpha the modal scrim dims the `surface` token to.
///
/// 0.55 keeps the background legible while reading clearly as
/// "input is blocked".
pub const SCRIM_ALPHA: f32 = 0.55;

/// The semi-transparent modal scrim colour.
///
/// Derived from the [`crate::ui::theme::Palette::surface`] token at
/// [`SCRIM_ALPHA`] rather than from a new literal, so the value stays
/// anchored to the FastMD palette.
#[must_use]
pub fn scrim_color() -> iced::Color {
    let mut color = Palette::fastmd_dark().surface;
    color.a = SCRIM_ALPHA;
    color
}

/// Builds the canned FastMD dark iced theme.
///
/// Uses `iced::Theme::custom(name, palette)` — `iced_core-0.14.0/src/
/// theme.rs:91-96` — over the canonical dark tokens, so the iced widgets'
/// derived styles (containers, scrollables, tables) resolve against the
/// same hand-tuned scheme the palette declares. Hand this to
/// `iced::application(...).theme(...)`.
#[must_use]
pub fn fastmd_dark_theme() -> iced::Theme {
    iced::Theme::custom(
        FASTMD_THEME_NAME,
        iced_theme_palette(&Palette::fastmd_dark()),
    )
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
