//! iced theme construction from the FastMD colour tokens ([TASK-008],
//! `doc/planning/iced.md`).
//!
//! [`crate::ui::theme::Palette`] stays the single source of truth for every
//! numeric colour value ([GUD-003]); this module only *maps* those
//! `egui::Color32` tokens into iced structures:
//!
//! - [`to_iced_color`] — the explicit `egui::Color32 → iced::Color`
//!   conversion (premultiplied `Color32` → straight `f32` 0..1 RGBA,
//!   α included),
//! - [`iced_theme_palette`] — the semantic token → iced palette-role mapping
//!   into `iced::theme::palette::Palette` (`background`/`text`/`primary`/
//!   `success`/`warning`/`danger`),
//! - [`fastmd_dark_theme`] — the `iced::Theme::custom` instance handed to the
//!   `iced::application(...).theme(...)` builder (verified
//!   `iced-0.14.0/src/application.rs:377-392`),
//! - [`scrim_color`] — the dimmed `surface` token the blocking modal overlay
//!   paints with ([TASK-017a], `ui_iced/overlay.rs`).
//!
//! No numeric colour literal appears here: every value flows out of
//! [`crate::ui::theme::Palette::fastmd_dark`] ([GUD-003]).
//!
//! Verified iced APIs (local sources): `Theme::custom` /
//! `Theme::custom_with_fn` — `iced_core-0.14.0/src/theme.rs:91-106`;
//! `iced::theme::palette::Palette` fields —
//! `iced_core-0.14.0/src/theme/palette.rs:7-22`; `Color::from_rgba8(r, g, b,
//! a: f32)` — `iced_core-0.14.0/src/color.rs:89`.
//!
//! Unit tests live in the sibling `theme_tests.rs` sidecar.

/// Name under which the FastMD custom iced theme is registered.
pub const FASTMD_THEME_NAME: &str = "FastMD Dark";

/// Converts an egui `Color32` token to an iced [`iced::Color`].
///
/// egui stores `Color32` **premultiplied** (`ecolor-0.36.1/src/color32.rs:31`,
/// "All colors have premultiplied alpha"), while iced `Color` holds straight
/// (unmultiplied) `f32` channels. The conversion therefore goes through
/// `Color32::to_srgba_unmultiplied` (`ecolor-0.36.1/src/color32.rs:273-286`)
/// for the straight RGBA bytes and normalises them to `f32` 0..1 exactly
/// (α included) via `Color::from_rgba8(r, g, b, a)`
/// (`iced_core-0.14.0/src/color.rs:89`).
///
/// Every token in [`crate::ui::theme::Palette`] is fully opaque (α=255), for
/// which `to_srgba_unmultiplied` is the identity — the mapping reproduces the
/// egui values bit-exactly.
pub fn to_iced_color(color: eframe::egui::Color32) -> iced::Color {
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    iced::Color::from_rgba8(r, g, b, f32::from(a) / 255.0)
}

/// Maps the FastMD dark tokens ([`crate::ui::theme::Palette`]) onto the
/// iced palette roles ([GUD-003]: tokens in, no numeric literals added).
///
/// Role assignments (semantic, matching the egui `Visuals` wiring in
/// `crate::ui::theme`):
/// - `background` ← `surface` — panel/window fills,
/// - `text` ← `text` — default body text,
/// - `primary` ← `accent` — selection / brand colour,
/// - `success` ← `success`, `warning` ← `warning`, `danger` ← `error`.
pub fn iced_theme_palette(palette: &crate::ui::theme::Palette) -> iced::theme::palette::Palette {
    iced::theme::palette::Palette {
        background: to_iced_color(palette.surface),
        text: to_iced_color(palette.text),
        primary: to_iced_color(palette.accent),
        success: to_iced_color(palette.success),
        warning: to_iced_color(palette.warning),
        danger: to_iced_color(palette.error),
    }
}

/// Alpha the modal scrim dims the `surface` token to ([TASK-017a]).
///
/// The egui build has no scrim — `egui::Window`s float over an undimmed
/// viewport — so this is a new, overlay-specific token. 0.55 keeps the
/// background legible while reading clearly as "input is blocked".
pub const SCRIM_ALPHA: f32 = 0.55;

/// The semi-transparent modal scrim colour ([TASK-017a]).
///
/// Derived from the [`crate::ui::theme::Palette::surface`] token at
/// [`SCRIM_ALPHA`] rather than from a new literal, so the value stays
/// anchored to the FastMD palette ([GUD-003]).
pub fn scrim_color() -> iced::Color {
    let mut color = to_iced_color(crate::ui::theme::Palette::fastmd_dark().surface);
    color.a = SCRIM_ALPHA;
    color
}

/// Builds the canned FastMD dark iced theme ([TASK-008]).
///
/// Uses `iced::Theme::custom(name, palette)` — `iced_core-0.14.0/src/
/// theme.rs:91-96` — over the canonical dark tokens, so the iced widgets'
/// derived styles (containers, scrollables, tables) resolve against the same
/// hand-tuned scheme the egui build uses. Hand this to
/// `iced::application(...).theme(...)`.
pub fn fastmd_dark_theme() -> iced::Theme {
    iced::Theme::custom(
        FASTMD_THEME_NAME,
        iced_theme_palette(&crate::ui::theme::Palette::fastmd_dark()),
    )
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
