//! Font configuration and fallback loading for the FastMD user interface.
//!
//! Unit tests live in the sibling `fonts_tests.rs` sidecar.

use eframe::egui;

/// Font identifier registered in [`egui::FontDefinitions`] for system symbol fallback.
pub const FALLBACK_FONT_NAME: &str = "fallback_symbols";

/// Candidate system font paths to search for symbol, arrow, and math coverage.
pub fn candidate_fallback_font_paths() -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();

    // 1. Explicit user/testing environment variable override
    if let Ok(env_path) = std::env::var("FASTMD_FALLBACK_FONT") {
        paths.push(std::path::PathBuf::from(env_path));
    }

    // 2. Windows standard font directories
    #[cfg(windows)]
    {
        let system_root = std::env::var("SYSTEMROOT").unwrap_or_else(|_| "C:\\Windows".to_string());
        let fonts_dir = std::path::Path::new(&system_root).join("Fonts");
        // Segoe UI Symbol has broad coverage of Unicode arrows, math, box drawing, and symbols
        paths.push(fonts_dir.join("seguisym.ttf"));
        paths.push(fonts_dir.join("segoeui.ttf"));
        paths.push(fonts_dir.join("arial.ttf"));
    }

    // 3. macOS standard font directories
    #[cfg(target_os = "macos")]
    {
        paths.push(std::path::PathBuf::from(
            "/System/Library/Fonts/Apple Symbols.ttf",
        ));
        paths.push(std::path::PathBuf::from("/System/Library/Fonts/SFNS.ttf"));
        paths.push(std::path::PathBuf::from(
            "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
        ));
        paths.push(std::path::PathBuf::from("/Library/Fonts/Arial Unicode.ttf"));
    }

    // 4. Linux and other Unix-like standard font directories
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        paths.push(std::path::PathBuf::from(
            "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        ));
        paths.push(std::path::PathBuf::from(
            "/usr/share/fonts/dejavu/DejaVuSans.ttf",
        ));
        paths.push(std::path::PathBuf::from(
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ));
        paths.push(std::path::PathBuf::from(
            "/usr/share/fonts/truetype/noto/NotoSansSymbols-Regular.ttf",
        ));
        paths.push(std::path::PathBuf::from(
            "/usr/share/fonts/truetype/noto/NotoSansSymbols2-Regular.ttf",
        ));
        paths.push(std::path::PathBuf::from(
            "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
        ));
        paths.push(std::path::PathBuf::from(
            "/usr/local/share/fonts/DejaVuSans.ttf",
        ));
    }

    paths
}

/// Discovers and reads the first available fallback font from the candidate paths.
pub fn load_fallback_font() -> Option<Vec<u8>> {
    for path in candidate_fallback_font_paths() {
        if let Ok(bytes) = std::fs::read(&path) {
            return Some(bytes);
        }
    }
    None
}

/// Builds [`egui::FontDefinitions`] configured with default typography,
/// the Phosphor icon font, and fallback symbols.
pub fn build_font_definitions() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    let font_bytes = egui_phosphor::Variant::Regular.font_bytes();
    fonts.font_data.insert(
        "phosphor".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(font_bytes)),
    );
    if let Some(font_keys) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        font_keys.insert(1, "phosphor".to_owned());
    }

    if let Some(data) = load_fallback_font() {
        fonts.font_data.insert(
            FALLBACK_FONT_NAME.to_owned(),
            std::sync::Arc::new(egui::FontData::from_owned(data)),
        );
        if let Some(font_keys) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
            font_keys.insert(2, FALLBACK_FONT_NAME.to_owned());
        }
        if let Some(font_keys) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
            font_keys.insert(1, FALLBACK_FONT_NAME.to_owned());
        }
    }

    fonts
}

/// Applies font configuration to an [`egui::Context`].
pub fn configure_fonts(ctx: &egui::Context) {
    ctx.set_fonts(build_font_definitions());
}

/// Checks whether the proportional font can render the given character without
/// falling back to a replacement tofu square (`'◻'`) or question mark (`'?'`).
pub fn is_glyph_supported(ctx: &egui::Context, c: char) -> bool {
    let mut supported = false;
    ctx.fonts_mut(|f| {
        let galley = f.layout_no_wrap(
            c.to_string(),
            egui::FontId::proportional(14.0),
            egui::Color32::WHITE,
        );
        supported = !galley.rows.is_empty()
            && galley
                .rows
                .iter()
                .flat_map(|r| &r.glyphs)
                .all(|g| g.chr != '◻' && g.chr != '?');
    });
    supported
}

/// Checks whether the configured proportional fonts can render all characters in
/// the string without falling back to replacement characters.
pub fn are_glyphs_supported(ctx: &egui::Context, s: &str) -> bool {
    s.chars().all(|c| is_glyph_supported(ctx, c))
}

/// Checks whether the monospace font can render the given character without
/// falling back to a replacement character.
pub fn is_monospace_glyph_supported(ctx: &egui::Context, c: char) -> bool {
    let mut supported = false;
    ctx.fonts_mut(|f| {
        let galley = f.layout_no_wrap(
            c.to_string(),
            egui::FontId::monospace(14.0),
            egui::Color32::WHITE,
        );
        supported = !galley.rows.is_empty()
            && galley
                .rows
                .iter()
                .flat_map(|r| &r.glyphs)
                .all(|g| g.chr != '◻' && g.chr != '?');
    });
    supported
}

#[cfg(test)]
#[path = "fonts_tests.rs"]
mod tests;
