//! Iced pilot binary — now a thin shim over the shared [TASK-022] launch
//! path (`fastmd::ui_iced::app::launch`).
//!
//! The pilot previously carried its own inline `boot`/`update`/`view` and a
//! private `PilotState`. Those moved into `crate::ui_iced::{app,update,view}`
//! so `cargo run --bin fastmd_iced` now exercises the REAL launch chain
//! (egui-free state construction, DEC-008 scale chain, DEC-009 config
//! arrival, the [TASK-019] subscription bridge) that [TASK-024] will flip
//! `main.rs` onto. The window title is `strings::WINDOW_TITLE` ("FastMD AI",
//! DEC-010), the icon and persisted geometry come from the shared
//! [`fastmd::ui_iced::app::window_settings`], and the close/save flow lives
//! in [`fastmd::ui_iced::update`].
//!
//! Storage wiring ([RUST-024]): the configuration is loaded from the
//! platform-default location (the same `load_config` the egui `main.rs`
//! uses) and persisted through a `FileConfigStorage` at `get_config_path()`;
//! the UI-state snapshot resolves through
//! [`fastmd::ui_iced::app::ui_state_storage_from_env`] — the
//! `FASTMD_ICED_UI_STATE_PATH` override points it at an explicit scratch
//! file, otherwise it stays in-process. The pilot never writes the
//! platform-default UI-state location ([RUST-006]).
//!
//! The former pilot state/reduction tests moved to
//! `src/app/ui_iced/app_tests.rs` / `update_tests.rs` alongside the code
//! they now exercise.

use std::sync::Arc;

use fastmd::config::FileConfigStorage;
use fastmd::ui_iced::app::{self, LaunchInputs};

fn main() -> iced::Result {
    // Panic hook + non-blocking background logging, mirroring the egui
    // entry (`src/app/main.rs:16-39`) so the real launch path is observable.
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
    let log_dir = fastmd::background::logs::get_log_dir();
    let _log_guard = fastmd::background::logs::init_logging(log_dir).ok();

    // Install the rustls crypto provider (the agent's network path).
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    let inputs = LaunchInputs {
        config: fastmd::config::load_config(),
        config_bus: fastmd::bus::config::config_bus(),
        config_storage: Arc::new(FileConfigStorage::new(fastmd::config::get_config_path()))
            as Arc<dyn fastmd::config::ConfigStorageHandler>,
        ui_state_storage: app::ui_state_storage_from_env(
            std::env::var(fastmd::ui_iced::app::UI_STATE_PATH_ENV).ok(),
        ),
        args: std::env::args().collect(),
    };

    app::launch(inputs)
}
