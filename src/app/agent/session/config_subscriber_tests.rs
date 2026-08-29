use super::*;
use crate::agent::tools::registry::ToolRegistry;
use crate::bus::events::typed::{BackgroundEvent, BackgroundEventSender, ProcessEvent};

#[test]
fn test_config_subscriber_falls_back_to_default_on_timeout() {
    // Publish nothing. The subscriber must time out waiting for the first
    // ConfigArrived event, fall back to the default configuration, and still
    // perform MCP startup initialization. Reaching the startup-complete log
    // is only possible via the timeout branch (nothing was ever published).
    let bus = crate::bus::config::config_bus();
    let (tx, rx) = std::sync::mpsc::channel::<BackgroundEvent>();

    let tm = Arc::new(arc_swap::ArcSwap::from_pointee(
        crate::agent::AgentToolContext::new(ToolRegistry::new()),
    ));
    spawn_config_subscription(tm, bus.clone(), BackgroundEventSender::new(tx));

    let start = std::time::Instant::now();
    while start.elapsed().as_secs() < 5 {
        match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(BackgroundEvent::Process(ProcessEvent::LogEntry(entry))) => {
                assert!(
                    entry
                        .message
                        .starts_with("MCP startup initialization complete"),
                    "unexpected log entry: {}",
                    entry.message
                );
                return;
            }
            Ok(_) | Err(_) => continue,
        }
    }
    panic!("no timeout-fallback MCP startup log entry observed within timeout");
}

#[test]
fn test_spawn_config_subscription_runs_init_in_background() {
    let bus = crate::bus::config::config_bus();
    let (tx, rx) = std::sync::mpsc::channel::<BackgroundEvent>();

    let tm = Arc::new(arc_swap::ArcSwap::from_pointee(
        crate::agent::AgentToolContext::new(ToolRegistry::new()),
    ));
    spawn_config_subscription(tm, bus.clone(), BackgroundEventSender::new(tx));

    bus.publish(ConfigArrived::new(crate::config::AppConfig::default()));

    let start = std::time::Instant::now();
    while start.elapsed().as_secs() < 5 {
        match rx.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(BackgroundEvent::Process(ProcessEvent::LogEntry(entry))) => {
                assert!(
                    entry
                        .message
                        .starts_with("MCP startup initialization complete"),
                    "unexpected log entry: {}",
                    entry.message
                );
                return;
            }
            Ok(_) | Err(_) => continue,
        }
    }
    panic!("no MCP startup log entry observed within timeout");
}
