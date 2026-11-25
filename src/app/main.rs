#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! FastMd desktop application entry point — initialises tracing, panic hooks and
//! the crypto provider, then launches the **iced** UI.
//!
//! `cargo run` (the `fastmd` bin) boots through
//! [`fastmd::ui_iced::app::launch`]. No feature flag gates the UI: iced is a
//! regular dependency, and the framework-free `ui/` tree supplies shared,
//! renderer-agnostic state.
//!
//! Startup ownership:
//!
//! | Step | Lives in |
//! | --- | --- |
//! | Panic hook (structured `ERROR` + backtrace, [NFR-004]/[NFR-005]) | this file |
//! | Non-blocking background logging ([NFR-002]) | this file |
//! | `rustls` ring crypto-provider install | this file |
//! | Optional Discord bot (`discord` feature) | this file |
//! | Config load + storage-handler injection ([RUST-024]) | this file |
//! | CLI workspace-argument injection | [`fastmd::ui_iced::app::boot`] |
//! | `ConfigArrived` publish | [`fastmd::ui_iced::app::build_state`] — synchronous |
//! | Title / icon / persisted geometry, theme, scale chain, bus bridge | [`fastmd::ui_iced::app::launch`] |
//!
//! `boot` constructs every subscriber before it returns the `Task::perform`
//! that publishes `ConfigArrived`, so every subscriber is in place before the
//! first arrival is emitted.
//!
//! Unit tests live in the sibling `main_tests.rs` sidecar.

use std::path::PathBuf;
use std::sync::Arc;

use fastmd::config::{ConfigStorageHandler, FileConfigStorage};
#[cfg(feature = "discord")]
use fastmd::integrations::discord::run_discord_bot;
use fastmd::ui_iced::app::{self, LaunchInputs};
use fastmd::ui_iced::persisted::{FileUiStateStorage, UiStateStorage};

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

/// Where the shipping binary keeps the persisted iced UI-state snapshot.
///
/// The *decision* is kept apart from the storage construction so the
/// persistence-location policy is unit-testable without touching the
/// filesystem or tripping the platform-default guard in
/// [`FileUiStateStorage::platform_default`] ([RUST-006]).
#[derive(Debug, PartialEq, Eq)]
enum UiStateSource {
    /// `FASTMD_ICED_UI_STATE_PATH` names an explicit scratch file.
    Override(PathBuf),
    /// No override — the platform-default `%APPDATA%\fastmd\ui_state.json`
    /// (or `~/.fastmd/ui_state.json`).
    PlatformDefault,
}

/// Decide where the UI-state snapshot lives from the raw environment override.
///
/// A missing or empty value means "no override", matching
/// [`app::ui_state_storage_from_env`]'s rule for what counts as an override.
fn resolve_ui_state_source(env_override: Option<String>) -> UiStateSource {
    match env_override.filter(|path| !path.is_empty()) {
        Some(path) => UiStateSource::Override(PathBuf::from(path)),
        None => UiStateSource::PlatformDefault,
    }
}

/// Build the injected [`UiStateStorage`] for a resolved [`UiStateSource`].
///
/// The platform-default branch resolves the user location only here, at
/// production launch — never inside a test ([RUST-024], [RUST-006]).
fn ui_state_storage_for(source: UiStateSource) -> Arc<dyn UiStateStorage> {
    match source {
        UiStateSource::Override(path) => Arc::new(FileUiStateStorage::new(path)),
        UiStateSource::PlatformDefault => Arc::new(FileUiStateStorage::platform_default()),
    }
}

fn main() -> iced::Result {
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

    // Initialize non-blocking background logging framework.
    // Writes to `%APPDATA%\fastmd\logs\background-process.log` (or `FASTMD_LOG_DIR`)
    // asynchronously via a dedicated worker thread, while outputting to stdout/stderr
    // with `FmtSpan::CLOSE` profiling and bridging to the in-memory UI panel.
    // The guard MUST stay alive for the whole run — `launch()` blocks until the
    // iced runtime exits, so dropping it here would shut logging down mid-session.
    let log_dir = fastmd::background::logs::get_log_dir();
    let _log_guard = fastmd::background::logs::init_logging(log_dir).ok();

    // Install rustls crypto provider
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    let config = fastmd::config::load_config();

    // The configuration-arrival bus is the fan-out channel that
    // each subsystem (background workers, agent, UI) subscribes to.
    // The UI does not wait on it — `boot` injects the config directly —
    // but the background workers still need the handle so
    // `boot` can publish the first arrival after they are constructed.
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

    // CLI workspace injection is applied inside `boot`; the
    // `ConfigArrived` publish happens synchronously inside
    // `build_state`. This entry only hands over the loaded config plus the
    // injected storage handlers ([RUST-024]) and the raw process arguments.
    let inputs = LaunchInputs {
        config,
        config_bus,
        config_storage: Arc::new(FileConfigStorage::new(fastmd::config::get_config_path()))
            as Arc<dyn ConfigStorageHandler>,
        ui_state_storage: ui_state_storage_for(resolve_ui_state_source(
            std::env::var(app::UI_STATE_PATH_ENV).ok(),
        )),
        args: std::env::args().collect(),
    };

    app::launch(inputs)
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
