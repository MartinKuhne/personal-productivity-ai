//! Unit tests for the shared application-icon loader ([RUST-006]: embedded
//! asset only — no filesystem access, no window or process spawn).

use super::*;

/// The PNG file signature of every valid PNG stream.
const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

#[test]
fn test_embedded_asset_is_a_png() {
    assert!(
        !APP_ICON_PNG.is_empty(),
        "embedded icon asset must not be empty"
    );
    assert!(
        APP_ICON_PNG.len() >= PNG_MAGIC.len() && &APP_ICON_PNG[..PNG_MAGIC.len()] == PNG_MAGIC,
        "embedded icon asset must start with the PNG file signature"
    );
}

#[test]
fn test_decoded_icon_is_nonempty_rgba() {
    let (rgba, width, height) =
        decode_app_icon_rgba().expect("the embedded 32px PNG icon must decode successfully");
    assert!(
        width > 0 && height > 0,
        "decoded icon must have a real size"
    );
    assert!(
        width == height,
        "the FastMD window icon is square (32px); got {width}x{height}"
    );
    assert_eq!(
        rgba.len(),
        4 * width as usize * height as usize,
        "RGBA pixel buffer must hold exactly 4 bytes per pixel"
    );
}
