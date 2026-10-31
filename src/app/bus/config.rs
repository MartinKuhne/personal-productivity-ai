//! Configuration-arrival bus constructor and subscriber timeout.
//!
//! The [`ConfigArrived`] event payload lives in
//! `crate::bus::events::config`; this module owns only the
//! constructor and the timeout constant that polling subscribers use
//! when they can't block the UI thread.
//!
//! Unit tests live in the sibling `config_tests.rs` sidecar.

use std::time::Duration;

use crate::bus::core::Bus;
use crate::bus::events::config::ConfigArrived;

/// Construct a fresh configuration-arrival bus.
///
/// Cloning the returned [`Bus`] is cheap and shares the same broadcast
/// channel. The bus is intentionally created by `main` (which loads the
/// config) and handed to every subsystem that needs to subscribe.
pub fn config_bus() -> Bus<ConfigArrived> {
    Bus::new()
}

/// Default timeout for subscribers that poll the bus without blocking
/// the UI thread. If the event is not observed within this window the
/// subscriber falls back to [`crate::config::AppConfig::default`] and
/// emits a `config.arrived.timeout` tracing event.
///
/// The window is deliberately generous. Boot warm-up (font load, first
/// frame, wgpu initialisation) runs before the `Task::perform` that
/// publishes `ConfigArrived`, so arrival can be more than 1 s
/// wall-clock after a subscriber starts waiting. The publish itself is
/// synchronous inside [`crate::ui_iced::app::build_state`] once every
/// subscriber has registered, but the generous window is kept as
/// defense-in-depth: a future asynchronous publisher must not silently
/// degrade the initial scan. The fallback also keeps scanning legal
/// after a genuine mis-ordering instead of wedging the workers forever.
///
/// The timeout is only ever waited on by worker/init *threads* and the
/// agent config-subscriber thread — never on the UI thread — so the
/// generous window cannot block user input: a user action must be
/// accepted within one frame regardless of background load.
pub const CONFIG_ARRIVAL_TIMEOUT: Duration = Duration::from_secs(10);

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
