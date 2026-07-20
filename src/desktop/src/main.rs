#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use fastmd::ui::FastMdApp;
use eframe::egui;

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> eframe::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Install rustls crypto provider
    rustls::crypto::ring::default_provider().install_default().ok();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_title("⚡ FastMD Viewer"),
        ..Default::default()
    };

    let config = fastmd::config::load_config();

    let prompt = fastmd::agent::get_base_system_prompt(&config);
    tracing::info!(
        name = "app.startup",
        system_prompt = %prompt,
        "Application started successfully. Emitted system prompt for diagnostics."
    );

    eframe::run_native(
        "fastmd",
        options,
        Box::new(move |cc| Box::new(FastMdApp::new(cc, config))),
    )
}