//! Crate facade. Re-exports the public API and provides the Android entry
//! point.
//!
//! Public surface:
//! - [`app::FastMdApp`] — the `eframe::App` implementation.
//! - [`config::AuthConfig`] — bundled MSAL-style config.
//! - [`file_node::FileNode`] / [`file_node::FileTreeProcessor`] — tree
//!   model and markdown filter (mirrors the Kotlin app's `FileNode.kt`).
//! - [`error::AppError`] / [`error::AppResult`] — crate-wide error type.
//! - [`onedrive::OneDriveClient`] — Microsoft Graph client.
//! - [`auth`] — OAuth 2.0 PKCE flow modules (hand-rolled; see module docs).
//!
//! Everything else is private. The `ui/` module is a sibling of the data
//! model so widgets can be swapped without touching the rest of the crate.

#![cfg_attr(target_os = "android", no_main)]

pub mod android;
pub mod app;
pub mod auth;
pub mod config;
pub mod error;
pub mod file_node;
pub mod onedrive;
pub mod ui;

pub use app::FastMdApp;
pub use config::AuthConfig;
pub use error::{AppError, AppResult};
pub use file_node::{FileNode, FileTreeProcessor};
pub use onedrive::OneDriveClient;

// ---------------------------------------------------------------------
// Android entry point
// ---------------------------------------------------------------------
//
// `cargo apk` packages this crate as a cdylib. The `android_main` symbol
// is the entry point that the generated `NativeActivity` calls. The
// eframe `App` is constructed here and handed to `eframe::run_native`,
// which takes ownership of the main thread and drives the event loop
// until the activity is destroyed.
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: winit::platform::android::activity::AndroidApp) {
    // Bridge `log` (used by eframe, winit, etc.) to logcat. Keep the level
    // modest so the logcat buffer doesn't fill up in normal use; bump it
    // with `RUST_LOG=fastmd_android_egui=debug` when debugging.
    let _ = android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("fastmd-egui"),
    );

    // Stash the AndroidApp in a `OnceLock` so the App struct can poll the
    // activity's intent for the `msauth://` redirect. Must happen before
    // `run_native` starts, which never returns.
    crate::android::install(app);

    let options = eframe::NativeOptions {
        android_app: Some(android_app_handle()),
        ..Default::default()
    };
    if let Err(e) = eframe::run_native(
        "FastMD egui",
        options,
        Box::new(|cc| Ok(Box::new(FastMdApp::new(cc)))),
    ) {
        tracing::error!("eframe::run_native failed: {e}");
    }
}

#[cfg(target_os = "android")]
fn android_app_handle() -> winit::platform::android::activity::AndroidApp {
    // The `install()` above stashed the AndroidApp; cloning it back out
    // is required because eframe takes ownership.
    crate::android::android_app()
        .expect("AndroidApp should be installed before run_native")
        .clone()
}

// ---------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------
// The Kotlin/Compose app's regression suite (`AppTest.kt`) lives as an
// integration test under `tests/file_tree_processor.rs`. Keeping them out
// of `lib.rs` means they only depend on the public API, which is the
// discipline the rest of the repo follows.
