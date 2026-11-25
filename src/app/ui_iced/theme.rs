//! iced theme construction from the framework design tokens.
//!
//! The `hoarfrost` framework crate is the single source of design truth:
//! [`crate::ui::theme::Palette`] derives its overlapping tokens from
//! `hoarfrost::token::color::Palette::dark` (`UIF-TOK-005`), and this
//! module maps that framework palette onto iced structures. It adds no
//! colour conversion and no literals of its own:
//!
//! - [`iced_theme_palette`] — the framework token → iced palette-role
//!   mapping into `iced::theme::palette::Palette` (`background`/`text`/
//!   `primary`/`success`/`warning`/`danger`),
//! - [`fastmd_dark_theme`] — the `iced::Theme::custom` instance handed to
//!   the `iced::application(...).theme(...)` builder (verified
//!   `iced-0.14.0/src/application.rs:377-392`),
//! - [`scrim_color`] — the dimmed backdrop the blocking modal overlay
//!   paints with (`ui_iced/overlay.rs`), delegated to
//!   [`hoarfrost::surface::zorder::scrim_for`] so the scrim alpha lives in
//!   exactly one place ([RUST-021]).
//!
//! No numeric colour literal appears here: every value flows out of the
//! framework palette, reachable through [`crate::ui::theme::Palette::pro`].
//!
//! Verified iced APIs (local sources): `Theme::custom` /
//! `Theme::custom_with_fn` — `iced_core-0.14.0/src/theme.rs:91-106`;
//! `iced::theme::palette::Palette` fields —
//! `iced_core-0.14.0/src/theme/palette.rs:7-22`.
//!
//! Unit tests live in the sibling `theme_tests.rs` sidecar.

use crate::ui::theme::Palette;
use hoarfrost::token::color::Palette as ProPalette;

/// Name under which the FastMD custom iced theme is registered.
pub const FASTMD_THEME_NAME: &str = "FastMD Dark";

/// Maps the framework dark palette ([`ProPalette`], `UIF-TOK-005`) onto
/// the iced palette roles: tokens in, no numeric literals added.
///
/// Role assignments (semantic):
/// - `background` ← `color.surface.base` — panel/window fills,
/// - `text` ← `color.text.primary` — default body text,
/// - `primary` ← `color.accent.fill` — selection / brand colour,
/// - `success` ← `color.status.success`,
/// - `warning` ← `color.status.warning`,
/// - `danger` ← `color.status.danger.text`.
///
/// The framework's remaining roles (`surface_hover`, `accent_border`,
/// `status_danger_fill`, …) have no iced palette slot to land in; iced
/// derives its own interaction shades, which is why the app's own
/// renderers read tokens directly rather than going through this struct.
#[must_use]
pub fn iced_theme_palette(pro: &ProPalette) -> iced::theme::palette::Palette {
    iced::theme::palette::Palette {
        background: pro.surface_base,
        text: pro.text_primary,
        primary: pro.accent_fill,
        success: pro.status_success,
        warning: pro.status_warning,
        danger: pro.status_danger_text,
    }
}

/// The semi-transparent modal scrim colour.
///
/// Delegated to [`hoarfrost::surface::zorder::scrim_for`], which composes
/// `color.surface.base` at the framework's `SCRIM_ALPHA`. The alpha is
/// **not** restated here ([RUST-021]): `hoarfrost` owns the single value,
/// and this is only the app-side name for the composed result.
#[must_use]
pub fn scrim_color() -> iced::Color {
    hoarfrost::surface::zorder::scrim_for(&Palette::pro())
}

/// Builds the canned FastMD dark iced theme.
///
/// Uses `iced::Theme::custom(name, palette)` — `iced_core-0.14.0/src/
/// theme.rs:91-96` — over the framework dark tokens, so the iced widgets'
/// derived styles (containers, scrollables, tables) resolve against the
/// same palette the token module declares. Hand this to
/// `iced::application(...).theme(...)`.
#[must_use]
pub fn fastmd_dark_theme() -> iced::Theme {
    iced::Theme::custom(FASTMD_THEME_NAME, iced_theme_palette(&Palette::pro()))
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
