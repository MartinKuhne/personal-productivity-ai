//! Shared decoding of the embedded FastMD application icon asset.
//!
//! The window bootstrap consumes the same `assets/icon-32.png` asset through
//! [`APP_ICON_PNG`] and [`decode_app_icon_rgba`], feeding
//! `iced::window::Settings::icon` via `iced::window::icon::from_rgba`.
//! Centralising the embed + decode here keeps the asset compiled in exactly
//! once ([RUST-021]/[RUST-023]).
//!
//! Unit tests live in the sibling `app_icon_tests.rs` sidecar.

/// Raw bytes of the compiled-in 32px FastMD window icon.
///
/// 32px is a good default for winit; Windows .ico embedding is handled separately via `build.rs`.
pub const APP_ICON_PNG: &[u8] = include_bytes!("../../../assets/icon-32.png");

/// Decodes the embedded application icon into straight RGBA pixels.
///
/// Returns `(rgba, width, height)` — unmultiplied-alpha RGBA, directly
/// consumable by `iced::window::icon::from_rgba`. Falls back to `None` if
/// decoding fails — never panics in production ([RUST-020]).
pub fn decode_app_icon_rgba() -> Option<(Vec<u8>, u32, u32)> {
    let img = image::load_from_memory(APP_ICON_PNG).ok()?;
    let rgba = img.to_rgba8();
    let (width, height) = (rgba.width(), rgba.height());
    Some((rgba.into_raw(), width, height))
}

#[cfg(test)]
#[path = "app_icon_tests.rs"]
mod tests;
