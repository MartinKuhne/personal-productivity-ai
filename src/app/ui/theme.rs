//! Centralised colour palette and egui `Visuals` construction (UI-002, UI-046).
//!
//! This module is the single source of truth for every numeric colour the UI
//! paints. Call sites resolve semantic tokens through [`PaletteExt::palette`]
//! (e.g. `ui.palette().error`) or [`Palette::fastmd_dark`], and the egui widget
//! `Visuals` is derived from the same [`Palette`] by [`install_dark`]. Keeping
//! the literals here means a brand change touches one file rather than a dozen
//! panel and renderer modules.
//!
//! The UI is dark-only (UI-002); the inverted inline-editor palette required by
//! UI-046 is modelled as [`EditorColors`] and stays a deliberately separate,
//! self-contained token set.
//!
//! Unit tests live in the sibling `theme_tests.rs` sidecar.

use eframe::egui;

/// Temporary-data key under which [`install_dark`] stores the active
/// [`Palette`] in the egui context.
const PALETTE_KEY: &str = "fastmd_theme_palette";

/// The FastMD dark colour palette (UI-002).
///
/// Every field is a semantic token; call sites should read tokens by role
/// rather than embedding raw `Color32` literals. The values reproduce the
/// hand-tuned scheme that previously lived inline in
/// `FastMdApp::configure_dark_theme`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// Base surface fill for panels and windows (RGB(9, 9, 11)).
    pub surface: egui::Color32,
    /// Slightly raised surface used for selected list rows (RGB(18, 18, 20)).
    pub surface_variant: egui::Color32,
    /// Surface behind fenced code blocks (RGB(20, 20, 22)).
    pub code_surface: egui::Color32,
    /// Surface behind YAML front-matter / data tables (RGB(24, 24, 27)).
    pub table_surface: egui::Color32,
    /// Warm sepia surface marking PDF-backed documents (RGB(35, 30, 20)).
    pub pdf_surface: egui::Color32,
    /// Subtle border around code blocks, tables and inline code (gray 40).
    pub border: egui::Color32,
    /// Stronger border around a whole markdown table perimeter (gray 120).
    pub border_strong: egui::Color32,
    /// Default body text (bright off-white, gray 210).
    pub text: egui::Color32,
    /// Emphasised / active text (pure white).
    pub text_strong: egui::Color32,
    /// Secondary value text such as YAML values (gray 220).
    pub text_value: egui::Color32,
    /// Muted text for hints and thinking traces (gray 160).
    pub text_muted: egui::Color32,
    /// Faint text for search-result metadata (gray 140).
    pub text_faint: egui::Color32,
    /// Primary brand / selection colour (indigo RGB(99, 102, 241)).
    pub accent: egui::Color32,
    /// Headings and titles (cyan RGB(100, 200, 255)).
    pub accent_text: egui::Color32,
    /// Soft accent text such as YAML keys (RGB(150, 200, 255)).
    pub accent_text_soft: egui::Color32,
    /// Subtle blue for search snippets (RGB(160, 190, 220)).
    pub accent_text_subtle: egui::Color32,
    /// Success / completion text (green RGB(100, 255, 100)).
    pub success: egui::Color32,
    /// Warning indicators (orange RGB(220, 130, 0)).
    pub warning: egui::Color32,
    /// Error text and error-marked rows (red RGB(244, 15, 2)).
    pub error: egui::Color32,
    /// Timestamp column of the background-logs viewer (dim gray).
    pub log_timestamp: egui::Color32,
    /// Category tag of the background-logs viewer (light blue).
    pub log_category: egui::Color32,
}

impl Palette {
    /// Returns the FastMD dark palette (UI-002).
    pub const fn fastmd_dark() -> Self {
        Self {
            surface: egui::Color32::from_rgb(9, 9, 11),
            surface_variant: egui::Color32::from_rgb(18, 18, 20),
            code_surface: egui::Color32::from_rgb(20, 20, 22),
            table_surface: egui::Color32::from_rgb(24, 24, 27),
            pdf_surface: egui::Color32::from_rgb(35, 30, 20),
            border: egui::Color32::from_gray(40),
            border_strong: egui::Color32::from_gray(120),
            text: egui::Color32::from_gray(210),
            text_strong: egui::Color32::WHITE,
            text_value: egui::Color32::from_gray(220),
            text_muted: egui::Color32::from_gray(160),
            text_faint: egui::Color32::from_gray(140),
            accent: egui::Color32::from_rgb(99, 102, 241),
            accent_text: egui::Color32::from_rgb(100, 200, 255),
            accent_text_soft: egui::Color32::from_rgb(150, 200, 255),
            accent_text_subtle: egui::Color32::from_rgb(160, 190, 220),
            success: egui::Color32::from_rgb(100, 255, 100),
            warning: egui::Color32::from_rgb(220, 130, 0),
            error: egui::Color32::from_rgb(244, 15, 2),
            log_timestamp: egui::Color32::DARK_GRAY,
            log_category: egui::Color32::LIGHT_BLUE,
        }
    }

    /// Writes this palette onto an egui [`egui::Visuals`] value.
    ///
    /// Only the fields the FastMD app owns are overwritten; everything else is
    /// left at the `Visuals::dark()` default so widget internals (hyperlink,
    /// error, warning, weak text) keep egui's own tuned defaults.
    fn apply_to_visuals(&self, visuals: &mut egui::Visuals) {
        visuals.window_fill = self.surface;
        visuals.panel_fill = self.surface;
        visuals.selection.bg_fill = self.accent;
        visuals.window_corner_radius = 8.0.into();
        visuals.widgets.noninteractive.corner_radius = 4.0.into();
        visuals.widgets.inactive.corner_radius = 4.0.into();
        visuals.widgets.hovered.corner_radius = 4.0.into();
        visuals.widgets.active.corner_radius = 4.0.into();
        visuals.widgets.noninteractive.fg_stroke.color = self.text;
        visuals.widgets.inactive.fg_stroke.color = self.text;
        visuals.widgets.active.fg_stroke.color = self.text_strong;
        visuals.widgets.hovered.fg_stroke.color = self.text_strong;
    }
}

/// Inverted colour scheme for the inline text editor (UI-046).
///
/// The editor must look distinctly different from the rest of the
/// dark-themed application: black text on a white background, with a
/// black border to make the inverted surface visually obvious.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorColors {
    /// Window / frame fill colour (the editor's surface).
    pub background: egui::Color32,
    /// Text colour used inside the editing area and the status bar.
    pub text: egui::Color32,
    /// Border stroke colour around the window.
    pub border: egui::Color32,
    /// Colour used to surface validation / save errors.
    pub error: egui::Color32,
}

impl EditorColors {
    /// Returns the inverted (black text on white) palette required by
    /// UI-046.
    pub const fn inverted() -> Self {
        Self {
            background: egui::Color32::WHITE,
            text: egui::Color32::BLACK,
            border: egui::Color32::BLACK,
            error: egui::Color32::RED,
        }
    }
}

impl Default for EditorColors {
    fn default() -> Self {
        Self::inverted()
    }
}

/// Reads the active FastMD [`Palette`] from an egui `Ui` or `Context`.
pub trait PaletteExt {
    /// Returns the active FastMD palette.
    fn palette(&self) -> Palette;
}

impl PaletteExt for egui::Context {
    fn palette(&self) -> Palette {
        // The palette is stored by `install_dark`; until then (and in tests
        // that never install visuals) fall back to the canonical dark palette.
        self.data(|d| d.get_temp::<Palette>(egui::Id::new(PALETTE_KEY)))
            .unwrap_or_else(Palette::fastmd_dark)
    }
}

impl PaletteExt for egui::Ui {
    fn palette(&self) -> Palette {
        self.ctx().palette()
    }
}

/// Installs the FastMD dark theme on `ctx` (UI-002).
///
/// egui 0.36 splits the global style into a Dark and a Light theme, picked at
/// runtime by `ThemePreference` (default `System`). `set_visuals` writes to the
/// *currently active* theme only, so on systems that report a light-mode
/// preference the next frame can flip the active theme back to the default
/// light visuals and the tuned dark background is lost. Forcing the dark theme
/// and applying the visuals to the dark theme explicitly makes the dark colour
/// scheme the source of truth regardless of the host preference.
pub fn install_dark(ctx: &egui::Context) {
    ctx.set_theme(egui::Theme::Dark);

    let palette = Palette::fastmd_dark();
    let mut visuals = egui::Visuals::dark();
    palette.apply_to_visuals(&mut visuals);
    ctx.set_visuals_of(egui::Theme::Dark, visuals);

    // Publish the palette so `PaletteExt::palette` returns it verbatim.
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(PALETTE_KEY), palette));
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod theme_tests;
