use super::*;
use crate::agent::tools::registry::ToolRegistry;
use crate::bus::events::typed::{BackgroundEvent, BackgroundEventSender, ProcessEvent};

/// Consume background events until the MCP startup-complete entry arrives.
/// Bounded so a regression fails the test instead of hanging the suite.
async fn await_startup_complete(
    rx: &mut tokio::sync::broadcast::Receiver<BackgroundEvent>,
) -> bool {
    tokio::time::timeout(std::time::Duration::from_secs(5), async move {
        loop {
            match rx.recv().await {
                Ok(BackgroundEvent::Process(ProcessEvent::LogEntry(entry))) => {
                    if entry
                        .message
                        .starts_with("MCP startup initialization complete")
                    {
                        return true;
                    }
                }
                Ok(_) => continue,
                Err(_) => return false,
            }
        }
    })
    .await
    .unwrap_or(false)
}

/// Spin (cooperatively, no sleeping) until the subscriber installs a new
/// [`AgentToolContext`], reporting `false` if it never does.
async fn wait_for_context_change(
    tool_context: &arc_swap::ArcSwap<AgentToolContext>,
    before: &Arc<AgentToolContext>,
) -> bool {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if !Arc::ptr_eq(before, &tool_context.load_full()) {
                return true;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or(false)
}

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

#[tokio::test]
async fn test_config_subscriber_refreshes_tool_context_on_later_config_arrival() {
    // The startup path only stores the tool context twice (placeholder then
    // initialised registry). A *second* ConfigArrived is the only way to reach
    // the long-lived refresh loop, and the observable effect of that loop is a
    // freshly stored AgentToolContext.
    let bus = crate::bus::config::config_bus();
    let (tx, mut rx) = tokio::sync::broadcast::channel::<BackgroundEvent>(64);
    let tool_context = Arc::new(arc_swap::ArcSwap::from_pointee(
        crate::agent::AgentToolContext::new(ToolRegistry::new()),
    ));

    spawn_config_subscription_with_timeout(
        tool_context.clone(),
        bus.clone(),
        BackgroundEventSender::new(tx),
        std::time::Duration::from_millis(100),
    );
    bus.publish(ConfigArrived::new(crate::config::AppConfig::default()));

    assert!(
        await_startup_complete(&mut rx).await,
        "startup initialization must complete before the refresh loop is exercised"
    );

    let after_init = tool_context.load_full();
    bus.publish(ConfigArrived::new(crate::config::AppConfig::default()));

    assert!(
        wait_for_context_change(&tool_context, &after_init).await,
        "a second ConfigArrived must run the refresh loop and re-store the tool context"
    );
}

#[tokio::test]
async fn test_config_subscriber_survives_a_burst_of_config_arrivals() {
    // Several arrivals published back-to-back are collapsed by the drain loop
    // down to the newest one. The subscriber must neither panic nor stop
    // listening: a later arrival still has to be handled.
    let bus = crate::bus::config::config_bus();
    let (tx, mut rx) = tokio::sync::broadcast::channel::<BackgroundEvent>(64);
    let tool_context = Arc::new(arc_swap::ArcSwap::from_pointee(
        crate::agent::AgentToolContext::new(ToolRegistry::new()),
    ));

    spawn_config_subscription_with_timeout(
        tool_context.clone(),
        bus.clone(),
        BackgroundEventSender::new(tx),
        std::time::Duration::from_millis(100),
    );
    bus.publish(ConfigArrived::new(crate::config::AppConfig::default()));

    assert!(
        await_startup_complete(&mut rx).await,
        "startup initialization must complete before the burst is published"
    );

    let after_init = tool_context.load_full();
    for _ in 0..4 {
        bus.publish(ConfigArrived::new(crate::config::AppConfig::default()));
    }
    assert!(
        wait_for_context_change(&tool_context, &after_init).await,
        "the refresh loop must consume the burst and re-store the tool context"
    );

    // The loop must still be alive after the burst.
    let after_burst = tool_context.load_full();
    bus.publish(ConfigArrived::new(crate::config::AppConfig::default()));
    assert!(
        wait_for_context_change(&tool_context, &after_burst).await,
        "the subscriber must still handle arrivals after draining a burst"
    );
}

#[tokio::test]
async fn test_config_subscriber_replaces_the_initial_tool_context_on_startup() {
    // The startup-complete marker is emitted only after the initialised
    // context has been stored, so seeing it implies the original context was
    // replaced. A generous arrival window proves the event arm was taken:
    // the fallback could not fire before the marker.
    let bus = crate::bus::config::config_bus();
    let (tx, mut rx) = tokio::sync::broadcast::channel::<BackgroundEvent>(64);
    let tool_context = Arc::new(arc_swap::ArcSwap::from_pointee(
        crate::agent::AgentToolContext::new(ToolRegistry::new()),
    ));
    let original = tool_context.load_full();

    let cfg = crate::config::AppConfig {
        max_tokens: 1234,
        table_width_strategy: "waterfill".to_string(),
        ..Default::default()
    };

    spawn_config_subscription_with_timeout(
        tool_context.clone(),
        bus.clone(),
        BackgroundEventSender::new(tx),
        std::time::Duration::from_secs(30),
    );
    bus.publish(ConfigArrived::new(cfg));

    assert!(
        await_startup_complete(&mut rx).await,
        "an arrived config must drive MCP startup initialization"
    );
    assert!(
        !Arc::ptr_eq(&original, &tool_context.load_full()),
        "MCP startup initialization must have installed a new tool context"
    );
}
