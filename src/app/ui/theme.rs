//! Centralised colour palette for the FastMD UI (UI-002, UI-046).
//!
//! This module is the single source of truth for every numeric colour the UI
//! paints. Call sites resolve semantic tokens through [`Palette::fastmd_dark`]
//! (e.g. `Palette::fastmd_dark().error`); the iced layer maps the same tokens
//! into iced theme structures in [`crate::ui_iced::theme`]. Keeping the
//! literals here means a brand change touches one file rather than a dozen
//! panel and renderer modules.
//!
//! The token type is `iced::Color` ([GUD-003]: the numeric VALUES are
//! identical to the legacy scheme they replace — only the type changed with
//! the framework swap, [TASK-029b]).
//!
//! The UI is dark-only (UI-002); the inverted inline-editor palette required by
//! UI-046 is modelled as [`EditorColors`] and stays a deliberately separate,
//! self-contained token set.
//!
//! Unit tests live in the sibling `theme_tests.rs` sidecar.

use iced::Color;

/// The FastMD dark colour palette (UI-002).
///
/// Every field is a semantic token; call sites should read tokens by role
/// rather than embedding raw colour literals. The values reproduce the
/// hand-tuned scheme that previously lived inline in
/// `FastMdApp::configure_dark_theme`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    /// Base surface fill for panels and windows (RGB(9, 9, 11)).
    pub surface: Color,
    /// Slightly raised surface used for selected list rows (RGB(18, 18, 20)).
    pub surface_variant: Color,
    /// Surface behind fenced code blocks (RGB(20, 20, 22)).
    pub code_surface: Color,
    /// Surface behind YAML front-matter / data tables (RGB(24, 24, 27)).
    pub table_surface: Color,
    /// Warm sepia surface marking PDF-backed documents (RGB(35, 30, 20)).
    pub pdf_surface: Color,
    /// Subtle border around code blocks, tables and inline code (gray 40).
    pub border: Color,
    /// Stronger border around a whole markdown table perimeter (gray 120).
    pub border_strong: Color,
    /// Default body text (bright off-white, gray 210).
    pub text: Color,
    /// Emphasised / active text (pure white).
    pub text_strong: Color,
    /// Secondary value text such as YAML values (gray 220).
    pub text_value: Color,
    /// Muted text for hints and thinking traces (gray 160).
    pub text_muted: Color,
    /// Faint text for search-result metadata (gray 140).
    pub text_faint: Color,
    /// Primary brand / selection colour (indigo RGB(99, 102, 241)).
    pub accent: Color,
    /// Headings and titles (cyan RGB(100, 200, 255)).
    pub accent_text: Color,
    /// Soft accent text such as YAML keys (RGB(150, 200, 255)).
    pub accent_text_soft: Color,
    /// Subtle blue for search snippets (RGB(160, 190, 220)).
    pub accent_text_subtle: Color,
    /// Success / completion text (green RGB(100, 255, 100)).
    pub success: Color,
    /// Warning indicators (orange RGB(220, 130, 0)).
    pub warning: Color,
    /// Error text and error-marked rows (red RGB(244, 15, 2)).
    pub error: Color,
    /// Timestamp column of the background-logs viewer (dark gray 96).
    pub log_timestamp: Color,
    /// Category tag of the background-logs viewer (light blue 173/216/230).
    pub log_category: Color,
}

impl Palette {
    /// Returns the FastMD dark palette (UI-002).
    #[must_use]
    pub const fn fastmd_dark() -> Self {
        Self {
            surface: Color::from_rgb8(9, 9, 11),
            surface_variant: Color::from_rgb8(18, 18, 20),
            code_surface: Color::from_rgb8(20, 20, 22),
            table_surface: Color::from_rgb8(24, 24, 27),
            pdf_surface: Color::from_rgb8(35, 30, 20),
            border: Color::from_rgb8(40, 40, 40),
            border_strong: Color::from_rgb8(120, 120, 120),
            text: Color::from_rgb8(210, 210, 210),
            text_strong: Color::WHITE,
            text_value: Color::from_rgb8(220, 220, 220),
            text_muted: Color::from_rgb8(160, 160, 160),
            text_faint: Color::from_rgb8(140, 140, 140),
            accent: Color::from_rgb8(99, 102, 241),
            accent_text: Color::from_rgb8(100, 200, 255),
            accent_text_soft: Color::from_rgb8(150, 200, 255),
            accent_text_subtle: Color::from_rgb8(160, 190, 220),
            success: Color::from_rgb8(100, 255, 100),
            warning: Color::from_rgb8(220, 130, 0),
            error: Color::from_rgb8(244, 15, 2),
            log_timestamp: Color::from_rgb8(96, 96, 96),
            log_category: Color::from_rgb8(173, 216, 230),
        }
    }
}

/// Inverted colour scheme for the inline text editor (UI-046).
///
/// The editor must look distinctly different from the rest of the
/// dark-themed application: black text on a white background, with a
/// black border to make the inverted surface visually obvious.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorColors {
    /// Window / frame fill colour (the editor's surface).
    pub background: Color,
    /// Text colour used inside the editing area and the status bar.
    pub text: Color,
    /// Border stroke colour around the window.
    pub border: Color,
    /// Colour used to surface validation / save errors.
    pub error: Color,
}

impl EditorColors {
    /// Returns the inverted (black text on white) palette required by
    /// UI-046.
    #[must_use]
    pub const fn inverted() -> Self {
        Self {
            background: Color::WHITE,
            text: Color::BLACK,
            border: Color::BLACK,
            error: Color::from_rgb8(255, 0, 0),
        }
    }
}

impl Default for EditorColors {
    fn default() -> Self {
        Self::inverted()
    }
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod theme_tests;
