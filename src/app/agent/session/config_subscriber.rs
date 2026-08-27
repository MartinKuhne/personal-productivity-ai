//! Background subscription that initializes and refreshes MCP tools on configuration arrivals.

use crate::agent::AgentToolContext;
use crate::background::{BackgroundLogEntry, LogCategory};
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
    let mut config_reader = config_bus.subscribe_async();
    let task = async move {
        let config = match tokio::time::timeout(CONFIG_ARRIVAL_TIMEOUT, config_reader.recv()).await
        {
            Ok(Ok(event)) => event.config,
            _ => {
                tracing::error!(
                    name = "config.arrived.timeout",
                    timeout_ms = CONFIG_ARRIVAL_TIMEOUT.as_millis() as u64,
                    "No ConfigArrived event observed within timeout; using default configuration"
                );
                crate::config::AppConfig::default()
            }
        };
        let agent_config = config.to_agent_config();
        {
            let current = tool_context.load_full();
            let mut new_ctx = (*current).clone();
            // Store placeholders immediately before blocking on discovery
            tool_context.store(Arc::new(new_ctx.clone()));
            
            new_ctx.registry.init_mcp_on_startup(&agent_config).await;
            tool_context.store(Arc::new(new_ctx));
        }
        let _ = tx.send(
            BackgroundLogEntry::new(
                LogCategory::Indexer,
                "MCP startup initialization complete".to_string(),
            )
            .into(),
        );

        loop {
            let event = match config_reader.recv().await {
                Ok(e) => e,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    tracing::warn!("ConfigSubscriber lagged, dropping some config updates");
                    continue;
                }
                Err(_) => break, // Channel closed
            };

            let mut latest_event = event;
            // Drain any pending config updates so we only perform expensive discovery
            // on the most recent configuration state.
            loop {
                match config_reader.try_recv() {
                    Ok(next) => latest_event = next,
                    Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
                    Err(tokio::sync::broadcast::error::TryRecvError::Closed) => break,
                    Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {
                        tracing::warn!("ConfigSubscriber lagged during drain");
                        // We continue draining to get the latest available event
                        continue;
                    }
                }
            }
            
            let agent_config = latest_event.config.to_agent_config();
            let current = tool_context.load_full();
            let mut new_ctx = (*current).clone();
            
            tool_context.store(Arc::new(new_ctx.clone()));

            new_ctx.registry.refresh_mcp_tools(&agent_config).await;
            tool_context.store(Arc::new(new_ctx));
        }
    };

    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(task);
    } else {
        std::thread::spawn(move || {
            if let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                rt.block_on(task);
            }
        });
    }
}

#[cfg(test)]
#[path = "config_subscriber_tests.rs"]
mod tests;
