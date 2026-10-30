//! System symbol-font fallback for the demo, ported from `main`'s
//! `src/app/ui/fonts.rs`.
//!
//! The demo asks iced for *generic* families — `hoarfrost`'s
//! [`Family::Sans`](hoarfrost::token::typography::Family::Sans) maps to
//! `iced::font::Family::SansSerif` and `Family::Mono` to
//! `iced::font::Family::Monospace` — so which physical face actually
//! draws a `✓` or a `⚠` is decided by the host, not by the demo. This
//! module narrows that by putting a known symbol-rich face into iced's
//! font database before the first frame is shaped, the same way `main`
//! does for the egui application.
//!
//! # What was ported, and what deliberately was not
//!
//! * [`candidate_fallback_font_paths`] and [`load_fallback_font`] are a
//!   faithful port of `main`'s discovery order, including the
//!   environment-variable override. The override is renamed
//!   [`FALLBACK_FONT_ENV`] (`HOARFROST_DEMO_FALLBACK_FONT`) rather than
//!   reusing `FASTMD_FALLBACK_FONT`: the two binaries run side by side
//!   during the port, and sharing one variable would make a value set for
//!   the egui app silently redirect the demo.
//! * **Phosphor is not ported.** It is 98.1% Private-Use-Area and
//!   contains none of the codepoints hoarfrost emits, so registering it
//!   would add weight and change nothing.
//!
//! # What the port can and cannot buy under iced
//!
//! `main` needed this because egui ships only its own embedded fonts and
//! never looks at the operating system. iced's text backend is
//! cosmic-text, which *does* call `fontdb::Database::load_system_fonts`
//! for every `std` target when its `FontSystem` is constructed
//! (`cosmic-text-0.15.0/src/font/system.rs:400`), and which carries its
//! own per-platform common-fallback list — on Windows that list is
//! `Segoe UI`, `Segoe UI Emoji`, `Segoe UI Symbol`, `Segoe UI Historic`.
//!
//! Measured on Windows, registering `seguisym.ttf` here added one face
//! to the database (518 → 519) and moved **none** of the fourteen
//! codepoints: the file was already in the database from the system scan,
//! and already reachable through the platform fallback list. The port is
//! therefore a coverage *guarantee* rather than a resolution *override*.
//! It earns its place on a stripped host — a container, a minimal Linux
//! install, a machine whose font scan found nothing — where the generic
//! families would otherwise have no symbol face to fall back to at all.
//!
//! It is not, and cannot be from this side of the boundary, a way to make
//! the same face appear on every operating system. That would mean
//! bundling the symbol font and naming it in the framework's
//! `Family::iced()` mapping instead of returning a generic family, which
//! is a `hoarfrost` change.
//!
//! Registration is idempotent and total: with no readable candidate, or
//! with a poisoned font-system lock, [`register`] logs a warning and does
//! nothing. It never panics.
//!
//! Unit tests live in the sibling `font_fallback_tests.rs` sidecar.

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use iced::advanced::graphics::text::font_system;

/// Environment variable that overrides discovery with one explicit font file.
///
/// Deliberately not `main`'s `FASTMD_FALLBACK_FONT` — see the module doc.
pub const FALLBACK_FONT_ENV: &str = "HOARFROST_DEMO_FALLBACK_FONT";

/// Environment variable that suppresses registration entirely.
///
/// Exists so the before/after glyph-resolution evidence in the
/// `glyph_resolution` probe can be reproduced in either state without
/// editing code.
pub const NO_FALLBACK_ENV: &str = "HOARFROST_DEMO_NO_FALLBACK";

/// Printed once when no candidate font could be loaded.
const NO_FALLBACK_WARNING: &str = "hoarfrost-demo: no symbol fallback font found; \
                                  symbol glyphs will resolve by host fallback only.";
/// Hint appended to [`NO_FALLBACK_WARNING`] naming the override variable.
const OVERRIDE_HINT: &str = "Set a .ttf against the override variable to pin one.";

/// Printed once when the shared font system could not be written to.
const LOCK_WARNING: &str = "hoarfrost-demo: iced's font system lock was unavailable; \
                           the symbol fallback was not registered.";

/// Windows font files tried in order, widest symbol coverage first.
///
/// `seguisym.ttf` is first on purpose: measured against the real files it
/// covers all fourteen of the codepoints hoarfrost emits, where
/// `segoeui.ttf` and `arial.ttf` each cover six.
#[cfg(windows)]
const WINDOWS_FONT_FILES: [&str; 3] = ["seguisym.ttf", "segoeui.ttf", "arial.ttf"];

/// macOS candidates, tried in order.
#[cfg(target_os = "macos")]
const MACOS_FONT_PATHS: [&str; 4] = [
    "/System/Library/Fonts/Apple Symbols.ttf",
    "/System/Library/Fonts/SFNS.ttf",
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    "/Library/Fonts/Arial Unicode.ttf",
];

/// Linux and other Unix candidates, tried in order.
#[cfg(all(unix, not(target_os = "macos")))]
const UNIX_FONT_PATHS: [&str; 7] = [
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/TTF/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansSymbols-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf",
    "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
    "/usr/local/share/fonts/DejaVuSans.ttf",
];

/// Every candidate fallback font path, in probe order.
///
/// An explicit [`FALLBACK_FONT_ENV`] value goes first, then the
/// platform's standard locations. Nothing here checks that a path exists
/// — [`load_fallback_font`] is what reads, and the first path that reads
/// successfully wins.
#[must_use]
pub fn candidate_fallback_font_paths() -> Vec<PathBuf> {
    let mut paths = override_candidate(std::env::var(FALLBACK_FONT_ENV).ok().as_deref());
    paths.extend(platform_candidates());
    paths
}

/// The candidate list an [`FALLBACK_FONT_ENV`] value produces.
///
/// Split out from [`candidate_fallback_font_paths`] so the override's
/// semantics — a non-empty value goes first, an unset or empty one adds
/// nothing — are testable without mutating the process environment.
#[must_use]
pub fn override_candidate(value: Option<&str>) -> Vec<PathBuf> {
    value
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .into_iter()
        .collect()
}

/// Whether a [`NO_FALLBACK_ENV`] value means "do not register".
///
/// Only the exact string `1` suppresses, matching the demo's existing
/// `HOARFROST_DEMO_GOLDEN` opt-in convention.
#[must_use]
pub fn suppression_requested(value: Option<&str>) -> bool {
    value == Some("1")
}

/// The platform's standard fallback font locations.
fn platform_candidates() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        windows_candidates()
    }
    #[cfg(target_os = "macos")]
    {
        MACOS_FONT_PATHS.iter().map(PathBuf::from).collect()
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        UNIX_FONT_PATHS.iter().map(PathBuf::from).collect()
    }
    #[cfg(not(any(windows, unix)))]
    {
        Vec::new()
    }
}

/// `%SYSTEMROOT%\Fonts\…`, falling back to `C:\Windows` when the
/// variable is unset.
#[cfg(windows)]
fn windows_candidates() -> Vec<PathBuf> {
    let system_root = std::env::var("SYSTEMROOT").unwrap_or_else(|_| String::from("C:\\Windows"));
    let fonts_dir = Path::new(&system_root).join("Fonts");
    WINDOWS_FONT_FILES
        .iter()
        .map(|file| fonts_dir.join(file))
        .collect()
}

/// The first path in `candidates` that reads, and the bytes it gave up.
///
/// Takes its candidates as an argument rather than reading them from the
/// environment and the platform itself, so the "no font anywhere" outcome
/// is reachable from a test without a fresh process.
///
/// A path that exists but cannot be read is skipped rather than fatal, so
/// one unreadable file in the list cannot hide a working font behind it.
#[must_use]
pub fn first_readable(candidates: &[PathBuf]) -> Option<(PathBuf, Vec<u8>)> {
    candidates
        .iter()
        .cloned()
        .find_map(|path| std::fs::read(&path).ok().map(|bytes| (path, bytes)))
}

/// The bytes of the first readable candidate, and where they came from.
///
/// Part of the ported surface; the demo's own registration goes through
/// [`register_from`], so a binary crate has no consumer for this beyond
/// the sidecar.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use]
pub fn load_fallback_font_with_path() -> Option<(PathBuf, Vec<u8>)> {
    first_readable(&candidate_fallback_font_paths())
}

/// The bytes of the first readable candidate fallback font.
///
/// Mirrors `main`'s `load_fallback_font`; [`load_fallback_font_with_path`]
/// carries the provenance this crate wants for its report. Kept as part of
/// the ported surface and exercised from the sidecar, which is the only
/// consumer a binary crate can have for it.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use]
pub fn load_fallback_font() -> Option<Vec<u8>> {
    load_fallback_font_with_path().map(|(_path, bytes)| bytes)
}

/// Loads the chosen fallback font into iced's shared font system.
///
/// Idempotent: the whole process registers at most once, and every later
/// call returns the same answer without touching the filesystem or the
/// font system again. That matters because iced's `load_font` only
/// de-duplicates *borrowed* bytes, and these bytes are owned.
///
/// Returns the path that was loaded, or `None` when registration was
/// suppressed ([`NO_FALLBACK_ENV`]) or no candidate was readable. A
/// `None` is logged, never raised.
pub fn register() -> Option<PathBuf> {
    REGISTERED.get_or_init(register_once).clone()
}

/// Registers the first readable entry in `candidates` into iced's font
/// system, bypassing the process-wide latch.
///
/// The candidate list is a parameter rather than something this function
/// discovers, so the "there is no font anywhere" branch is exercisable
/// from a test without a fresh process.
///
/// Returns the path that was loaded, or `None` when nothing readable was
/// given or the font system could not be written to. Never panics.
#[must_use]
pub fn register_from(candidates: &[PathBuf]) -> Option<PathBuf> {
    let (path, bytes) = first_readable(candidates)?;

    match font_system().write() {
        Ok(mut system) => {
            system.load_font(Cow::Owned(bytes));
            Some(path)
        }
        Err(_) => {
            eprintln!("{LOCK_WARNING}");
            None
        }
    }
}

/// The path [`register`] settled on, without forcing registration.
///
/// Read by the resolution report in the `glyph_resolution` probe; a
/// binary crate has no other consumer for it.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use]
pub fn registered_path() -> Option<PathBuf> {
    REGISTERED.get().cloned().flatten()
}

/// Whether [`register`] has already run, in either outcome.
///
/// Lets a test tell "registered and found nothing" apart from "never
/// attempted", which `registered_path` alone cannot.
#[cfg_attr(not(test), allow(dead_code))]
#[must_use]
pub fn registration_attempted() -> bool {
    REGISTERED.get().is_some()
}

static REGISTERED: OnceLock<Option<PathBuf>> = OnceLock::new();

/// The body behind [`register`], run at most once.
fn register_once() -> Option<PathBuf> {
    if suppression_requested(std::env::var(NO_FALLBACK_ENV).ok().as_deref()) {
        return None;
    }

    let path = register_from(&candidate_fallback_font_paths());
    if path.is_none() {
        eprintln!("{NO_FALLBACK_WARNING} {OVERRIDE_HINT} {FALLBACK_FONT_ENV}");
    }
    path
}

#[cfg(test)]
#[path = "font_fallback_tests.rs"]
mod tests;
