//! Background subscription that initializes and refreshes MCP tools on configuration arrivals.

use crate::agent::AgentToolContext;
use crate::app::background::{BackgroundLogEntry, LogCategory};
use crate::bus::config::CONFIG_ARRIVAL_TIMEOUT;
use crate::bus::core::Bus;
use crate::bus::events::config::ConfigArrived;
use std::sync::Arc;

/// Spawns a background thread that listens for [`ConfigArrived`] events on `config_bus`,
/// performs one-time MCP initialization on startup, and refreshes MCP tools on subsequent
/// configuration updates.
pub fn spawn_config_subscription(
    tool_context: Arc<arc_swap::ArcSwap<AgentToolContext>>,
    config_bus: Bus<ConfigArrived>,
    tx: crate::bus::events::typed::BackgroundEventSender,
) {
    let config_reader = config_bus.subscribe();
    std::thread::spawn(move || {
        let config = match config_reader.recv_timeout(CONFIG_ARRIVAL_TIMEOUT) {
            Ok(event) => event.config,
            Err(_) => {
                tracing::error!(
                    name = "config.arrived.timeout",
                    timeout_ms = CONFIG_ARRIVAL_TIMEOUT.as_millis() as u64,
                    "No ConfigArrived event observed within timeout; using default configuration"
                );
                crate::config::AppConfig::default()
            }
        };
        let agent_config = config.to_agent_config();
        tool_context.rcu(|ctx| {
            let mut new_ctx = (**ctx).clone();
            new_ctx.registry.init_mcp_on_startup(&agent_config);
            new_ctx
        });
        let _ = tx.send(
            BackgroundLogEntry::new(
                LogCategory::Indexer,
                "MCP startup initialization complete".to_string(),
            )
            .into(),
        );

        loop {
            if let Ok(event) = config_reader.recv() {
                let agent_config = event.config.to_agent_config();
                tool_context.rcu(|ctx| {
                    let mut new_ctx = (**ctx).clone();
                    new_ctx.registry.refresh_mcp_tools(&agent_config);
                    new_ctx
                });
            }
        }
    });
}

#[cfg(test)]
#[path = "config_subscriber_tests.rs"]
mod tests;
