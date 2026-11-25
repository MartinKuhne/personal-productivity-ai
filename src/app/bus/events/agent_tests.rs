//! Tests for the agent bus observers in `bus::events::agent`.

use super::*;
use crate::agent::events::{AgentObserverEvent, DebugEntryKind, DebugEntryRow};
use crate::bus::core::BusReader;
use crate::bus::events::file::{FileChanged, FileChangedKind};
use std::path::{Path, PathBuf};

#[test]
fn test_bus_agent_event_observer_forwards_file_events() {
    let session_id = Uuid::new_v4();
    let agent_bus = Bus::new();
    let agent_reader = agent_bus.subscribe();
    let file_bus: Bus<FileChanged> = Bus::new();
    let file_reader = file_bus.subscribe();

    let observer = BusAgentEventObserver::with_file_bus(session_id, agent_bus, file_bus);

    let test_path = PathBuf::from("notes/test.md");
    observer.on_tool_side_effect(ToolSideEffect::FileChanged {
        path: test_path.clone(),
    });

    let ev = file_reader
        .try_recv()
        .expect("file event must be published");
    assert!(
        matches!(
            &ev,
            changed
                if changed.kind == FileChangedKind::Changed
                    && changed.files.len() == 1
                    && changed.files[0].path == test_path
        ),
        "tool side effect publishes [FileChanged][Changed] for the path"
    );

    let agent_ev = agent_reader
        .try_recv()
        .expect("agent event must be published");
    match agent_ev {
        AgentEvent::ToolSideEffect { effect, .. } => {
            assert_eq!(effect, ToolSideEffect::FileChanged { path: test_path });
        }
        other => panic!("expected ToolSideEffect, got {:?}", other),
    }
}

#[test]
fn test_composite_agent_event_observer_forwards_all_events() {
    let obs1 = Arc::new(crate::agent::events::RecordingObserver::new());
    let obs2 = Arc::new(crate::agent::events::RecordingObserver::new());

    let composite = CompositeAgentEventObserver::new(vec![obs1.clone(), obs2.clone()]);
    composite.on_session_started();
    composite.on_thinking("reasoning".to_string());
    composite.on_content_delta("delta".to_string());
    composite.on_status(AgentStatus::Done);
    composite.on_session_finished(vec![serde_json::json!({"role": "user", "content": "hi"})]);

    assert_eq!(obs1.events().len(), 5);
    assert_eq!(obs2.events().len(), 5);
    assert_eq!(obs1.events(), obs2.events());
}

/// A debug entry with a fixed payload for observer round-trip assertions.
fn sample_debug_entry() -> AgentDebugEntry {
    AgentDebugEntry {
        turn: 1,
        timestamp: chrono::Local::now(),
        kind: DebugEntryKind::Outgoing,
        summary: "outgoing".into(),
        content: Some(serde_json::json!({ "role": "user" })),
        row_type: DebugEntryRow::Entry,
    }
}

/// A token-usage payload with every field populated.
fn sample_usage() -> TokenUsageInfo {
    TokenUsageInfo {
        prompt_tokens: 10,
        completion_tokens: 4,
        total_tokens: 14,
        cached_tokens: Some(2),
        reasoning_tokens: Some(1),
    }
}

/// Drain every event currently queued on a bus reader.
fn drain<T: Clone>(reader: &BusReader<T>) -> Vec<T> {
    let mut out = Vec::new();
    while let Ok(event) = reader.try_recv() {
        out.push(event);
    }
    out
}

#[test]
fn test_bus_observer_replays_every_callback_on_the_bus_in_order() {
    let session_id = Uuid::new_v4();
    let bus: Bus<AgentEvent> = Bus::new();
    let reader = bus.subscribe();
    let observer = BusAgentEventObserver::new(session_id, bus.clone());

    let entry = sample_debug_entry();
    let usage = sample_usage();
    let history = vec![serde_json::json!({ "role": "user", "content": "hi" })];

    observer.on_session_started();
    observer.on_status(AgentStatus::AwaitingLlm);
    observer.on_thinking("think".into());
    observer.on_content_delta("chunk".into());
    observer.on_tool_call_started("id1".into(), "grep".into(), serde_json::json!({ "q": "x" }));
    observer.on_tool_result(
        "id1".into(),
        "grep".into(),
        serde_json::json!({ "hits": 1 }),
    );
    observer.on_tool_side_effect(ToolSideEffect::FileChanged {
        path: PathBuf::from("notes/a.md"),
    });
    observer.on_debug_entry(entry.clone());
    observer.on_token_usage(usage.clone());
    observer.on_failed("boom".into());
    observer.on_session_finished(history.clone());

    let expected = vec![
        AgentEvent::SessionStarted { session_id },
        AgentEvent::Status {
            session_id,
            status: AgentStatus::AwaitingLlm,
        },
        AgentEvent::Thinking {
            session_id,
            text: "think".into(),
        },
        AgentEvent::ContentDelta {
            session_id,
            text: "chunk".into(),
        },
        AgentEvent::ToolCallStarted {
            session_id,
            id: "id1".into(),
            name: "grep".into(),
            args: serde_json::json!({ "q": "x" }),
        },
        AgentEvent::ToolResult {
            session_id,
            id: "id1".into(),
            name: "grep".into(),
            result: serde_json::json!({ "hits": 1 }),
        },
        AgentEvent::ToolSideEffect {
            session_id,
            effect: ToolSideEffect::FileChanged {
                path: PathBuf::from("notes/a.md"),
            },
        },
        AgentEvent::DebugEntry {
            session_id,
            entry: entry.clone(),
        },
        AgentEvent::TokenUsage {
            session_id,
            usage: usage.clone(),
        },
        AgentEvent::Failed {
            session_id,
            error: "boom".into(),
        },
        AgentEvent::SessionFinished {
            session_id,
            history: history.clone(),
        },
    ];

    let actual = drain(&reader);
    assert_eq!(
        actual.len(),
        expected.len(),
        "exactly one bus event per observer callback, got {actual:?}"
    );
    for (index, (got, want)) in actual.iter().zip(expected.iter()).enumerate() {
        // `AgentEvent` is `Debug` but not `PartialEq`, so the payload
        // comparison goes through the derived debug representation.
        assert_eq!(
            format!("{got:?}"),
            format!("{want:?}"),
            "bus event #{index} differs from the observer callback"
        );
    }
}

#[test]
fn test_session_id_returns_the_id_of_every_variant() {
    // Every variant carries its own id so a shuffled match arm cannot pass
    // by returning some other variant's id.
    let id = |n: u128| Uuid::from_u128(n);
    let cases: Vec<(&str, AgentEvent)> = vec![
        (
            "SessionStarted",
            AgentEvent::SessionStarted { session_id: id(1) },
        ),
        (
            "SessionFinished",
            AgentEvent::SessionFinished {
                session_id: id(2),
                history: Vec::new(),
            },
        ),
        (
            "Status",
            AgentEvent::Status {
                session_id: id(3),
                status: AgentStatus::Done,
            },
        ),
        (
            "Thinking",
            AgentEvent::Thinking {
                session_id: id(4),
                text: String::new(),
            },
        ),
        (
            "ContentDelta",
            AgentEvent::ContentDelta {
                session_id: id(5),
                text: String::new(),
            },
        ),
        (
            "ToolCallStarted",
            AgentEvent::ToolCallStarted {
                session_id: id(6),
                id: String::new(),
                name: String::new(),
                args: serde_json::Value::Null,
            },
        ),
        (
            "ToolResult",
            AgentEvent::ToolResult {
                session_id: id(7),
                id: String::new(),
                name: String::new(),
                result: serde_json::Value::Null,
            },
        ),
        (
            "ToolSideEffect",
            AgentEvent::ToolSideEffect {
                session_id: id(8),
                effect: ToolSideEffect::FileChanged {
                    path: PathBuf::from("a.md"),
                },
            },
        ),
        (
            "DebugEntry",
            AgentEvent::DebugEntry {
                session_id: id(9),
                entry: sample_debug_entry(),
            },
        ),
        (
            "TokenUsage",
            AgentEvent::TokenUsage {
                session_id: id(10),
                usage: sample_usage(),
            },
        ),
        (
            "Failed",
            AgentEvent::Failed {
                session_id: id(11),
                error: "boom".into(),
            },
        ),
    ];
    assert_eq!(
        cases.len(),
        11,
        "the table must stay in sync with the number of AgentEvent variants"
    );

    for (index, (name, event)) in cases.iter().enumerate() {
        assert_eq!(
            event.session_id(),
            id(index as u128 + 1),
            "session_id() of {name}"
        );
    }
}

#[test]
fn test_file_side_effects_publish_changed_to_the_file_bus() {
    let cases = [
        (
            "FileChanged",
            ToolSideEffect::FileChanged {
                path: PathBuf::from("notes/a.md"),
            },
        ),
        (
            "FileCreated",
            ToolSideEffect::FileCreated {
                path: PathBuf::from("notes/b.md"),
                tags: vec!["tag".into()],
            },
        ),
    ];

    for (name, effect) in cases {
        let session_id = Uuid::new_v4();
        let agent_bus: Bus<AgentEvent> = Bus::new();
        let agent_reader = agent_bus.subscribe();
        let file_bus: Bus<FileChanged> = Bus::new();
        let file_reader = file_bus.subscribe();
        let observer = BusAgentEventObserver::with_file_bus(session_id, agent_bus, file_bus);
        let path = match &effect {
            ToolSideEffect::FileCreated { path, .. } | ToolSideEffect::FileChanged { path } => {
                path.clone()
            }
        };

        observer.on_tool_side_effect(effect.clone());

        assert_eq!(
            drain(&file_reader),
            vec![FileChanged::updated_one(path.clone())],
            "{name} must publish exactly one [FileChanged][Changed] for {path:?} \
             (FileCreated is announced as Changed, not Added)"
        );

        let events = drain(&agent_reader);
        assert_eq!(
            events.len(),
            1,
            "{name} must publish exactly one agent event"
        );
        match &events[0] {
            AgentEvent::ToolSideEffect {
                session_id: emitted,
                effect: forwarded,
            } => {
                assert_eq!(
                    *emitted, session_id,
                    "{name} must carry the observer session id"
                );
                assert_eq!(
                    forwarded, &effect,
                    "{name} must forward the side effect unchanged"
                );
            }
            other => panic!("{name} must emit ToolSideEffect, got {other:?}"),
        }
    }
}

#[test]
fn test_side_effect_without_file_bus_only_emits_the_agent_event() {
    let session_id = Uuid::new_v4();
    let agent_bus: Bus<AgentEvent> = Bus::new();
    let agent_reader = agent_bus.subscribe();
    let observer = BusAgentEventObserver::new(session_id, agent_bus);

    observer.on_tool_side_effect(ToolSideEffect::FileCreated {
        path: PathBuf::from("notes/b.md"),
        tags: vec![],
    });

    let events = drain(&agent_reader);
    assert_eq!(
        events.len(),
        1,
        "the side effect must still reach the agent bus when no file bus is wired up"
    );
    assert!(
        matches!(events[0], AgentEvent::ToolSideEffect { .. }),
        "expected ToolSideEffect, got {:?}",
        events[0]
    );
}

#[test]
fn test_on_file_changed_reuses_the_side_effect_path() {
    let session_id = Uuid::new_v4();
    let agent_bus: Bus<AgentEvent> = Bus::new();
    let agent_reader = agent_bus.subscribe();
    let file_bus: Bus<FileChanged> = Bus::new();
    let file_reader = file_bus.subscribe();
    let observer = BusAgentEventObserver::with_file_bus(session_id, agent_bus, file_bus);

    OnFileChanged::on_file_changed(&observer, Path::new("notes/c.md"));

    assert_eq!(
        drain(&file_reader),
        vec![FileChanged::updated_one(PathBuf::from("notes/c.md"))],
        "on_file_changed must publish [FileChanged][Changed] for the path"
    );

    let events = drain(&agent_reader);
    assert_eq!(
        events.len(),
        1,
        "on_file_changed must emit exactly one agent event"
    );
    match &events[0] {
        AgentEvent::ToolSideEffect {
            effect: ToolSideEffect::FileChanged { path },
            ..
        } => {
            assert_eq!(
                path,
                &PathBuf::from("notes/c.md"),
                "the wrapped side effect must carry the reported path"
            );
        }
        other => panic!("expected a FileChanged side effect, got {other:?}"),
    }
}

#[test]
fn test_composite_forwards_the_remaining_observer_methods() {
    let obs1 = Arc::new(crate::agent::events::RecordingObserver::new());
    let obs2 = Arc::new(crate::agent::events::RecordingObserver::new());
    let composite = CompositeAgentEventObserver::new(vec![obs1.clone(), obs2.clone()]);

    let entry = sample_debug_entry();
    let usage = sample_usage();
    let effect = ToolSideEffect::FileChanged {
        path: PathBuf::from("notes/a.md"),
    };

    composite.on_tool_call_started("id".into(), "grep".into(), serde_json::json!({}));
    composite.on_tool_result(
        "id".into(),
        "grep".into(),
        serde_json::json!({ "ok": true }),
    );
    composite.on_tool_side_effect(effect.clone());
    composite.on_debug_entry(entry.clone());
    composite.on_token_usage(usage.clone());
    composite.on_failed("boom".into());

    let expected = vec![
        AgentObserverEvent::ToolCallStarted {
            id: "id".into(),
            name: "grep".into(),
            args: serde_json::json!({}),
        },
        AgentObserverEvent::ToolResult {
            id: "id".into(),
            name: "grep".into(),
            result: serde_json::json!({ "ok": true }),
        },
        AgentObserverEvent::ToolSideEffect(effect.clone()),
        AgentObserverEvent::DebugEntry(entry.clone()),
        AgentObserverEvent::TokenUsage(usage.clone()),
        AgentObserverEvent::Failed("boom".into()),
    ];
    assert_eq!(
        obs1.events(),
        expected,
        "first observer must see every forwarded call"
    );
    assert_eq!(
        obs2.events(),
        expected,
        "second observer must see the identical sequence"
    );
}

#[test]
fn test_composite_without_observers_is_a_no_op() {
    // An empty composite must accept the full observer protocol: a panic
    // here would take the agent turn loop down with it.
    let composite = CompositeAgentEventObserver::new(Vec::new());

    composite.on_session_started();
    composite.on_status(AgentStatus::Done);
    composite.on_thinking("t".into());
    composite.on_content_delta("c".into());
    composite.on_tool_call_started("id".into(), "n".into(), serde_json::Value::Null);
    composite.on_tool_result("id".into(), "n".into(), serde_json::Value::Null);
    composite.on_tool_side_effect(ToolSideEffect::FileChanged {
        path: PathBuf::from("a.md"),
    });
    composite.on_debug_entry(sample_debug_entry());
    composite.on_token_usage(sample_usage());
    composite.on_failed("e".into());
    composite.on_session_finished(Vec::new());
}
