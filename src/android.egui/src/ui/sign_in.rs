//! Sign-in screen. Port of the unauthenticated branch of `MainActivity.kt`.
//!
//! Renders a centered title and a "Sign In with OneDrive" button. The
//! click callback is supplied by the caller so the App can wire it to
//! the actual PKCE flow.

use eframe::egui::{self, Button, Color32, RichText};

/// Render the sign-in screen into `ui`. `on_sign_in` is invoked when the
/// user taps the button.
pub fn render(ui: &mut egui::Ui, on_sign_in: impl FnOnce()) {
    // Solid dark background panel that matches the App's outer theme.
    let rect = ui.available_rect_before_wrap();
    ui.painter()
        .rect_filled(rect, 0.0, Color32::from_rgb(0x1E, 0x1E, 0x22));

    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.35);
        ui.label(
            RichText::new("FastMD egui")
                .size(28.0)
                .strong()
                .color(Color32::from_rgb(0xE0, 0xE0, 0xE6)),
        );
        ui.add_space(8.0);
        ui.label(
            RichText::new("Rust + eframe OneDrive viewer")
                .size(14.0)
                .color(Color32::from_rgb(0xA0, 0xA0, 0xA8)),
        );
        ui.add_space(24.0);
        let btn = Button::new(
            RichText::new("Sign In with OneDrive")
                .size(16.0)
                .color(Color32::WHITE),
        )
        .fill(Color32::from_rgb(0x00, 0x78, 0xD4))
        .min_size(egui::vec2(220.0, 44.0))
        .corner_radius(6.0);
        if ui.add(btn).clicked() {
            on_sign_in();
        }
    });
}
