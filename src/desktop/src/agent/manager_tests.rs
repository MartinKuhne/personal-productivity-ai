//! Tests for `agent/manager.rs`.

use super::*;
use crate::bus::events::typed::AgentEvent;
use crate::config::AppConfig;

#[test]
fn test_new_manager_is_empty() {
    let config = AppConfig::default();
    let mgr = AgentSessionManager::new_for_test(
        config,
        Arc::new(crate::app::session::BrowserSession::new(
            &AppConfig::default(),
        )),
    );
    let state = mgr.state();
    assert!(!state.running);
    assert!(state.status.is_empty());
    assert!(state.response.is_empty());
}

#[test]
fn test_cancel_sets_running_false() {
    let config = AppConfig::default();
    let mut mgr = AgentSessionManager::new_for_test(
        config,
        Arc::new(crate::app::session::BrowserSession::new(
            &AppConfig::default(),
        )),
    );
    mgr.state_mut().running = true;
    mgr.cancel_flag = Some(Arc::new(AtomicBool::new(false)));
    mgr.cancel();
    assert!(!mgr.state.running);
    assert!(mgr.state.status.contains("Aborted"));
}

#[test]
fn test_clear_history_resets_fields() {
    let config = AppConfig::default();
    let mut mgr = AgentSessionManager::new_for_test(
        config,
        Arc::new(crate::app::session::BrowserSession::new(
            &AppConfig::default(),
        )),
    );
    mgr.state.history = Some(vec![Value::String("old".to_string())]);
    mgr.state.token_usage = Some(TokenUsageInfo {
        prompt_tokens: 100,
        completion_tokens: 50,
        total_tokens: 150,
        ..Default::default()
    });
    mgr.clear_history();
    assert!(mgr.state.history.is_none());
    assert!(mgr.state.token_usage.is_none());
}

/// Regression: the bus-driven constructor must observe the
/// first published `ConfigArrived` and store it. The second
/// `drain_config` call (after the reader is dropped) is a
/// no-op.
#[test]
fn test_drain_config_observes_first_event() {
    use crate::bus::config::config_bus;
    use crate::bus::events::config::ConfigArrived;

    let bus = config_bus();
    let mut mgr = AgentSessionManager::new(
        bus.clone(),
        Arc::new(crate::app::session::BrowserSession::new(
            &AppConfig::default(),
        )),
        Arc::new(crate::app::session::PdfBackingTracker::new()),
        Arc::new(std::sync::RwLock::new(
            crate::agent::tools::manager::ToolManager::new(),
        )),
    );

    // Before any event: not arrived, default config in use.
    assert!(!mgr.config_arrived());

    let cfg = AppConfig {
        csv_db_path: Some("/tmp/foo".to_string()),
        ..AppConfig::default()
    };
    bus.publish(ConfigArrived::new(cfg.clone()));

    assert!(mgr.drain_config());
    assert!(mgr.config_arrived());
    assert_eq!(mgr.config.csv_db_path, cfg.csv_db_path);

    // Subsequent drain is a no-op (reader dropped after first).
    assert!(!mgr.drain_config());
}

/// Regression: `drain_config` returns false when no event has
/// been published yet, and does not flip the `config_arrived`
/// flag.
#[test]
fn test_drain_config_returns_false_when_empty() {
    let bus = crate::bus::config::config_bus();
    let mut mgr = AgentSessionManager::new(
        bus,
        Arc::new(crate::app::session::BrowserSession::new(
            &AppConfig::default(),
        )),
        Arc::new(crate::app::session::PdfBackingTracker::new()),
        Arc::new(std::sync::RwLock::new(
            crate::agent::tools::manager::ToolManager::new(),
        )),
    );

    assert!(!mgr.drain_config());
    assert!(!mgr.config_arrived());
}

/// Regression: the production startup order in `main.rs`
/// must be "construct subscribers first, then publish" —
/// `tokio::sync::broadcast` only delivers an event to
/// subscribers that exist at publish time. Reversing the
/// order silently drops the event and the agent never sees
/// the loaded config. This test pins that contract: build
/// the manager (which subscribes), then publish, then drain.
#[test]
fn test_construct_then_publish_order_drains_config() {
    let bus = crate::bus::config::config_bus();
    // 1. Construct first — this is the `AgentSessionManager::new`
    //    call inside `FastMdApp::new`. It subscribes here.
    let mut mgr = AgentSessionManager::new(
        bus.clone(),
        Arc::new(crate::app::session::BrowserSession::new(
            &AppConfig::default(),
        )),
        Arc::new(crate::app::session::PdfBackingTracker::new()),
        Arc::new(std::sync::RwLock::new(
            crate::agent::tools::manager::ToolManager::new(),
        )),
    );

    // 2. Publish second — this is the line in `main.rs` that
    //    fires `config_bus.publish(...)` after construction.
    //    Because the subscription is already in place, the
    //    broadcast channel delivers the event to this reader.
    let cfg = AppConfig {
        csv_db_path: Some("/tmp/regression".to_string()),
        ..AppConfig::default()
    };
    bus.publish(ConfigArrived::new(cfg.clone()));

    // 3. Drain on the first UI frame.
    assert!(mgr.drain_config());
    assert!(mgr.config_arrived());
    assert_eq!(mgr.config.csv_db_path, cfg.csv_db_path);
}

/// Regression: the *reverse* order (publish, then construct)
/// silently drops the event. We assert this so any future
/// refactor that flips the order is caught immediately.
#[test]
fn test_publish_then_construct_order_drops_event() {
    let bus = crate::bus::config::config_bus();
    bus.publish(ConfigArrived::new(AppConfig::default()));

    // Subscribing after the publish means the broadcast
    // channel won't deliver the event to this reader.
    let mut mgr = AgentSessionManager::new(
        bus,
        Arc::new(crate::app::session::BrowserSession::new(
            &AppConfig::default(),
        )),
        Arc::new(crate::app::session::PdfBackingTracker::new()),
        Arc::new(std::sync::RwLock::new(
            crate::agent::tools::manager::ToolManager::new(),
        )),
    );

    assert!(!mgr.drain_config());
    assert!(!mgr.config_arrived());
}

#[test]
fn test_queue_prompt_and_take_next() {
    let config = AppConfig::default();
    let mut mgr = AgentSessionManager::new_for_test(
        config,
        Arc::new(crate::app::session::BrowserSession::new(
            &AppConfig::default(),
        )),
    );

    // Initially no queued prompts
    assert_eq!(mgr.queued_prompt_count(), 0);
    assert!(mgr.take_next_queued_prompt().is_none());

    // Queue a prompt
    mgr.queue_prompt("first prompt".to_string());
    assert_eq!(mgr.queued_prompt_count(), 1);

    // Queue another
    mgr.queue_prompt("second prompt".to_string());
    assert_eq!(mgr.queued_prompt_count(), 2);

    // Take first
    let first = mgr.take_next_queued_prompt();
    assert_eq!(first, Some("first prompt".to_string()));
    assert_eq!(mgr.queued_prompt_count(), 1);

    // Take second
    let second = mgr.take_next_queued_prompt();
    assert_eq!(second, Some("second prompt".to_string()));
    assert_eq!(mgr.queued_prompt_count(), 0);

    // Empty now
    assert!(mgr.take_next_queued_prompt().is_none());
}

#[test]
fn test_handle_agent_event_finished_returns_queued_prompt() {
    use crate::bus::events::typed::AgentEvent;
    let config = AppConfig::default();
    let mut mgr = AgentSessionManager::new_for_test(
        config,
        Arc::new(crate::app::session::BrowserSession::new(
            &AppConfig::default(),
        )),
    );

    // Queue a prompt
    mgr.queue_prompt("queued prompt".to_string());
    mgr.state.running = true;

    // Handle Finished event - should return the queued prompt
    let result = mgr.handle_agent_event(AgentEvent::Finished(vec![]));
    assert_eq!(result, Some("queued prompt".to_string()));
    assert!(!mgr.state.running);
    assert_eq!(mgr.queued_prompt_count(), 0);
}

#[test]
fn test_handle_agent_event_finished_no_queued_prompt() {
    use crate::bus::events::typed::AgentEvent;
    let config = AppConfig::default();
    let mut mgr = AgentSessionManager::new_for_test(
        config,
        Arc::new(crate::app::session::BrowserSession::new(
            &AppConfig::default(),
        )),
    );

    mgr.state.running = true;

    // Handle Finished event with no queued prompts
    let result = mgr.handle_agent_event(AgentEvent::Finished(vec![]));
    assert!(result.is_none());
    assert!(!mgr.state.running);
}

#[test]
fn test_handle_agent_event_failed_clears_queue() {
    use crate::bus::events::typed::AgentEvent;
    let config = AppConfig::default();
    let mut mgr = AgentSessionManager::new_for_test(
        config,
        Arc::new(crate::app::session::BrowserSession::new(
            &AppConfig::default(),
        )),
    );

    // Queue some prompts
    mgr.queue_prompt("prompt 1".to_string());
    mgr.queue_prompt("prompt 2".to_string());
    mgr.state.running = true;

    // Handle Failed event - should clear the queue
    let result = mgr.handle_agent_event(AgentEvent::Failed("test error".to_string()));
    assert!(result.is_none());
    assert!(!mgr.state.running);
    assert_eq!(mgr.queued_prompt_count(), 0);
    assert!(mgr.state.status.contains("Error"));
}

#[test]
fn test_handle_agent_event_debug_entry_accumulates() {
    use crate::bus::events::debug::{AgentDebugEntry, DebugEntryKind, DebugEntryRow};
    use crate::bus::events::typed::AgentEvent;

    let config = AppConfig::default();
    let mut mgr = AgentSessionManager::new_for_test(
        config,
        Arc::new(crate::app::session::BrowserSession::new(
            &AppConfig::default(),
        )),
    );

    let entry = AgentDebugEntry {
        turn: 1,
        session: 1,
        timestamp: chrono::Local::now(),
        kind: DebugEntryKind::Outgoing,
        summary: "Turn 1 — Outgoing".to_string(),
        content: Some(serde_json::json!({"test": true})),
        row_type: DebugEntryRow::Entry,
    };

    let result = mgr.handle_agent_event(AgentEvent::DebugEntry(entry));
    assert!(result.is_none());
    assert_eq!(mgr.state.debug_entries.len(), 1);
    assert_eq!(mgr.state.debug_entries[0].turn, 1);
    assert_eq!(mgr.state.debug_entries[0].session, 1);
    assert_eq!(mgr.state.debug_entries[0].kind, DebugEntryKind::Outgoing);

    // Second entry accumulates (no clearing)
    let entry2 = AgentDebugEntry {
        turn: 2,
        session: 1,
        timestamp: chrono::Local::now(),
        kind: DebugEntryKind::Incoming,
        summary: "Turn 2 — Incoming".to_string(),
        content: Some(serde_json::json!({"choices": []})),
        row_type: DebugEntryRow::Entry,
    };
    mgr.handle_agent_event(AgentEvent::DebugEntry(entry2));
    assert_eq!(mgr.state.debug_entries.len(), 2);
}

#[test]
fn test_debug_entries_never_cleared_on_new_session() {
    use crate::bus::events::debug::{AgentDebugEntry, DebugEntryKind, DebugEntryRow};
    use crate::bus::events::typed::AgentEvent;

    let config = AppConfig::default();
    let mut mgr = AgentSessionManager::new_for_test(
        config,
        Arc::new(crate::app::session::BrowserSession::new(
            &AppConfig::default(),
        )),
    );

    let entry = AgentDebugEntry {
        turn: 1,
        session: 1,
        timestamp: chrono::Local::now(),
        kind: DebugEntryKind::Outgoing,
        summary: "Turn 1 — Outgoing".to_string(),
        content: Some(serde_json::json!({"test": true})),
        row_type: DebugEntryRow::Entry,
    };
    mgr.handle_agent_event(AgentEvent::DebugEntry(entry));
    assert_eq!(mgr.state.debug_entries.len(), 1);

    // Simulate a new session start (clears history but not debug entries)
    mgr.clear_history();
    assert_eq!(
        mgr.state.debug_entries.len(),
        1,
        "debug entries must survive clear_history"
    );
}

#[test]
fn test_session_counter_increments_on_start_session() {
    use crate::bus::core::Bus;

    let mut config = AppConfig::default();
    config.models.insert(
        "test".to_string(),
        crate::config::LlmConfig {
            model: "test".to_string(),
            api_url: "http://localhost".to_string(),
            api_key: "".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        },
    );

    let mut mgr = AgentSessionManager::new_for_test(
        config,
        Arc::new(crate::app::session::BrowserSession::new(
            &AppConfig::default(),
        )),
    );
    // new_for_test sets session_counter to 0

    let (tx, rx) = std::sync::mpsc::channel();
    let bus = Bus::new();

    // Start first session — should increment to 1
    mgr.start_session(
        tx.clone(),
        "prompt 1".to_string(),
        None,
        None,
        HashSet::new(),
        bus.clone(),
    );
    // Blocking drain: wait until the spawned agent thread finishes (Failed, no valid API key)
    while let Ok(ev) = rx.recv() {
        match &ev {
            BackgroundEvent::Agent(AgentEvent::DebugEntry(e)) => {
                assert_eq!(
                    e.session, 1,
                    "first session debug entries must have session 1"
                );
            }
            BackgroundEvent::Agent(AgentEvent::Failed(_)) => break,
            _ => {}
        }
    }

    // Start second session — should increment to 2
    mgr.start_session(
        tx.clone(),
        "prompt 2".to_string(),
        None,
        None,
        HashSet::new(),
        bus.clone(),
    );
    while let Ok(ev) = rx.recv() {
        match &ev {
            BackgroundEvent::Agent(AgentEvent::DebugEntry(e)) => {
                assert_eq!(
                    e.session, 2,
                    "second session debug entries must have session 2"
                );
            }
            BackgroundEvent::Agent(AgentEvent::Failed(_)) => break,
            _ => {}
        }
    }
}
