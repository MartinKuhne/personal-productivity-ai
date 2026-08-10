//! Agent isolation test — verifies the agent loop runs and emits the
//! contract-correct event sequence on `Bus<AgentEvent>` with **no** UI
//! crate, no `AppOrchestrator`, no `egui` import (quickstart scenario 2,
//! SC-001/SC-004).

use crate::agent::agent_impl::run_agent;
use crate::agent::context::AgentContext;
use crate::agent::events::{AgentEvent, AgentStatus};
use crate::bus::core::{Bus, BusReader};
use crate::config::{AppConfig, LlmConfig};
use std::collections::HashSet;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

fn make_config(port: u16) -> AppConfig {
    let mut config = AppConfig::default();
    config.models.insert(
        "test".to_string(),
        LlmConfig {
            model: "test".to_string(),
            api_url: format!("http://127.0.0.1:{}", port),
            api_key: "valid-key".to_string(),
            cost: None,
            use_case: vec!["chat".to_string()],
        },
    );
    config
}

fn make_ctx(config: AppConfig) -> (AgentContext, BusReader<AgentEvent>) {
    let browser_session = Arc::new(crate::app::session::BrowserSession::new(
        &AppConfig::default(),
    ));
    let agent_event_bus = Bus::new();
    let bus_reader = agent_event_bus.subscribe();
    let ctx = AgentContext {
        config,
        file_event_bus: Bus::new(),
        agent_event_bus,
        active_file: None,
        active_dir: None,
        selected_files: HashSet::new(),
        prompt: "Hello".to_string(),
        cancel_flag: Arc::new(AtomicBool::new(false)),
        history: None,
        model_name: None,
        session_id: uuid::Uuid::new_v4(),
        browser_session,
        pdf_backing: Arc::new(crate::app::session::PdfBackingTracker::new()),
        tool_manager: Arc::new(std::sync::RwLock::new(
            crate::agent::tools::manager::ToolManager::new(),
        )),
        uuid_gen: Arc::new(crate::utils::uuid::SystemUuidGenerator),
    };
    (ctx, bus_reader)
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

fn collect_bus_events(
    reader: &mut BusReader<AgentEvent>,
    timeout: std::time::Duration,
) -> Vec<AgentEvent> {
    let mut events = Vec::new();
    let deadline = std::time::Instant::now() + timeout;
    while std::time::Instant::now() < deadline {
        while let Ok(ev) = reader.try_recv() {
            let is_finished = matches!(ev, AgentEvent::SessionFinished { .. });
            events.push(ev);
            if is_finished {
                while let Ok(ev) = reader.try_recv() {
                    events.push(ev);
                }
                return events;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    events
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

fn classify(ev: &AgentEvent) -> Phase {
    match ev {
        AgentEvent::SessionStarted { .. } => Phase::SessionStarted,
        AgentEvent::Status {
            status: AgentStatus::AwaitingLlm,
            ..
        } => Phase::StatusAwaiting,
        AgentEvent::Status {
            status: AgentStatus::Done,
            ..
        } => Phase::StatusDone,
        AgentEvent::Status {
            status: AgentStatus::ExecutingTools,
            ..
        } => Phase::Other,
        AgentEvent::Thinking { .. } => Phase::Thinking,
        AgentEvent::ContentDelta { .. } => Phase::ContentDelta,
        AgentEvent::ToolCallStarted { .. } => Phase::ToolCallStarted,
        AgentEvent::ToolResult { .. } => Phase::ToolResult,
        AgentEvent::SessionFinished { .. } => Phase::SessionFinished,
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
    let (ctx, mut bus_reader) = make_ctx(make_config(port));
    let session_id = ctx.session_id;
    run_agent(ctx);
    let events = collect_bus_events(&mut bus_reader, std::time::Duration::from_secs(5));

    assert!(!events.is_empty(), "must receive at least one event");

    // Every event must carry the correct session_id (FR-003).
    for (i, ev) in events.iter().enumerate() {
        assert_eq!(
            ev.session_id(),
            session_id,
            "event {i} has wrong session_id: {ev:?}"
        );
    }

    // Extract the contract-relevant phase sequence (ignore DebugEntry,
    // TokenUsage, Status(ExecutingTools) — they are not part of the
    // ordering contract).
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
    assert!(
        awaiting_pos == 1,
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
