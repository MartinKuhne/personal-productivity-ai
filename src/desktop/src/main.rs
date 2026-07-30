#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! FastMd desktop application entry point â€” initialises tracing, panic hooks, and launches the egui app.

use eframe::egui;
use fastmd::ui::FastMdApp;

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> eframe::Result<()> {
    std::panic::set_hook(Box::new(|panic_info| {
        let location = panic_info.location().map(|l| l.to_string());
        let payload = panic_info.payload();
        let msg = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(|s| s.as_str()))
            .unwrap_or("<no message>");
        let backtrace = std::backtrace::Backtrace::force_capture();
        tracing::error!(
            name = "panic",
            location = ?location,
            message = %msg,
            backtrace = %backtrace,
            "Fatal panic"
        );
    }));

    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Install rustls crypto provider
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 700.0])
            .with_title("⚡ FastMD Viewer"),
        ..Default::default()
    };

    let config = fastmd::config::load_config();

    // The configuration-arrival bus is the fan-out channel that
    // each subsystem (background workers, agent, UI) subscribes to
    // and uses to perform its own initialization work. We create it
    // here, hand it to the app, and publish the loaded config so
    // every subscriber observes the same first-arrival event.
    let config_bus = fastmd::bus::config::config_bus();

    eframe::run_native(
        "fastmd",
        options,
        // egui 0.28+ wraps the `Box<dyn App>` returned by the
        // creation callback in a `Result`, so the app can fail
        // during setup instead of panicking. We don't currently
        // have any fallible setup work to do, so we always return
        // `Ok`.
        Box::new(move |cc| {
            // Build the app first so its internal subscribers
            // (`Task::new`, `AgentSessionManager::new`, and the UI
            // reader in `FastMdApp::new`) register *before* we
            // publish. `tokio::sync::broadcast` only delivers an
            // event to subscribers that exist at publish time —
            // publishing before construction would silently drop
            // the event on the floor and the background workers
            // would fall back to the default (empty) config and
            // never start scanning.
            let app = FastMdApp::new(cc, config_bus.clone());
            config_bus.publish(fastmd::ConfigArrived::new(config.clone()));
            Ok(Box::new(app))
        }),
    )
}
