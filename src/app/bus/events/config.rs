//! Configuration-arrival event — the payload published on the
//! `Bus<ConfigArrived>` channel exactly once at startup.

use crate::config::AppConfig;

/// The single event published to the configuration bus. Carries the
/// loaded [`AppConfig`] by value so each subscriber can own a private
/// clone without going through a shared lock.
#[derive(Debug, Clone)]
pub struct ConfigArrived {
    /// The application configuration that has just been loaded.
    pub config: AppConfig,
}

impl ConfigArrived {
    /// Build a `ConfigArrived` event from a freshly loaded configuration.
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }
}
