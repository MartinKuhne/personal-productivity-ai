//! Agent isolation test — verifies the agent loop runs and emits the
//! contract-correct event sequence on `AgentEventObserver` with **no** UI
//! crate, no `AppOrchestrator`, no `egui` import (quickstart scenario 2,
//! SC-001/SC-004).

use crate::agent_impl::run_agent;
use crate::config::AgentConfig;
use crate::config::AgentConfigBuilder;
use crate::config::LlmConfig;
use crate::context::AgentContext;
use crate::events::{AgentObserverEvent, AgentStatus, RecordingObserver};
use std::sync::Arc;

fn make_agent_config(port: u16) -> AgentConfig {
    use std::collections::HashMap;
    let mut models = HashMap::new();
    models.insert(
        "test".to_string(),
        LlmConfig {
            model: "test".to_string(),
            api_url: format!("http://127.0.0.1:{}", port),
            api_key: "valid-key".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        },
    );
    AgentConfigBuilder::new().with_models(models).build()
}

fn make_ctx(config: AgentConfig) -> (AgentContext, Arc<RecordingObserver>) {
    let recorded = Arc::new(RecordingObserver::new());
    let session_id = uuid::Uuid::new_v4();
    let ctx = crate::context::AgentContextBuilder::new(config, session_id, "Hello".to_string())
        .with_file_observer(std::sync::Arc::new(
            crate::tools::observer::DefaultFileObserver,
        ))
        .with_observer(recorded.clone())
        .with_system_prompts(Vec::new())
        .build();
    (ctx, recorded)
}

fn spawn_one_shot_http_server(body: &[u8]) -> u16 {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let body = body.to_vec();
    std::thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0; 8192];
            let _ = stream.read(&mut buf);
            let _ = stream.write_all(&body);
            let _ = stream.flush();
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    });
    port
}

fn http_response(status_line: &str, body: &str) -> Vec<u8> {
    format!(
        "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

fn collect_observer_events(
    recorded: &RecordingObserver,
    timeout: std::time::Duration,
) -> Vec<AgentObserverEvent> {
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        let events = recorded.events();
        if events
            .iter()
            .any(|ev| matches!(ev, AgentObserverEvent::SessionFinished(..)))
        {
            return events;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    recorded.events()
}

/// Classify an event into a contract-level phase for ordering checks.
#[derive(Debug, PartialEq, Clone, Copy)]
enum Phase {
    SessionStarted,
    StatusAwaiting,
    Thinking,
    ContentDelta,
    ToolCallStarted,
    ToolResult,
    StatusDone,
    SessionFinished,
    Other,
}

fn classify(ev: &AgentObserverEvent) -> Phase {
    match ev {
        AgentObserverEvent::SessionStarted => Phase::SessionStarted,
        AgentObserverEvent::Status(AgentStatus::AwaitingLlm) => Phase::StatusAwaiting,
        AgentObserverEvent::Status(AgentStatus::Done) => Phase::StatusDone,
        AgentObserverEvent::Status(AgentStatus::ExecutingTools) => Phase::Other,
        AgentObserverEvent::Thinking(..) => Phase::Thinking,
        AgentObserverEvent::ContentDelta(..) => Phase::ContentDelta,
        AgentObserverEvent::ToolCallStarted { .. } => Phase::ToolCallStarted,
        AgentObserverEvent::ToolResult { .. } => Phase::ToolResult,
        AgentObserverEvent::SessionFinished(..) => Phase::SessionFinished,
        _ => Phase::Other,
    }
}

/// SC-001/SC-004: The agent runs without any UI crate and emits the
/// contract event ordering:
/// `SessionStarted → Status(AwaitingLlm) → [Thinking|ContentDelta|ToolCallStarted|ToolResult]* → Status(Done) → SessionFinished`.
#[test]
fn test_agent_isolation_event_ordering_no_ui() {
    let body = serde_json::json!({
        "id": "chatcmpl-test", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "All done."}, "finish_reason": "stop"}]
    })
    .to_string();
    let port = spawn_one_shot_http_server(&http_response("HTTP/1.1 200 OK", &body));
    let (ctx, recorded) = make_ctx(make_agent_config(port));
    run_agent(ctx);
    let events = collect_observer_events(&recorded, std::time::Duration::from_secs(5));

    assert!(!events.is_empty(), "must receive at least one event");

    // Extract the contract-relevant phase sequence.
    let phases: Vec<Phase> = events
        .iter()
        .map(classify)
        .filter(|p| *p != Phase::Other)
        .collect();

    // First event must be SessionStarted.
    assert_eq!(
        phases.first(),
        Some(&Phase::SessionStarted),
        "first event must be SessionStarted: {phases:?}"
    );

    // Last event must be SessionFinished.
    assert_eq!(
        phases.last(),
        Some(&Phase::SessionFinished),
        "last event must be SessionFinished: {phases:?}"
    );

    // Must contain Status(AwaitingLlm) after SessionStarted.
    let awaiting_pos = phases
        .iter()
        .position(|p| *p == Phase::StatusAwaiting)
        .expect("must emit Status(AwaitingLlm)");
    assert_eq!(
        awaiting_pos, 1,
        "Status(AwaitingLlm) must be the second event: {phases:?}"
    );

    // Must contain Status(Done) before SessionFinished.
    let done_pos = phases
        .iter()
        .position(|p| *p == Phase::StatusDone)
        .expect("must emit Status(Done)");
    let finished_pos = phases
        .iter()
        .rposition(|p| *p == Phase::SessionFinished)
        .unwrap();
    assert!(
        done_pos < finished_pos,
        "Status(Done) must come before SessionFinished: {phases:?}"
    );

    // No event after SessionFinished (it must be last).
    assert_eq!(
        phases.len(),
        finished_pos + 1,
        "SessionFinished must be the last event: {phases:?}"
    );
}

/// FR-009 / quickstart scenario 6: continuation prompts reuse the same
/// `session_id` and carry forward the conversation history; a new
/// `session_id` starts with no history.
#[test]
fn test_session_continuity_history_carries_over() {
    let body = serde_json::json!({
        "id": "chatcmpl-test", "object": "chat.completion", "created": 0, "model": "test",
        "choices": [{"index": 0, "message": {"role": "assistant", "content": "Reply."}, "finish_reason": "stop"}]
    })
    .to_string();

    // Session 1: fresh session_id, no history.
    let port1 = spawn_one_shot_http_server(&http_response("HTTP/1.1 200 OK", &body));
    let (mut ctx1, recorded1) = make_ctx(make_agent_config(port1));
    let session_id_1 = ctx1.session_id;
    ctx1.history = None;
    run_agent(ctx1);
    let events1 = collect_observer_events(&recorded1, std::time::Duration::from_secs(5));
    let finished1 = events1
        .iter()
        .find_map(|ev| match ev {
            AgentObserverEvent::SessionFinished(history) => Some(history.clone()),
            _ => None,
        })
        .expect("session 1 must emit SessionFinished");
    assert!(
        !finished1.is_empty(),
        "session 1 history must be non-empty after a turn: {finished1:?}"
    );

    // Session 2: SAME session_id, carrying forward history from session 1.
    let port2 = spawn_one_shot_http_server(&http_response("HTTP/1.1 200 OK", &body));
    let (mut ctx2, recorded2) = make_ctx(make_agent_config(port2));
    ctx2.session_id = session_id_1;
    ctx2.history = Some(finished1.clone());
    run_agent(ctx2);
    let events2 = collect_observer_events(&recorded2, std::time::Duration::from_secs(5));
    let finished2 = events2
        .iter()
        .find_map(|ev| match ev {
            AgentObserverEvent::SessionFinished(history) => Some(history.clone()),
            _ => None,
        })
        .expect("session 2 must emit SessionFinished");
    assert!(
        finished2.len() > finished1.len(),
        "continuation session must carry forward and extend history: before={} after={}",
        finished1.len(),
        finished2.len()
    );

    // Session 3: NEW session_id, no history. Must start fresh.
    let port3 = spawn_one_shot_http_server(&http_response("HTTP/1.1 200 OK", &body));
    let (mut ctx3, recorded3) = make_ctx(make_agent_config(port3));
    let session_id_3 = ctx3.session_id;
    assert_ne!(
        session_id_3, session_id_1,
        "new session must have a different session_id"
    );
    ctx3.history = None;
    run_agent(ctx3);
    let events3 = collect_observer_events(&recorded3, std::time::Duration::from_secs(5));
    let finished3 = events3
        .iter()
        .find_map(|ev| match ev {
            AgentObserverEvent::SessionFinished(history) => Some(history.clone()),
            _ => None,
        })
        .expect("session 3 must emit SessionFinished");
    assert!(
        finished3.len() < finished2.len(),
        "new session (different session_id, no history) must start fresh: session3={} session2={}",
        finished3.len(),
        finished2.len()
    );
}
