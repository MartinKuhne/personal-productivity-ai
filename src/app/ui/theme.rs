//! Centralised colour palette for the FastMD UI (UI-002, UI-046).
//!
//! This module is the app-side entry point for every numeric colour the UI
//! paints. Call sites resolve semantic tokens through [`Palette::fastmd_dark`]
//! (e.g. `Palette::fastmd_dark().error`); the iced layer maps the same tokens
//! into iced theme structures in [`crate::ui_iced::theme`].
//!
//! # Where the values come from
//!
//! The `iced-pro` framework crate is the single source of design truth.
//! Every **overlapping** semantic token here is *derived* from
//! [`iced_pro::token::color::Palette::dark`] (`UIF-TOK-005`) rather than
//! written as a literal — this module keeps the FastMD token *names* the
//! renderers already speak and adds no colour value of its own. The rename
//! is one line per field in [`Palette::fastmd_dark`]:
//!
//! | FastMD token | `iced-pro` source (`UIF-TOK-005`) |
//! |---|---|
//! | `surface` | `color.surface.base` |
//! | `surface_variant` | `color.surface.raised` |
//! | `code_surface` | `color.surface.code` |
//! | `table_surface` | `color.surface.overlay` |
//! | `border` | `color.border.subtle` |
//! | `border_strong` | `color.border.strong` |
//! | `text` | `color.text.primary` |
//! | `text_strong` | `color.text.strong` |
//! | `text_muted` | `color.text.muted` |
//! | `text_faint` | `color.text.disabled` |
//! | `accent` | `color.accent.fill` |
//! | `accent_text` | `color.accent.text` |
//! | `success` | `color.status.success` |
//! | `warning` | `color.status.warning` |
//! | `error` | `color.status.danger.text` |
//!
//! The parity is asserted against the framework itself in the sidecar, so a
//! future divergence between app token and framework token fails loudly
//! instead of drifting.
//!
//! # The six app-owned content tokens
//!
//! [`Palette::pdf_surface`], [`Palette::text_value`],
//! [`Palette::accent_text_soft`], [`Palette::accent_text_subtle`],
//! [`Palette::log_timestamp`] and [`Palette::log_category`] stay
//! **app-owned literals**. They are FastMD *document and log content*
//! colours — the sepia of a PDF-backed row, the blue of a YAML key, the
//! timestamp column of the log viewer — not framework UI roles, so
//! decision **D-2** (`doc/planning/iced-pro-tasks.md`) keeps them out of
//! `iced-pro`. They must not be invented there, and they are the only
//! colour literals left in this file.
//!
//! Reach the underlying framework palette directly through
//! [`Palette::pro`] when a consumer wants the source rather than the
//! app-side rename.
//!
//! # Rendered values that changed under INT-1
//!
//! Aligning to the `UIF-TOK-005` palette moved three colours that are
//! visible on screen:
//!
//! - `accent` `RGB(99, 102, 241)` → `RGB(93, 96, 238)`
//! - `error` `RGB(244, 15, 2)` → `RGB(255, 92, 92)`
//! - the inverted editor's `error` `RGB(255, 0, 0)` → `RGB(180, 0, 0)`
//!   (decision **D-4**; the darker red clears the 4.5:1 normal-text rule
//!   where pure red failed it — gap **G-6**)
//!
//! The other twelve mapped fields were already byte-identical.
//!
//! The UI is dark-only (UI-002); the inverted inline-editor palette required by
//! UI-046 is modelled as [`EditorColors`] and stays a deliberately separate,
//! self-contained token set.
//!
//! Unit tests live in the sibling `theme_tests.rs` sidecar.

use iced::Color;
use iced_pro::token::color::EditorPalette;
use iced_pro::token::color::Palette as ProPalette;

/// The FastMD dark colour palette (UI-002).
///
/// Every field is a semantic token; call sites should read tokens by role
/// rather than embedding raw colour literals. The fifteen overlapping
/// fields are derived from [`iced_pro::token::color::Palette::dark`]
/// (`UIF-TOK-005`); the six content tokens are app-owned (decision
/// **D-2**). See this module's doc comment for the full mapping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    /// Base surface fill for panels and windows — `color.surface.base`,
    /// `RGB(9, 9, 11)`.
    pub surface: Color,
    /// Slightly raised surface used for selected list rows —
    /// `color.surface.raised`, `RGB(18, 18, 20)`.
    pub surface_variant: Color,
    /// Surface behind fenced code blocks — `color.surface.code`,
    /// `RGB(20, 20, 22)`.
    pub code_surface: Color,
    /// Surface behind YAML front-matter / data tables —
    /// `color.surface.overlay`, `RGB(24, 24, 27)`.
    pub table_surface: Color,
    /// Warm sepia surface marking PDF-backed documents — **app-owned**
    /// content colour (`RGB(35, 30, 20)`), not a framework UI token
    /// under decision **D-2**.
    pub pdf_surface: Color,
    /// Subtle border around code blocks, tables and inline code —
    /// `color.border.subtle` (gray 40).
    pub border: Color,
    /// Stronger border around a whole markdown table perimeter —
    /// `color.border.strong` (gray 120).
    pub border_strong: Color,
    /// Default body text — `color.text.primary` (bright off-white,
    /// gray 210).
    pub text: Color,
    /// Emphasised / active text — `color.text.strong` (pure white).
    pub text_strong: Color,
    /// Secondary value text such as YAML values — **app-owned** content
    /// colour (gray 220); the framework has no document-value text role
    /// under decision **D-2**.
    pub text_value: Color,
    /// Muted text for hints and thinking traces — `color.text.muted`
    /// (gray 160).
    pub text_muted: Color,
    /// Faint text for search-result metadata — `color.text.disabled`
    /// (gray 140).
    pub text_faint: Color,
    /// Primary brand / selection colour — `color.accent.fill`,
    /// `RGB(93, 96, 238)`.
    ///
    /// Moved from `RGB(99, 102, 241)` when the app was aligned to
    /// `UIF-TOK-005`. Note the framework's polarity split: this is the
    /// *fill* role. A stroke (focus ring, active indicator) is
    /// `color.accent.border` (`ProPalette::accent_border`), which the
    /// app does not currently use.
    pub accent: Color,
    /// Headings and titles — `color.accent.text`, `RGB(100, 200, 255)`.
    pub accent_text: Color,
    /// Soft accent text such as YAML keys — **app-owned** content colour
    /// (`RGB(150, 200, 255)`); the framework has no second accent text
    /// rung under decision **D-2**.
    pub accent_text_soft: Color,
    /// Subtle blue for search snippets — **app-owned** content colour
    /// (`RGB(160, 190, 220)`), not a framework UI token under decision
    /// **D-2**.
    pub accent_text_subtle: Color,
    /// Success / completion text — `color.status.success`,
    /// `RGB(100, 255, 100)`.
    pub success: Color,
    /// Warning indicators — `color.status.warning`, `RGB(220, 130, 0)`.
    pub warning: Color,
    /// Error text and error-marked rows — `color.status.danger.text`,
    /// `RGB(255, 92, 92)`.
    ///
    /// Moved from `RGB(244, 15, 2)` when the app was aligned to
    /// `UIF-TOK-005`. Per decision **D-5** this is the *text/icon* red;
    /// the destructive *background* red is the separate
    /// `color.status.danger.fill`, which the app does not currently use.
    pub error: Color,
    /// Timestamp column of the background-logs viewer — **app-owned**
    /// content colour (dark gray 96), not a framework UI token under
    /// decision **D-2**.
    pub log_timestamp: Color,
    /// Category tag of the background-logs viewer — **app-owned**
    /// content colour (light blue 173/216/230), not a framework UI
    /// token under decision **D-2**.
    pub log_category: Color,
}

impl Palette {
    /// The framework palette this app's tokens are derived from
    /// (`iced_pro::token::color::Palette::dark`, `UIF-TOK-005`).
    ///
    /// Exposed so a consumer that wants the framework's own vocabulary —
    /// `surface_hover`, `accent_border`, `status_danger_fill` — can read
    /// it without going through the app-side rename, and so the parity
    /// tests can assert against the source rather than a copy.
    #[must_use]
    pub const fn pro() -> ProPalette {
        ProPalette::dark()
    }

    /// Returns the FastMD dark palette (UI-002).
    ///
    /// Every overlapping semantic field is read off [`Palette::pro`];
    /// only the six app-owned content tokens (decision **D-2**) are
    /// literals here.
    #[must_use]
    pub const fn fastmd_dark() -> Self {
        let pro = Self::pro();
        Self {
            surface: pro.surface_base,
            surface_variant: pro.surface_raised,
            code_surface: pro.surface_code,
            table_surface: pro.surface_overlay,
            border: pro.border_subtle,
            border_strong: pro.border_strong,
            text: pro.text_primary,
            text_strong: pro.text_strong,
            text_muted: pro.text_muted,
            text_faint: pro.text_disabled,
            accent: pro.accent_fill,
            accent_text: pro.accent_text,
            success: pro.status_success,
            warning: pro.status_warning,
            error: pro.status_danger_text,
            // --- app-owned content tokens (D-2) -----------------------------
            // FastMD document/log content colours. The framework does not
            // model them and must not (decision D-2), so they stay literals.
            pdf_surface: Color::from_rgb8(35, 30, 20),
            text_value: Color::from_rgb8(220, 220, 220),
            accent_text_soft: Color::from_rgb8(150, 200, 255),
            accent_text_subtle: Color::from_rgb8(160, 190, 220),
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
///
/// The three colours the framework models are delegated to
/// [`iced_pro::token::color::EditorPalette::inverted`] (`UIF-TOK-005`,
/// `color.editor.inverted.*`); the app's four-field public shape is kept
/// unchanged and mapped from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorColors {
    /// Window / frame fill colour (the editor's surface) —
    /// `color.editor.inverted.bg`, `RGB(255, 255, 255)`.
    pub background: Color,
    /// Text colour used inside the editing area and the status bar —
    /// `color.editor.inverted.fg`, `RGB(0, 0, 0)`.
    pub text: Color,
    /// Border stroke colour around the window — `RGB(0, 0, 0)`.
    ///
    /// **Not a framework delegation:** `iced_pro::token::color::EditorPalette`
    /// carries no border token (`UIF-TOK-005` models bg/fg/err only), so
    /// there is nothing to map this from. It stays the named iced
    /// constant `Color::BLACK`, which [`EditorColors::text`] also
    /// resolves to today. Reported as a framework token gap rather than
    /// filled by borrowing the text token, which would claim the border
    /// *is* the text colour.
    pub border: Color,
    /// Colour used to surface validation / save errors —
    /// `color.editor.inverted.err`, `RGB(180, 0, 0)`.
    ///
    /// Moved from `RGB(255, 0, 0)` per decision **D-4**. The old pure
    /// red measured `3.9985:1` on white and failed the `UIF-TOK-006`
    /// 4.5:1 normal-text rule (gap **G-6**); `RGB(180, 0, 0)` clears it.
    pub error: Color,
    /// Placeholder text on the inverted canvas — `RGB(115, 115, 115)`.
    ///
    /// **Framework token gap, reported not filled blindly:**
    /// `iced_pro::token::color::EditorPalette` models `bg` / `fg` / `err`
    /// only (`UIF-TOK-005`), and a real iced `text_editor` also needs a
    /// placeholder colour. It is pinned here rather than in the editor
    /// module so [AGENTS §4a] holds, and the sidecar asserts it clears the
    /// `MIN_TEXT_CONTRAST` 4.5:1 rule against [`background`] — the same
    /// bar the error colour was moved to meet.
    ///
    /// [`background`]: Self::background
    pub placeholder: Color,
    /// Selection highlight on the inverted canvas — `RGB(90, 140, 210)`.
    ///
    /// Same token gap as [`placeholder`](Self::placeholder): the framework
    /// carries no selection colour. Chosen against two measured bounds,
    /// both asserted in the sidecar via
    /// [`iced_pro::token::contrast`] rather than eyeballed:
    /// * `meets_chrome_contrast` against the white canvas (≥ 3.0) — it
    ///   measured `3.43:1`, so the highlight is actually visible;
    /// * `meets_text_contrast` for the **black buffer text sitting on it**
    ///   (≥ 4.5) — it measured `6.12:1`, so a selection never turns the
    ///   text unreadable.
    ///
    /// A lighter blue clears the first bound and fails the second, which is
    /// why this one is darker than it looks.
    pub selection: Color,
}

impl EditorColors {
    /// Returns the inverted (black text on white) palette required by
    /// UI-046, mapped from the framework's
    /// [`iced_pro::token::color::EditorPalette::inverted`].
    #[must_use]
    pub const fn inverted() -> Self {
        let pro = EditorPalette::inverted();
        Self {
            background: pro.background,
            text: pro.text,
            border: Color::BLACK,
            error: pro.error,
            placeholder: Color::from_rgb8(115, 115, 115),
            selection: Color::from_rgb8(90, 140, 210),
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
