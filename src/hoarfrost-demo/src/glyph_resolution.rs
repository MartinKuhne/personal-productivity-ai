//! Glyph-resolution probe: which physical face each symbol the demo
//! renders actually lands on.
//!
//! The demo asks iced for generic families, so "did that `✓` come from
//! Segoe UI Symbol or from a tofu box" is not visible from any of the
//! demo's own types. This module asks the question at the only layer that
//! can answer it: it shapes a single codepoint through
//! [`iced`'s own `to_attributes`] over the process-wide
//! [`font_system`] — the same database, the same cosmic-text fallback
//! walk, the same `Shaping::Advanced` the widgets use — and reads the
//! `font_id` back off the laid-out glyph.
//!
//! A glyph with id `0` is `.notdef`, the `.notdef` glyph every TrueType
//! and CFF face carries, and is what the user sees as a tofu box. A codepoint
//! that produces no glyph at all is reported the same way.
//!
//! The report is what the fallback port is measured against: run it with
//! [`crate::font_fallback::NO_FALLBACK_ENV`] unset and again set to `1`
//! and the two tables are the before/after.
//!
//! Unit tests live in the sibling `glyph_resolution_tests.rs` sidecar.

use hoarfrost::token::typography::{Family, Weight};
use iced::Font;
use iced::advanced::graphics::text::{cosmic_text, font_system, to_attributes};

/// The symbol codepoints hoarfrost renders, as the demo renders them.
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

/// The two token families the probe resolves through, with their spec names.
pub const PROBED_FAMILIES: [(Family, &str); 2] = [
    (Family::Sans, "font.family.sans"),
    (Family::Mono, "font.family.mono"),
];

/// Text size the probe shapes at — the demo's `type.body` rung.
const PROBE_SIZE: f32 = 14.0;
/// Line height the probe shapes at, matching [`PROBE_SIZE`].
const PROBE_LINE_HEIGHT: f32 = 20.0;
/// Column width for the rendered face name in [`report`].
const FACE_COLUMN: usize = 28;

/// One codepoint's resolution outcome in one family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolution {
    /// The codepoint that was shaped.
    pub ch: char,
    /// The `font.family.*` token it was shaped under.
    pub token: &'static str,
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
    /// One report line: `U+2713 ✓ sans -> Segoe UI Symbol gid=305 ok`.
    #[must_use]
    pub fn line(&self) -> String {
        let name = format!("{} [{}]", self.face, self.post_script);
        let verdict = if self.resolved { "ok" } else { "TOFU" };
        format!(
            "U+{:04X} {} {:<5} -> {:<FACE_COLUMN$} gid={:<5} {}",
            u32::from(self.ch),
            self.ch,
            short_token(self.token),
            name,
            self.glyph_id,
            verdict
        )
    }
}

/// Resolves `ch` under `family` through the live iced font chain.
#[must_use]
pub fn resolve(family: Family, token: &'static str, ch: char) -> Resolution {
    let mut guard = font_system()
        .write()
        .expect("the demo's font system lock is not poisoned");
    let system = guard.raw();

    match first_glyph(system, family.font(Weight::Regular), ch) {
        Some((id, glyph_id)) => {
            let (face, post_script) = face_names(system, id);
            Resolution {
                ch,
                token,
                face,
                post_script,
                glyph_id,
                resolved: glyph_id != 0,
            }
        }
        None => Resolution {
            ch,
            token,
            face: String::from("<no glyph>"),
            post_script: String::from("<none>"),
            glyph_id: 0,
            resolved: false,
        },
    }
}

/// Every codepoint under every probed family, in declaration order.
#[must_use]
pub fn resolve_all() -> Vec<Resolution> {
    PROBED_FAMILIES
        .iter()
        .flat_map(|(family, token)| SYMBOL_CODEPOINTS.map(|ch| resolve(*family, token, ch)))
        .collect()
}

/// The whole probe rendered as a report.
///
/// The header names the fallback file that was actually registered, so a
/// reader can tell "the fallback is in the chain and changed nothing"
/// apart from "the fallback never loaded" without a second run.
#[must_use]
pub fn report() -> String {
    let resolutions = resolve_all();
    let ok = resolutions.iter().filter(|r| r.resolved).count();
    let fallback = crate::font_fallback::registered_path()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| String::from("<none registered>"));

    let mut out = format!(
        "glyph resolution: {ok}/{} resolved | fallback registered: {fallback}\n",
        resolutions.len()
    );
    for resolution in &resolutions {
        out.push_str(&resolution.line());
        out.push('\n');
    }
    out
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

/// The tail of a `font.family.*` token, for the narrow report column.
fn short_token(token: &'static str) -> &'static str {
    match token {
        "font.family.sans" => "sans",
        "font.family.mono" => "mono",
        other => other,
    }
}

#[cfg(test)]
#[path = "glyph_resolution_tests.rs"]
mod tests;
