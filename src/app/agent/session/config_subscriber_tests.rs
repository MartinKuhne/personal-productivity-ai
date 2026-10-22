use super::*;
use crate::agent::tools::registry::ToolRegistry;
use crate::bus::events::typed::{BackgroundEvent, BackgroundEventSender, ProcessEvent};

#[tokio::test]
async fn test_config_subscriber_falls_back_to_default_on_timeout() {
    // Publish nothing. The subscriber must time out waiting for the first
    // ConfigArrived event, fall back to the default configuration, and still
    // perform MCP startup initialization. Reaching the startup-complete log
    // is only possible via the timeout branch (nothing was ever published).
    //
    // The window is injected at 100 ms rather than left at the production
    // [`CONFIG_ARRIVAL_TIMEOUT`] (10 s) so this step stays a fast test.
    let bus = crate::bus::config::config_bus();
    let (tx, mut rx) = tokio::sync::broadcast::channel::<BackgroundEvent>(16);

    let tm = Arc::new(arc_swap::ArcSwap::from_pointee(
        crate::agent::AgentToolContext::new(ToolRegistry::new()),
    ));
    spawn_config_subscription_with_timeout(
        tm,
        bus.clone(),
        BackgroundEventSender::new(tx),
        std::time::Duration::from_millis(100),
    );

    let mut timeout = tokio::time::interval(std::time::Duration::from_millis(100));
    let start = std::time::Instant::now();
    while start.elapsed().as_secs() < 5 {
        tokio::select! {
            Ok(event) = rx.recv() => {
                if let BackgroundEvent::Process(ProcessEvent::LogEntry(entry)) = event {
                    assert!(
                        entry
                            .message
                            .starts_with("MCP startup initialization complete"),
                        "unexpected log entry: {}",
                        entry.message
                    );
                    return;
                }
            }
            _ = timeout.tick() => {
                if start.elapsed().as_secs() >= 5 {
                    break;
                }
            }
        }
    }
    panic!("no timeout-fallback MCP startup log entry observed within timeout");
}

#[tokio::test]
async fn test_spawn_config_subscription_runs_init_in_background() {
    let bus = crate::bus::config::config_bus();
    let (tx, mut rx) = tokio::sync::broadcast::channel::<BackgroundEvent>(16);

    let tm = Arc::new(arc_swap::ArcSwap::from_pointee(
        crate::agent::AgentToolContext::new(ToolRegistry::new()),
    ));
    spawn_config_subscription(tm, bus.clone(), BackgroundEventSender::new(tx));

    bus.publish(ConfigArrived::new(crate::config::AppConfig::default()));

    let mut timeout = tokio::time::interval(std::time::Duration::from_millis(100));
    let start = std::time::Instant::now();
    while start.elapsed().as_secs() < 5 {
        tokio::select! {
            Ok(event) = rx.recv() => {
                if let BackgroundEvent::Process(ProcessEvent::LogEntry(entry)) = event {
                    assert!(
                        entry
                            .message
                            .starts_with("MCP startup initialization complete"),
                        "unexpected log entry: {}",
                        entry.message
                    );
                    return;
                }
            }
            _ = timeout.tick() => {
                if start.elapsed().as_secs() >= 5 {
                    break;
                }
            }
        }
    }
    panic!("no MCP startup log entry observed within timeout");
}
