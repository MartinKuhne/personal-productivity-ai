#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use fastmd::ui::FastMdApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    // Install rustls crypto provider
    rustls::crypto::ring::default_provider().install_default().ok();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_title("⚡ FastMD Viewer"),
        ..Default::default()
    };

    eframe::run_native(
        "fastmd",
        options,
        Box::new(|cc| Box::new(FastMdApp::new(cc))),
    )
}