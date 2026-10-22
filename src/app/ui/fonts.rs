//! Font configuration and fallback loading for the FastMD user interface.
//!
//! This module owns the framework-free font inventory: the embedded Phosphor
//! icon bytes and the system fallback-font discovery. The iced layer
//! ([`crate::ui_iced::fonts`]) registers these bytes with
//! `iced::font::load`.
//!
//! Unit tests live in the sibling `fonts_tests.rs` sidecar.

/// Font identifier used for the system symbol-fallback collection.
pub const FALLBACK_FONT_NAME: &str = "fallback_symbols";

/// Candidate system font paths to search for symbol, arrow, and math coverage.
#[must_use]
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
#[must_use]
pub fn load_fallback_font() -> Option<Vec<u8>> {
    for path in candidate_fallback_font_paths() {
        if let Ok(bytes) = std::fs::read(&path) {
            return Some(bytes);
        }
    }
    None
}

/// Embedded Phosphor Regular icon font bytes.
///
/// The iced runtime registers this byte collection, so it stays declared
/// here as the single font inventory ([RUST-023]).
pub const PHOSPHOR_REGULAR_FONT: &[u8] =
    include_bytes!("../../../assets/fonts/Phosphor-Regular.ttf");

#[cfg(test)]
#[path = "fonts_tests.rs"]
mod tests;
