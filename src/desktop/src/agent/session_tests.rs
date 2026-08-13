//! Tests for `agent/session.rs`.

use super::*;
use crate::agent::config::AgentConfigBuilder;
use crate::bus::events::debug::{AgentDebugEntry, DebugEntryKind, DebugEntryRow};
use std::collections::HashSet;

/// Build a default `AgentSession` for unit tests. Each call spawns a
/// fresh driver thread.
fn make_session() -> AgentSession {
    AgentSession::new(
        crate::bus::core::Bus::new(),
        Arc::new(crate::app::session::BrowserSession::with_resolved(
            AgentConfig::default().browser().clone(),
        )),
        Arc::new(crate::app::session::PdfBackingTracker::new()),
        Arc::new(arc_swap::ArcSwap::from_pointee(
            crate::agent::AgentToolContext::new(crate::agent::tools::registry::ToolRegistry::new()),
        )),
    )
}

#[test]
fn test_new_manager_is_empty() {
    let mgr = make_session();
    let state = mgr.state();
    assert!(!state.running);
    assert!(state.status.is_empty());
    assert!(state.response.is_empty());
}

#[test]
fn test_cancel_sets_running_false() {
    let mut mgr = make_session();
    mgr.state_mut().running = true;
    mgr.cancel_flag = Some(Arc::new(AtomicBool::new(false)));
    mgr.cancel();
    assert!(!mgr.state.running);
    assert!(mgr.state.status.contains("Aborted"));
}

#[test]
fn test_clear_history_resets_fields() {
    let mut mgr = make_session();
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

#[test]
fn test_set_agent_config_round_trip() {
    let mgr = make_session();
    let mut models = std::collections::HashMap::new();
    models.insert(
        "a".to_string(),
        crate::config::LlmConfig {
            model: "a".to_string(),
            api_url: "http://a".to_string(),
            api_key: "k".to_string(),
            cost: Some(0),
            use_case: vec!["chat".to_string()],
        },
    );
    let new_cfg = AgentConfigBuilder::new().with_models(models).build();
    mgr.set_agent_config(new_cfg.clone());
    let read_back = mgr.agent_config();
    assert_eq!(read_back.models().len(), 1);
    assert!(read_back.models().contains_key("a"));
}

#[test]
fn test_replace_agent_config_passes_current() {
    let mgr = make_session();
    mgr.set_agent_config(AgentConfigBuilder::new().with_max_tokens(8192).build());
    mgr.replace_agent_config(|c| {
        AgentConfigBuilder::new()
            .with_max_tokens(c.max_tokens() * 2)
            .build()
    });
    assert_eq!(mgr.agent_config().max_tokens(), 16384);
}

#[test]
fn test_queue_prompt_and_take_next() {
    let mut mgr = make_session();

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
fn test_finished_returns_queued_prompt() {
    let mut mgr = make_session();

    // Queue a prompt
    mgr.queue_prompt("queued prompt".to_string());
    mgr.state.running = true;

    // Simulate SessionFinished: set running false, store history, dequeue
    mgr.set_running(false);
    mgr.set_history(Some(vec![]));
    let result = mgr.take_next_queued_prompt();
    assert_eq!(result, Some("queued prompt".to_string()));
    assert!(!mgr.state.running);
    assert_eq!(mgr.queued_prompt_count(), 0);
}

#[test]
fn test_finished_no_queued_prompt() {
    let mut mgr = make_session();

    mgr.state.running = true;

    // Simulate SessionFinished with no queued prompts
    mgr.set_running(false);
    mgr.set_history(Some(vec![]));
    let result = mgr.take_next_queued_prompt();
    assert!(result.is_none());
    assert!(!mgr.state.running);
}

#[test]
fn test_failed_clears_queue() {
    let mut mgr = make_session();

    // Queue some prompts
    mgr.queue_prompt("prompt 1".to_string());
    mgr.queue_prompt("prompt 2".to_string());
    mgr.state.running = true;

    // Simulate Failed event
    mgr.set_running(false);
    mgr.set_status("Error: test error".to_string());
    mgr.clear_queued_prompts();
    assert!(!mgr.state.running);
    assert_eq!(mgr.queued_prompt_count(), 0);
    assert!(mgr.state.status.contains("Error"));
}

#[test]
fn test_debug_entry_accumulates() {
    let mut mgr = make_session();

    let entry = AgentDebugEntry {
        turn: 1,
        timestamp: chrono::Local::now(),
        kind: DebugEntryKind::Outgoing,
        summary: "Turn 1 — Outgoing".to_string(),
        content: Some(serde_json::json!({"test": true})),
        row_type: DebugEntryRow::Entry,
    };

    mgr.push_debug_entry(entry);
    assert_eq!(mgr.state.debug_entries.len(), 1);
    assert_eq!(mgr.state.debug_entries[0].turn, 1);
    assert_eq!(mgr.state.debug_entries[0].kind, DebugEntryKind::Outgoing);

    // Second entry accumulates (no clearing)
    let entry2 = AgentDebugEntry {
        turn: 2,
        timestamp: chrono::Local::now(),
        kind: DebugEntryKind::Incoming,
        summary: "Turn 2 — Incoming".to_string(),
        content: Some(serde_json::json!({"choices": []})),
        row_type: DebugEntryRow::Entry,
    };
    mgr.push_debug_entry(entry2);
    assert_eq!(mgr.state.debug_entries.len(), 2);
}

#[test]
fn test_debug_entries_never_cleared_on_new_session() {
    let mut mgr = make_session();

    let entry = AgentDebugEntry {
        turn: 1,
        timestamp: chrono::Local::now(),
        kind: DebugEntryKind::Outgoing,
        summary: "Turn 1 — Outgoing".to_string(),
        content: Some(serde_json::json!({"test": true})),
        row_type: DebugEntryRow::Entry,
    };
    mgr.push_debug_entry(entry);
    assert_eq!(mgr.state.debug_entries.len(), 1);

    // Simulate a new session start (clears history but not debug entries)
    mgr.clear_history();
    assert_eq!(
        mgr.state.debug_entries.len(),
        1,
        "debug entries must survive clear_history"
    );
}

/// T035: Uuid session identity — verifies `current_session_id` is set
/// on each `submit_prompt` call and is a valid `Uuid`. Uses
/// `Bus<AgentEvent>` since the agent no longer publishes on `tx_gui`.
#[test]
fn test_current_session_id_set_on_submit_prompt() {
    use crate::app::events::AgentEvent as SeamAgentEvent;

    let mut mgr = make_session();
    // Before first session: no session_id
    assert!(mgr.current_session_id().is_none());

    // Submit first session — current_session_id must be set
    let reader1 = mgr.event_bus().subscribe();
    let session_id_1 = uuid::Uuid::new_v4();
    mgr.submit_prompt(crate::agent::events::AgentPrompt {
        session_id: session_id_1,
        text: "prompt 1".to_string(),
        system_prompts: Vec::new(),
        active_file: None,
        active_dir: None,
        selected_files: HashSet::new(),
        cancel_flag: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    });
    assert_eq!(
        mgr.current_session_id(),
        Some(session_id_1),
        "current_session_id must match the submitted prompt"
    );
    assert!(!session_id_1.is_nil(), "session_id must be a real Uuid");

    // Drain bus until the agent's prompt loop processes the message.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        let mut got_started = false;
        while let Ok(ev) = reader1.try_recv() {
            if let SeamAgentEvent::SessionStarted { session_id } = &ev {
                assert_eq!(
                    *session_id, session_id_1,
                    "SessionStarted must carry the manager's session_id"
                );
                got_started = true;
            }
        }
        if got_started {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
