//! Glyph-resolution probe: which physical face each symbol the app renders
//! actually lands on.
//!
//! The app asks iced for generic handles — `Font::DEFAULT`, `Font::MONOSPACE`
//! — and for the embedded Phosphor icon font by name, so "did that `✓` come
//! from Segoe UI Symbol or from a tofu box" is not visible from any of the
//! app's own types. This module asks the question at the only layer that can
//! answer it: it shapes a single codepoint through iced's own
//! [`to_attributes`] over the process-wide [`font_system`] — the same
//! database, the same cosmic-text fallback walk, the same
//! `Shaping::Advanced` the widgets use — and reads the `font_id` back off
//! the laid-out glyph.
//!
//! A glyph with id `0` is `.notdef`, the not-defined glyph every TrueType
//! and CFF face carries, and is what the user sees as a tofu box. A
//! codepoint that produces no glyph at all is reported the same way.
//!
//! The probe deliberately does **not** register anything: whether
//! [`crate::ui_iced::fonts::register_fonts`] has run is exactly what the
//! race test measures. Callers that want the registered state call it first.
//!
//! Test-only: gated `#[cfg(test)]` at its declaration in [`crate::ui_iced`],
//! so it adds nothing to the shipped crate.
//!
//! Unit tests live in the sibling `glyph_resolution_tests.rs` sidecar.

use iced::Font;
use iced::advanced::graphics::text::{cosmic_text, font_system, to_attributes};

/// The symbol codepoints the FastMD UI renders as glyphs, as the UI renders
/// them.
///
/// These are the check marks, warning triangles, disclosure triangles,
/// bullets and arithmetic signs the panels and Markdown renderers emit; each
/// one must land on a real face rather than `.notdef`.
pub const SYMBOL_CODEPOINTS: [char; 14] = [
    '\u{2713}',  // ✓ check mark
    '\u{2715}',  // ✕ multiplication X
    '\u{26A0}',  // ⚠ warning sign
    '\u{25CF}',  // ● black circle
    '\u{25B2}',  // ▲ black up-pointing triangle
    '\u{25BC}',  // ▼ black down-pointing triangle
    '\u{25BE}',  // ▾ black down-pointing small triangle
    '\u{25B4}',  // ▴ black up-pointing small triangle
    '\u{1F50D}', // 🔍 left-pointing magnifying glass
    '\u{2630}',  // ☰ trigram for heaven
    '\u{2212}',  // − minus sign
    '\u{00D7}',  // × multiplication sign
    '\u{2022}',  // • bullet
    '\u{22EF}',  // ⋯ midline horizontal ellipsis
];

/// The iced font handles the app actually renders with, each with the short
/// label the report gives it.
///
/// Mirrors the real call sites: `Font::DEFAULT` for ordinary UI text,
/// `Font::MONOSPACE` for inline code and fenced blocks, and
/// `Font::with_name(`[`PHOSPHOR_FONT_NAME`]`)` for the icon glyphs.
pub const PROBED_FONTS: [(Font, &str); 3] = [
    (Font::DEFAULT, "default"),
    (Font::MONOSPACE, "mono"),
    (
        Font::with_name(crate::ui_iced::fonts::PHOSPHOR_FONT_NAME),
        "phosphor",
    ),
];

/// Text size the probe shapes at — the app's body rung.
const PROBE_SIZE: f32 = 14.0;
/// Line height the probe shapes at, matching [`PROBE_SIZE`].
const PROBE_LINE_HEIGHT: f32 = 20.0;
/// Column width for the rendered face name in [`Resolution::line`].
const FACE_COLUMN: usize = 30;
/// Label [`handle_resolves_to_family`] gives its throwaway probe resolution.
const PROBE_LABEL: &str = "probe";

/// One codepoint's resolution outcome under one font handle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    /// The codepoint that was shaped.
    pub ch: char,
    /// The short label of the iced `Font` it was shaped under.
    pub font: &'static str,
    /// The family name of the face that drew it, e.g. `Segoe UI Symbol`.
    pub face: String,
    /// The PostScript name of that face, e.g. `SegoeUISymbol`.
    pub post_script: String,
    /// The glyph index that face drew, `0` meaning `.notdef`.
    pub glyph_id: u16,
    /// Whether a real, non-`.notdef` glyph was found.
    pub resolved: bool,
}

impl Resolution {
    /// One report line: `U+2713 ✓ default -> Segoe UI Symbol [SegoeUISymbol] gid=305 ok`.
    #[must_use]
    pub fn line(&self) -> String {
        let name = format!("{} [{}]", self.face, self.post_script);
        let verdict = if self.resolved { "ok" } else { "TOFU" };
        format!(
            "U+{:04X} {} {:<8} -> {:<FACE_COLUMN$} gid={:<5} {}",
            u32::from(self.ch),
            self.ch,
            self.font,
            name,
            self.glyph_id,
            verdict
        )
    }
}

/// Resolves `ch` under `font` through the live iced font chain.
#[must_use]
pub fn resolve(font: Font, label: &'static str, ch: char) -> Resolution {
    let mut guard = font_system()
        .write()
        .expect("the shared iced font system lock is not poisoned");
    let system = guard.raw();

    match first_glyph(system, font, ch) {
        Some((id, glyph_id)) => {
            let (face, post_script) = face_names(system, id);
            Resolution {
                ch,
                font: label,
                face,
                post_script,
                glyph_id,
                resolved: glyph_id != 0,
            }
        }
        None => Resolution {
            ch,
            font: label,
            face: String::from("<no glyph>"),
            post_script: String::from("<none>"),
            glyph_id: 0,
            resolved: false,
        },
    }
}

/// Every codepoint under every probed font handle, in declaration order.
#[must_use]
pub fn resolve_all() -> Vec<Resolution> {
    PROBED_FONTS
        .iter()
        .flat_map(|(font, label)| SYMBOL_CODEPOINTS.map(|ch| resolve(*font, label, ch)))
        .collect()
}

/// The whole probe rendered as a report.
///
/// The header names the fallback file the inventory would discover and
/// whether registration has run, so a reader can tell "the fallback is in
/// the chain and changed nothing" apart from "the fallback never loaded"
/// without a second run.
#[must_use]
pub fn report() -> String {
    let resolutions = resolve_all();
    let ok = resolutions.iter().filter(|r| r.resolved).count();
    let registered = if crate::ui_iced::fonts::registration_attempted() {
        "registered"
    } else {
        "NOT registered"
    };
    let fallback = crate::ui::fonts::candidate_fallback_font_paths()
        .into_iter()
        .find(|path| path.exists())
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| String::from("<none discoverable>"));

    let mut out = format!(
        "glyph resolution: {ok}/{} resolved | registration: {registered} | fallback: {fallback}\n",
        resolutions.len()
    );
    for resolution in &resolutions {
        out.push_str(&resolution.line());
        out.push('\n');
    }
    out
}

/// Whether a face whose family name matches `family_name` (case-insensitive)
/// is present in the live font database at all.
///
/// The positive control that keeps [`handle_resolves_to_family`] from
/// passing vacuously: if the face is not in the database, "the handle never
/// reaches it" is true for the wrong reason.
#[must_use]
pub fn face_registered(family_name: &str) -> bool {
    let mut guard = font_system()
        .write()
        .expect("the shared iced font system lock is not poisoned");
    face_id(guard.raw(), family_name).is_some()
}

/// Whether the `Font::with_name(family_name)` handle actually reaches a face
/// whose family is `family_name`, rather than falling through to the
/// platform fallback chain.
///
/// This is the shaping-path question, not a cmap lookup. cosmic-text's
/// `FontSystem::get_font_supported_codepoints_in_word` cannot answer it: it
/// reads `Font::unicode_codepoints`, which is empty for every face not
/// registered as a monospace fallback
/// (`cosmic-text-0.15.0/src/font/mod.rs:82-86`), so it reports "supports
/// nothing" for ordinary faces. Shaping through the real chain is the only
/// honest probe.
#[must_use]
pub fn handle_resolves_to_family(family_name: &'static str, ch: char) -> bool {
    let resolution = resolve(Font::with_name(family_name), PROBE_LABEL, ch);
    resolution.resolved && resolution.face.eq_ignore_ascii_case(family_name)
}

/// The database id of the first face whose family name matches
/// `family_name`, if any.
fn face_id(system: &cosmic_text::FontSystem, family_name: &str) -> Option<cosmic_text::fontdb::ID> {
    system
        .db()
        .faces()
        .find(|face| {
            face.families
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case(family_name))
        })
        .map(|face| face.id)
}

/// The first laid-out glyph's face id and glyph index, if the codepoint
/// produced anything at all.
fn first_glyph(
    system: &mut cosmic_text::FontSystem,
    font: Font,
    ch: char,
) -> Option<(cosmic_text::fontdb::ID, u16)> {
    let mut buffer =
        cosmic_text::Buffer::new_empty(cosmic_text::Metrics::new(PROBE_SIZE, PROBE_LINE_HEIGHT));
    buffer.set_wrap(system, cosmic_text::Wrap::None);
    buffer.set_text(
        system,
        &ch.to_string(),
        &to_attributes(font),
        cosmic_text::Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(system, false);

    buffer
        .layout_runs()
        .flat_map(|run| run.glyphs.iter())
        .map(|glyph| (glyph.font_id, glyph.glyph_id))
        .next()
}

/// The family and PostScript names recorded for `id` in the font database.
fn face_names(system: &cosmic_text::FontSystem, id: cosmic_text::fontdb::ID) -> (String, String) {
    match system.db().face(id) {
        Some(face) => {
            let family = face
                .families
                .first()
                .map(|(name, _)| name.clone())
                .unwrap_or_else(|| String::from("<unnamed>"));
            (family, face.post_script_name.clone())
        }
        None => (String::from("<unknown face>"), String::from("<unknown>")),
    }
}

#[cfg(test)]
#[path = "glyph_resolution_tests.rs"]
mod tests;
