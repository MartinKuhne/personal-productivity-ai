//! Mapping the hoarfrost token palette onto an iced theme.
//!
//! `hoarfrost::token::color::Palette` is the single source of design truth
//! for this demo (`UIF-TOK-005`). The framework widgets read that palette
//! directly; iced's *own* widgets (the `scrollable`, `container` and text
//! primitives this demo uses to build its shell) resolve against an
//! `iced::Theme` instead. [`demo_theme`] is the bridge: it copies the
//! framework's semantic colours into the six iced palette roles and adds no
//! numeric literal of its own.
//!
//! Mirrors `src/app/ui_iced/theme.rs`, which does the same job for the
//! real application.

use hoarfrost::token::color::Palette;

/// Name under which this demo's custom iced theme is registered.
pub const DEMO_THEME_NAME: &str = "hoarfrost Demo";

/// Maps the framework dark palette onto the six iced palette roles.
///
/// Role assignments mirror the host application's mapping:
/// * `background` ← `surface_base` — window and panel fills,
/// * `text` ← `text_primary` — default body text,
/// * `primary` ← `accent_fill` — selection and brand colour,
/// * `success` ← `status_success`,
/// * `warning` ← `status_warning`,
/// * `danger` ← `status_danger_text`.
///
/// The framework's remaining roles (`surface_hover`, `accent_border`, …)
/// have no iced slot to land in; iced derives its own interaction shades,
/// which is why the hoarfrost widgets take a [`Palette`] explicitly rather
/// than going through this struct.
#[must_use]
pub fn demo_palette(pro: &Palette) -> iced::theme::palette::Palette {
    iced::theme::palette::Palette {
        background: pro.surface_base,
        text: pro.text_primary,
        primary: pro.accent_fill,
        success: pro.status_success,
        warning: pro.status_warning,
        danger: pro.status_danger_text,
    }
}

/// The canned dark iced theme for the demo window.
///
/// Hand this to `iced::application(...).theme(...)`.
#[must_use]
pub fn demo_theme() -> iced::Theme {
    iced::Theme::custom(DEMO_THEME_NAME, demo_palette(&Palette::dark()))
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
