#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! FastMd desktop application entry point — initialises tracing, panic hooks, and launches the egui app.

use eframe::egui;
#[cfg(feature = "discord")]
use fastmd::integrations::discord::run_discord_bot;
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

    // Initialize tracing.
    // `FmtSpan::CLOSE` prints `time.busy` (CPU) and `time.idle` (async wait)
    // for every span when it closes — enabling zero-overhead profiling of
    // instrumented hot paths via `RUST_LOG`.
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,fastmd=debug"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .init();

    // Install rustls crypto provider
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id("fastmd")
            .with_inner_size([1000.0, 700.0])
            .with_title("⚡ FastMD Viewer"),
        ..Default::default()
    };

    let mut config = fastmd::config::load_config();

    let args: Vec<String> = std::env::args().collect();
    if args.len() > 1 {
        let path = std::path::PathBuf::from(&args[1]);
        if path.exists() && path.is_dir() {
            let mut path_str = path
                .canonicalize()
                .unwrap_or(path)
                .to_string_lossy()
                .to_string();
            if path_str.starts_with(r"\\?\") {
                path_str = path_str[4..].to_string();
            }
            let found = config
                .content_libraries
                .iter()
                .any(|lib| lib.root_folder == path_str);
            if !found {
                config
                    .content_libraries
                    .push(fastmd::config::ContentLibrary {
                        root_folder: path_str,
                        name: "Workspace".to_string(),
                        kind: "text".to_string(),
                        readonly: false,
                        priority: 0,
                    });
            }
        }
    }

    if config.content_libraries.len() <= 1
        && config
            .content_libraries
            .iter()
            .all(|lib| lib.name == config.system_library_display_name())
    {
        let mut current_dir =
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        if let Ok(canon) = std::fs::canonicalize(&current_dir) {
            current_dir = canon;
        }
        let mut path_str = current_dir.to_string_lossy().to_string();
        if path_str.starts_with(r"\\?\") {
            path_str = path_str[4..].to_string();
        }
        config
            .content_libraries
            .push(fastmd::config::ContentLibrary {
                root_folder: path_str,
                name: "Workspace".to_string(),
                kind: "text".to_string(),
                readonly: false,
                priority: 0,
            });
    }

    // The configuration-arrival bus is the fan-out channel that
    // each subsystem (background workers, agent, UI) subscribes to
    // and uses to perform its own initialization work. We create it
    // here, hand it to the app, and publish the loaded config so
    // every subscriber observes the same first-arrival event.
    let config_bus = fastmd::bus::config::config_bus();

    // Start Discord bot if configured. The bot integration
    // lives behind the `discord` Cargo feature; without it
    // the `discord` config field is parsed but never
    // dispatched (and `run_discord_bot` doesn't exist).
    #[cfg(feature = "discord")]
    if let Some(discord_config) = config.discord.as_ref()
        && discord_config.bot_token.is_some()
    {
        let app_config = config.clone();
        tokio::spawn(async move {
            if let Err(e) = run_discord_bot(&app_config).await {
                tracing::error!(name = "discord.bot.error", error = %e, "Discord bot error");
            }
        });
    }

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
            // (`Task::new`, `AgentSession::new`, and the UI
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
