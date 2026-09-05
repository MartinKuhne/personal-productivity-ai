//! Tests for `agent/session.rs`.

use super::*;
use crate::config::AgentConfigBuilder;
use crate::events::{
    AgentDebugEntry, AgentObserverEvent, DebugEntryKind, DebugEntryRow, RecordingObserver,
};
use std::collections::HashSet;
use std::sync::atomic::{AtomicUsize, Ordering};

struct TestNoopObserver;
impl crate::events::AgentEventObserver for TestNoopObserver {
    fn on_session_started(&self) {}
    fn on_session_finished(&self, _history: Vec<serde_json::Value>) {}
    fn on_status(&self, _status: crate::events::AgentStatus) {}
    fn on_thinking(&self, _text: String) {}
    fn on_content_delta(&self, _text: String) {}
    fn on_tool_call_started(&self, _id: String, _name: String, _args: serde_json::Value) {}
    fn on_tool_result(&self, _id: String, _name: String, _result: serde_json::Value) {}
    fn on_tool_side_effect(&self, _effect: crate::events::ToolSideEffect) {}
    fn on_debug_entry(&self, _entry: AgentDebugEntry) {}
    fn on_token_usage(&self, _usage: TokenUsageInfo) {}
    fn on_failed(&self, _error: String) {}
}

/// Build a default `AgentSession` for unit tests. Each call spawns a
/// fresh driver thread.
fn make_session() -> AgentSession {
    let factory: AgentObserverFactory = Arc::new(|_session_id| Arc::new(TestNoopObserver));
    AgentSession::builder()
        .with_observer_factory(factory)
        .with_file_observer(std::sync::Arc::new(
            crate::tools::observer::DefaultFileObserver,
        ))
        .with_tool_call_policy(Arc::new(crate::tools::policy::DefaultToolCallPolicy))
        .with_tool_context(Arc::new(arc_swap::ArcSwap::from_pointee(
            crate::AgentToolContext::new(crate::tools::registry::ToolRegistry::new()),
        )))
        .build()
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
fn slow_session_exponential_backoff() {
    use std::collections::HashMap;
    let mut models = HashMap::new();
    models.insert(
        "test".to_string(),
        crate::config::LlmConfig {
            model: "test".to_string(),
            api_url: "http://127.0.0.1:0".to_string(),
            api_key: "valid-key".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        },
    );
    let agent_config = crate::config::AgentConfigBuilder::new()
        .with_models(models)
        .build();

    let recorded = Arc::new(RecordingObserver::new());
    let recorded_clone = recorded.clone();
    let factory: AgentObserverFactory = Arc::new(move |_session_id| recorded_clone.clone());
    let mut mgr = AgentSession::builder()
        .with_agent_config(agent_config)
        .with_observer_factory(factory)
        .with_file_observer(std::sync::Arc::new(
            crate::tools::observer::DefaultFileObserver,
        ))
        .with_tool_call_policy(Arc::new(crate::tools::policy::DefaultToolCallPolicy))
        .with_tool_context(Arc::new(arc_swap::ArcSwap::from_pointee(
            crate::AgentToolContext::new(crate::tools::registry::ToolRegistry::new()),
        )))
        .build();

    // Before first session: no session_id
    assert!(mgr.current_session_id().is_none());

    // Submit first session — current_session_id must be set
    let session_id_1 = uuid::Uuid::new_v4();
    mgr.submit_prompt(crate::events::AgentPrompt {
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

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        let events = recorded.events();
        if events.contains(&AgentObserverEvent::SessionStarted) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(
        recorded
            .events()
            .contains(&AgentObserverEvent::SessionStarted)
    );
}

/// Regression test: submitting two prompts with the same `session_id` to
/// `AgentSession` must carry forward and extend the conversation history
/// in `spawn_driver`.
#[test]
fn test_driver_continuation_reuses_history() {
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let body = serde_json::json!({
        "id": "chatcmpl-test", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "Reply."}, "finish_reason": "stop"}]
    })
    .to_string();
    let response_bytes = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for _ in 0..2 {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0; 8192];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(&response_bytes);
                let _ = stream.flush();
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        }
    });

    let mut models = HashMap::new();
    models.insert(
        "test".to_string(),
        crate::config::LlmConfig {
            model: "test".to_string(),
            api_url: format!("http://127.0.0.1:{}", port),
            api_key: "valid-key".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        },
    );
    let agent_config = crate::config::AgentConfigBuilder::new()
        .with_models(models)
        .build();

    let recorded = Arc::new(RecordingObserver::new());
    let recorded_clone = recorded.clone();
    let factory: AgentObserverFactory = Arc::new(move |_session_id| recorded_clone.clone());
    let mut mgr = AgentSession::builder()
        .with_agent_config(agent_config)
        .with_observer_factory(factory)
        .with_file_observer(std::sync::Arc::new(
            crate::tools::observer::DefaultFileObserver,
        ))
        .with_tool_call_policy(Arc::new(crate::tools::policy::DefaultToolCallPolicy))
        .with_tool_context(Arc::new(arc_swap::ArcSwap::from_pointee(
            crate::AgentToolContext::new(crate::tools::registry::ToolRegistry::new()),
        )))
        .build();

    let session_id = uuid::Uuid::new_v4();

    // Turn 1
    mgr.submit_prompt(crate::events::AgentPrompt {
        session_id,
        text: "first prompt".to_string(),
        system_prompts: Vec::new(),
        active_file: None,
        active_dir: None,
        selected_files: HashSet::new(),
        cancel_flag: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    });

    // Wait for Turn 1 to finish
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut finished1 = None;
    while std::time::Instant::now() < deadline {
        let events = recorded.events();
        for ev in &events {
            if let AgentObserverEvent::SessionFinished(h) = ev {
                finished1 = Some(h.clone());
                break;
            }
        }
        if finished1.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let h1 = finished1.expect("turn 1 must emit SessionFinished");
    assert!(!h1.is_empty(), "turn 1 history must be non-empty");

    // Turn 2 with SAME session_id
    mgr.submit_prompt(crate::events::AgentPrompt {
        session_id,
        text: "second prompt".to_string(),
        system_prompts: Vec::new(),
        active_file: None,
        active_dir: None,
        selected_files: HashSet::new(),
        cancel_flag: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    });

    // Wait for Turn 2 to finish
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut finished2 = None;
    while std::time::Instant::now() < deadline {
        let events = recorded.events();
        let finish_events: Vec<_> = events
            .into_iter()
            .filter_map(|ev| match ev {
                AgentObserverEvent::SessionFinished(h) => Some(h),
                _ => None,
            })
            .collect();
        if finish_events.len() >= 2 {
            finished2 = Some(finish_events[1].clone());
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let h2 = finished2.expect("turn 2 must emit SessionFinished");
    assert!(
        h2.len() > h1.len(),
        "turn 2 must carry forward and extend turn 1 history: turn1_len={} turn2_len={}",
        h1.len(),
        h2.len()
    );
}

/// Regression test: submitting a continuation prompt for the same session must NOT
/// reset the turn number to 1.
#[test]
fn test_driver_continuation_turn_number_increments() {
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    let body = serde_json::json!({
        "id": "chatcmpl-test", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "Reply."}, "finish_reason": "stop"}]
    })
    .to_string();
    let response_bytes = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes();

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for _ in 0..2 {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = [0; 8192];
                let _ = stream.read(&mut buf);
                let _ = stream.write_all(&response_bytes);
                let _ = stream.flush();
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        }
    });

    let mut models = HashMap::new();
    models.insert(
        "test".to_string(),
        crate::config::LlmConfig {
            model: "test".to_string(),
            api_url: format!("http://127.0.0.1:{}", port),
            api_key: "valid-key".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        },
    );
    let agent_config = crate::config::AgentConfigBuilder::new()
        .with_models(models)
        .build();

    let recorded = Arc::new(RecordingObserver::new());
    let recorded_clone = recorded.clone();
    let factory_calls = Arc::new(AtomicUsize::new(0));
    let factory_calls_clone = factory_calls.clone();
    let factory: AgentObserverFactory = Arc::new(move |_session_id| {
        factory_calls_clone.fetch_add(1, Ordering::SeqCst);
        recorded_clone.clone()
    });
    let mut mgr = AgentSession::builder()
        .with_agent_config(agent_config)
        .with_observer_factory(factory)
        .with_file_observer(std::sync::Arc::new(
            crate::tools::observer::DefaultFileObserver,
        ))
        .with_tool_call_policy(Arc::new(crate::tools::policy::DefaultToolCallPolicy))
        .with_tool_context(Arc::new(arc_swap::ArcSwap::from_pointee(
            crate::AgentToolContext::new(crate::tools::registry::ToolRegistry::new()),
        )))
        .build();

    let session_id = uuid::Uuid::new_v4();

    // Turn 1
    mgr.submit_prompt(crate::events::AgentPrompt {
        session_id,
        text: "first prompt".to_string(),
        system_prompts: Vec::new(),
        active_file: None,
        active_dir: None,
        selected_files: HashSet::new(),
        cancel_flag: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    });

    // Wait for Turn 1 to finish
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        let finish_count = recorded
            .events()
            .iter()
            .filter(|ev| matches!(ev, AgentObserverEvent::SessionFinished(_)))
            .count();
        if finish_count >= 1 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // Turn 2 with SAME session_id
    mgr.submit_prompt(crate::events::AgentPrompt {
        session_id,
        text: "second prompt".to_string(),
        system_prompts: Vec::new(),
        active_file: None,
        active_dir: None,
        selected_files: HashSet::new(),
        cancel_flag: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
    });

    // Wait for Turn 2 to finish
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while std::time::Instant::now() < deadline {
        let finish_count = recorded
            .events()
            .iter()
            .filter(|ev| matches!(ev, AgentObserverEvent::SessionFinished(_)))
            .count();
        if finish_count >= 2 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    let debug_entries: Vec<_> = recorded
        .events()
        .into_iter()
        .filter_map(|ev| match ev {
            AgentObserverEvent::DebugEntry(entry) => Some(entry),
            _ => None,
        })
        .filter(|entry| entry.row_type == DebugEntryRow::Entry)
        .collect();

    // The first turn in the first prompt should be 1.
    // The first turn in the second prompt should be 2.
    // Let's collect all the unique turn numbers used for 'Outgoing' entries.
    let outgoing_turns: Vec<usize> = debug_entries
        .iter()
        .filter(|e| e.kind == DebugEntryKind::Outgoing)
        .map(|e| e.turn)
        .collect();

    assert_eq!(outgoing_turns, vec![1, 2], "The turns should be 1, then 2.");
    assert_eq!(
        factory_calls.load(Ordering::SeqCst),
        1,
        "a continued logical session must reuse its observer"
    );
}
