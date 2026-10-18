//! Shared inline-run styling for the markdown renderers (RUST-023).
//!
//! Three renderers paint the same markdown inline runs through different egui
//! APIs: [`inline`](super::inline) (paragraph and list runs),
//! [`heading`](super::heading) (headings, at a level-dependent size) and
//! `table::cell` (which has both a `RichText` path and a `LayoutJob` path).
//!
//! Before this module each of them re-implemented the `bold` / `italic` /
//! `code` / `muted` / `strikethrough` → styling mapping, and the two cell
//! paths could silently drift apart (the module docs on `table::cell` warn
//! that the `LayoutJob` path must stay consistent with the label path). Both
//! mappings now live here — [`styled_text`] for the `RichText` path and
//! [`text_format`] for the `LayoutJob` path — so a change to the mapping is
//! made exactly once.
//!
//! Unit tests live in the sibling `style_tests.rs` sidecar.

use crate::markdown::{TextStyle, WIKILINK_SCHEME};
use crate::ui::theme::PaletteExt;
use eframe::egui;
use egui::RichText;
use egui::epaint::text::TextFormat;

/// The resolved colours a [`TextFormat`] run needs, derived once from the
/// active egui `Visuals` and the FastMD [`Palette`](crate::ui::theme::Palette).
///
/// Kept as an explicit value (rather than reading `ui` inside the per-run
/// builder) so [`text_format`] stays a pure function that tests can call
/// without an egui context.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RunColors {
    /// Default body text colour.
    pub text: egui::Color32,
    /// Colour for strong/bold runs.
    pub strong: egui::Color32,
    /// Background behind inline code spans.
    pub code_bg: egui::Color32,
    /// Colour for muted/secondary runs.
    pub muted: egui::Color32,
}

impl RunColors {
    /// Resolve the run colours from the active [`egui::Ui`].
    ///
    /// `code_bg` uses the palette's `border` token (matching the `RichText`
    /// path in [`styled_text`]) rather than egui's `visuals.code_bg_color`, so
    /// the two cell paths cannot drift.
    pub(crate) fn from_ui(ui: &egui::Ui) -> Self {
        let visuals = ui.visuals();
        Self {
            text: visuals.text_color(),
            strong: visuals.strong_text_color(),
            code_bg: ui.palette().border,
            muted: ui.palette().text_muted,
        }
    }
}

/// Builds the [`RichText`] for one styled inline text run.
///
/// `size` is applied when `Some` (headings pass their level size; table cells
/// and paragraph runs pass `None`). The `code` background comes from the
/// palette's `border` token; the `muted` colour from the palette's
/// `text_muted` token.
pub(crate) fn styled_text(
    text: &str,
    style: &TextStyle,
    size: Option<f32>,
    palette: crate::ui::theme::Palette,
) -> RichText {
    let mut rt = RichText::new(text);
    if let Some(size) = size {
        rt = rt.size(size);
    }
    if style.bold {
        rt = rt.strong();
    }
    if style.italic {
        rt = rt.italics();
    }
    if style.code {
        rt = rt.monospace().background_color(palette.border);
    }
    if style.muted {
        rt = rt.color(palette.text_muted);
    }
    if style.strikethrough {
        rt = rt.strikethrough();
    }
    rt
}

/// The [`TextFormat`] counterpart of [`styled_text`] for the inline
/// `LayoutJob` table-cell path.
///
/// Both builders derive from the same [`TextStyle`] mapping; keeping them in
/// one module is what stops the `LayoutJob` path drifting from the label path
/// (see the `table::cell` module note).
pub(crate) fn text_format(
    style: &TextStyle,
    body_font: &egui::FontId,
    mono_font: &egui::FontId,
    colors: RunColors,
) -> TextFormat {
    let font_id = if style.code {
        mono_font.clone()
    } else {
        body_font.clone()
    };
    let color = if style.muted {
        colors.muted
    } else if style.bold {
        colors.strong
    } else {
        colors.text
    };
    TextFormat {
        font_id,
        color,
        background: if style.code {
            colors.code_bg
        } else {
            egui::Color32::TRANSPARENT
        },
        italics: style.italic,
        strikethrough: if style.strikethrough {
            egui::Stroke::new(1.0, colors.text)
        } else {
            egui::Stroke::NONE
        },
        ..Default::default()
    }
}

/// The hover tooltip for a markdown link URL.
///
/// In-app wikilinks (`wikilink:<target>`) show `"Note: <target>"`; every other
/// URL shows itself verbatim. Extracted from the three inline renderers
/// (RUST-021/023).
pub(crate) fn link_hover_tip(url: &str) -> String {
    match url.strip_prefix(WIKILINK_SCHEME) {
        Some(target) => format!("Note: {target}"),
        None => url.to_owned(),
    }
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `style_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "style_tests.rs"]
mod style_tests;
