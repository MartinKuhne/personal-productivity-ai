//! Unit tests for the bus → iced subscription bridge ([TASK-019]).

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::Arc;

use iced::futures::StreamExt;
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

use super::{
    BackgroundReceiver, BusReceivers, InboundBusEvent, SharedReceiver, agent_stream,
    background_stream, bus_event_to_message, close_request_to_message, config_stream,
    escape_to_message, file_stream, keyboard_events, modifiers_to_message, resize_event_to_message,
    subscription,
};
use crate::agent::events::{
    AgentDebugEntry, AgentStatus, DebugEntryKind, DebugEntryRow, TokenUsageInfo, ToolSideEffect,
};
use crate::bus::core::Bus;
use crate::bus::events::{
    AgentEvent, BackgroundEvent, BackgroundLogEntry, ConfigArrived, FileEvent, FileEventKind,
    FsEvent, LogCategory, McpAuthEvent, ProcessEvent,
};
use crate::config::AppConfig;
use crate::ui_iced::message::{BusSource, InboundMessage, Message, OverlayMessage};

fn sample_path() -> PathBuf {
    PathBuf::from("workspace/notes.md")
}

fn sample_debug_entry() -> AgentDebugEntry {
    AgentDebugEntry {
        turn: 1,
        timestamp: chrono::Local::now(),
        kind: DebugEntryKind::Outgoing,
        summary: "turn 1 outgoing".to_string(),
        content: Some(serde_json::json!({ "role": "user" })),
        row_type: DebugEntryRow::Entry,
    }
}

/// Every `AgentEvent` variant the bus can carry.
fn all_agent_events() -> Vec<AgentEvent> {
    let session_id = Uuid::new_v4();
    vec![
        AgentEvent::SessionStarted { session_id },
        AgentEvent::SessionFinished {
            session_id,
            history: vec![serde_json::json!({ "role": "user", "content": "hi" })],
        },
        AgentEvent::Status {
            session_id,
            status: AgentStatus::AwaitingLlm,
        },
        AgentEvent::Thinking {
            session_id,
            text: "reasoning".to_string(),
        },
        AgentEvent::ContentDelta {
            session_id,
            text: "delta".to_string(),
        },
        AgentEvent::ToolCallStarted {
            session_id,
            id: "call-1".to_string(),
            name: "read_file".to_string(),
            args: serde_json::json!({ "path": "a.md" }),
        },
        AgentEvent::ToolResult {
            session_id,
            id: "call-1".to_string(),
            name: "read_file".to_string(),
            result: serde_json::json!({ "ok": true }),
        },
        AgentEvent::ToolSideEffect {
            session_id,
            effect: ToolSideEffect::FileChanged {
                path: sample_path(),
            },
        },
        AgentEvent::DebugEntry {
            session_id,
            entry: sample_debug_entry(),
        },
        AgentEvent::TokenUsage {
            session_id,
            usage: TokenUsageInfo {
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
                cached_tokens: Some(4),
                reasoning_tokens: Some(6),
            },
        },
        AgentEvent::Failed {
            session_id,
            error: "stream reset".to_string(),
        },
    ]
}

/// Every `BackgroundEvent` variant the channel can carry.
fn all_background_events() -> Vec<BackgroundEvent> {
    vec![
        BackgroundEvent::Fs(FsEvent::FileParsed {
            path: sample_path(),
            tags: vec!["tag".to_string()],
        }),
        BackgroundEvent::Fs(FsEvent::DirParsed {
            path: PathBuf::from("workspace"),
        }),
        BackgroundEvent::Fs(FsEvent::FileModified {
            path: sample_path(),
            tags: vec![],
        }),
        BackgroundEvent::Fs(FsEvent::FileDeleted {
            path: sample_path(),
        }),
        BackgroundEvent::Fs(FsEvent::Finished),
        BackgroundEvent::Fs(FsEvent::FinishedWithoutWatcher),
        BackgroundEvent::Process(ProcessEvent::LogEntry(BackgroundLogEntry::new(
            LogCategory::Indexer,
            "indexed 12 files".to_string(),
        ))),
        BackgroundEvent::Process(ProcessEvent::FileLoaded {
            path: sample_path(),
            content: Ok("# body".to_string()),
        }),
        BackgroundEvent::Process(ProcessEvent::FileLoaded {
            path: sample_path(),
            content: Err("permission denied".to_string()),
        }),
        BackgroundEvent::McpAuth(McpAuthEvent::Completed {
            server_name: "mcp-1".to_string(),
            error: None,
        }),
        BackgroundEvent::McpAuth(McpAuthEvent::Completed {
            server_name: "mcp-1".to_string(),
            error: Some("access denied".to_string()),
        }),
    ]
}

fn hash_of<T: Hash>(value: &T) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn assert_send_static<T: Send + 'static>() {}

// ---------------------------------------------------------------------------
// Pure mapping — bus event → Message
// ---------------------------------------------------------------------------

#[test]
fn test_config_arrived_maps_to_inbound_config() {
    let message = bus_event_to_message(InboundBusEvent::Config(Box::new(ConfigArrived::new(
        AppConfig::default(),
    ))))
    .expect("the configuration handoff must reach the UI");

    match message {
        Message::Inbound(InboundMessage::ConfigArrived(config)) => {
            assert_eq!(
                config.inline_editor_enabled,
                AppConfig::default().inline_editor_enabled
            );
        }
        other => panic!("expected Inbound(ConfigArrived), got {other:?}"),
    }
}

#[test]
fn test_every_file_event_kind_maps_to_file_changed() {
    for kind in [
        FileEventKind::Discovered,
        FileEventKind::Updated,
        FileEventKind::Removed,
        FileEventKind::DirDiscovered,
        FileEventKind::DirRemoved,
    ] {
        let event = FileEvent {
            kind,
            paths: vec![sample_path()],
        };
        let message = bus_event_to_message(InboundBusEvent::File(event.clone()))
            .expect("file events must map");
        assert_eq!(
            message,
            Message::Inbound(InboundMessage::FileChanged(event)),
            "FileEventKind::{kind:?} must arrive unchanged"
        );
    }
}

#[test]
fn test_every_agent_event_variant_maps_to_the_agent_inbound_lane() {
    for event in all_agent_events() {
        let session_id = event.session_id();
        let message = bus_event_to_message(InboundBusEvent::Agent(event.clone()))
            .expect("agent events must map");
        match message {
            Message::Inbound(InboundMessage::Agent(carried)) => {
                assert_eq!(
                    carried.session_id(),
                    session_id,
                    "the session id must survive the mapping (FR-003 routing)"
                );
            }
            other => panic!("expected Inbound(Agent), got {other:?}"),
        }
    }
}

#[test]
fn test_every_background_event_variant_maps_to_the_background_inbound_lane() {
    for event in all_background_events() {
        let message = bus_event_to_message(InboundBusEvent::Background(event.clone()))
            .expect("background events must map");
        assert!(
            matches!(message, Message::Inbound(InboundMessage::Background(_))),
            "expected Inbound(Background), got {message:?}"
        );
    }
}

#[test]
fn test_dropped_report_maps_to_events_dropped_with_count() {
    for source in [
        BusSource::Config,
        BusSource::File,
        BusSource::Agent,
        BusSource::Background,
    ] {
        let message = bus_event_to_message(InboundBusEvent::Dropped { source, count: 17 })
            .expect("a loss report must reach the UI");
        assert_eq!(
            message,
            Message::Inbound(InboundMessage::EventsDropped { source, count: 17 }),
            "the dropped count must survive the mapping ([FMD-072])"
        );
    }
}

#[test]
fn test_mapping_is_total_over_every_current_bus_event() {
    let mut total = 0usize;
    for event in all_agent_events() {
        assert!(bus_event_to_message(InboundBusEvent::Agent(event)).is_some());
        total += 1;
    }
    for event in all_background_events() {
        assert!(bus_event_to_message(InboundBusEvent::Background(event)).is_some());
        total += 1;
    }
    for kind in [
        FileEventKind::Discovered,
        FileEventKind::Updated,
        FileEventKind::Removed,
        FileEventKind::DirDiscovered,
        FileEventKind::DirRemoved,
    ] {
        assert!(
            bus_event_to_message(InboundBusEvent::File(FileEvent {
                kind,
                paths: vec![sample_path()],
            }))
            .is_some()
        );
        total += 1;
    }
    assert!(
        bus_event_to_message(InboundBusEvent::Config(Box::new(ConfigArrived::new(
            AppConfig::default()
        ))))
        .is_some()
    );
    total += 1;
    assert!(
        bus_event_to_message(InboundBusEvent::Dropped {
            source: BusSource::Agent,
            count: 1,
        })
        .is_some()
    );
    total += 1;

    assert_eq!(
        total, 29,
        "11 agent + 11 background + 5 file + 1 config + 1 lag report"
    );
}

#[test]
fn test_inbound_clone_preserves_delivery_identity() {
    let message = Message::Inbound(InboundMessage::Agent(Arc::new(
        AgentEvent::SessionStarted {
            session_id: Uuid::new_v4(),
        },
    )));
    assert_eq!(
        message.clone(),
        message,
        "cloning must preserve the shared delivery instance"
    );
}

#[test]
fn test_two_independent_deliveries_of_equal_data_are_not_equal() {
    let session_id = Uuid::new_v4();
    let first = bus_event_to_message(InboundBusEvent::Agent(AgentEvent::Thinking {
        session_id,
        text: "same".to_string(),
    }))
    .expect("maps");
    let second = bus_event_to_message(InboundBusEvent::Agent(AgentEvent::Thinking {
        session_id,
        text: "same".to_string(),
    }))
    .expect("maps");

    assert_ne!(
        first, second,
        "two deliveries are two deliveries — a dedup check must not merge them"
    );
}

#[test]
fn test_file_events_compare_by_value() {
    let a = bus_event_to_message(InboundBusEvent::File(FileEvent::updated_one(sample_path())))
        .expect("maps");
    let b = bus_event_to_message(InboundBusEvent::File(FileEvent::updated_one(sample_path())))
        .expect("maps");
    assert_eq!(
        a, b,
        "`FileEvent` supports `PartialEq`, so equal payloads compare equal"
    );
}

#[test]
fn test_inbound_variants_of_different_kinds_are_not_equal() {
    let dropped = bus_event_to_message(InboundBusEvent::Dropped {
        source: BusSource::Agent,
        count: 1,
    })
    .expect("maps");
    let file = bus_event_to_message(InboundBusEvent::File(FileEvent::updated_one(sample_path())))
        .expect("maps");
    assert_ne!(dropped, file);
    assert_ne!(file, dropped);
}

#[test]
fn test_events_dropped_distinguishes_source_and_count() {
    let agent = bus_event_to_message(InboundBusEvent::Dropped {
        source: BusSource::Agent,
        count: 1,
    })
    .expect("maps");
    let file = bus_event_to_message(InboundBusEvent::Dropped {
        source: BusSource::File,
        count: 1,
    })
    .expect("maps");
    let more = bus_event_to_message(InboundBusEvent::Dropped {
        source: BusSource::Agent,
        count: 2,
    })
    .expect("maps");
    assert_ne!(agent, file);
    assert_ne!(agent, more);
}

// ---------------------------------------------------------------------------
// Subscription identity — what keeps the runtime from respawning a stream
// ---------------------------------------------------------------------------

#[test]
fn test_shared_receiver_clones_share_one_identity() {
    let bus: Bus<FileEvent> = Bus::new();
    let handle = SharedReceiver::from_bus(&bus);
    assert_eq!(
        hash_of(&handle),
        hash_of(&handle.clone()),
        "clones must key the same iced subscription"
    );
}

#[test]
fn test_distinct_shared_receivers_have_distinct_identities() {
    let bus: Bus<FileEvent> = Bus::new();
    let first = SharedReceiver::from_bus(&bus);
    let second = SharedReceiver::from_bus(&bus);
    assert_ne!(
        hash_of(&first),
        hash_of(&second),
        "two independent subscriptions must not collapse into one"
    );
}

#[test]
fn test_background_receiver_clones_share_one_identity() {
    let (_sender, receiver) = mpsc::unbounded_channel::<BackgroundEvent>();
    let handle = BackgroundReceiver::from_unbounded(receiver);
    assert_eq!(hash_of(&handle), hash_of(&handle.clone()));
}

// ---------------------------------------------------------------------------
// Live stream wiring — headless, deterministic (no timeouts needed)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_config_stream_delivers_the_mapped_message() {
    let bus: Bus<ConfigArrived> = Bus::new();
    let handle = SharedReceiver::from_bus(&bus);
    bus.publish(ConfigArrived::new(AppConfig::default()));

    let message = config_stream(&handle)
        .next()
        .await
        .expect("the published config must arrive");
    assert!(matches!(
        message,
        Message::Inbound(InboundMessage::ConfigArrived(_))
    ));
}

#[tokio::test]
async fn test_file_stream_delivers_the_mapped_message() {
    let bus: Bus<FileEvent> = Bus::new();
    let handle = SharedReceiver::from_bus(&bus);
    let event = FileEvent::discovered_one(sample_path());
    bus.publish(event.clone());

    let message = file_stream(&handle)
        .next()
        .await
        .expect("the published file event must arrive");
    assert_eq!(
        message,
        Message::Inbound(InboundMessage::FileChanged(event))
    );
}

#[tokio::test]
async fn test_agent_stream_delivers_the_mapped_message() {
    let bus: Bus<AgentEvent> = Bus::new();
    let handle = SharedReceiver::from_bus(&bus);
    let session_id = Uuid::new_v4();
    bus.publish(AgentEvent::ContentDelta {
        session_id,
        text: "hello".to_string(),
    });

    let message = agent_stream(&handle)
        .next()
        .await
        .expect("the published agent delta must arrive");
    match message {
        Message::Inbound(InboundMessage::Agent(carried)) => {
            assert_eq!(carried.session_id(), session_id);
        }
        other => panic!("expected Inbound(Agent), got {other:?}"),
    }
}

#[tokio::test]
async fn test_background_stream_delivers_the_mapped_message() {
    let (sender, receiver) = mpsc::unbounded_channel::<BackgroundEvent>();
    let handle = BackgroundReceiver::from_unbounded(receiver);
    sender
        .send(BackgroundEvent::Fs(FsEvent::Finished))
        .expect("send");

    let message = background_stream(&handle)
        .next()
        .await
        .expect("the bridged background event must arrive");
    assert!(matches!(
        message,
        Message::Inbound(InboundMessage::Background(_))
    ));
}

#[tokio::test]
async fn test_from_std_bridges_the_blocking_channel() {
    let (sender, receiver) = std::sync::mpsc::channel::<BackgroundEvent>();
    let handle = BackgroundReceiver::from_std(receiver);
    sender
        .send(BackgroundEvent::Process(ProcessEvent::LogEntry(
            BackgroundLogEntry::new(LogCategory::Batch, "batch done".to_string()),
        )))
        .expect("send");

    let message = background_stream(&handle)
        .next()
        .await
        .expect("the forwarding thread must deliver the event");
    match message {
        Message::Inbound(InboundMessage::Background(event)) => match &*event {
            BackgroundEvent::Process(ProcessEvent::LogEntry(entry)) => {
                assert_eq!(entry.message, "batch done");
                assert_eq!(entry.category, LogCategory::Batch);
            }
            other => panic!("expected a Process::LogEntry, got {other:?}"),
        },
        other => panic!("expected Inbound(Background), got {other:?}"),
    }
}

#[tokio::test]
async fn test_lag_surfaces_as_events_dropped_then_resynchronises() {
    // Capacity 1: the second publish evicts the unread first one and the
    // receiver reports the loss — the same `Lagged(n)` the legacy UI drain loops
    // read off `BusReader::try_recv_exposing_lag`.
    let (sender, receiver) = broadcast::channel::<FileEvent>(1);
    let handle = SharedReceiver::new(receiver);
    let evicted = FileEvent::updated_one(PathBuf::from("evicted.md"));
    let survivor = FileEvent::updated_one(PathBuf::from("survivor.md"));
    sender.send(evicted).expect("send");
    sender.send(survivor.clone()).expect("send");

    let mut stream = file_stream(&handle);
    assert_eq!(
        stream.next().await.expect("a lag report"),
        Message::Inbound(InboundMessage::EventsDropped {
            source: BusSource::File,
            count: 1,
        }),
        "the dropped count must reach the UI ([FMD-067], [FMD-072])"
    );
    assert_eq!(
        stream.next().await.expect("the next valid event"),
        Message::Inbound(InboundMessage::FileChanged(survivor)),
        "the stream must keep running and re-synchronise on the next event"
    );
}

#[tokio::test]
async fn test_broadcast_stream_ends_when_the_bus_is_closed() {
    let bus: Bus<FileEvent> = Bus::new();
    let handle = SharedReceiver::from_bus(&bus);
    drop(bus);

    let mut stream = file_stream(&handle);
    assert!(
        stream.next().await.is_none(),
        "a closed bus must end the stream, not spin"
    );
}

#[tokio::test]
async fn test_background_stream_ends_when_the_sender_is_dropped() {
    let (sender, receiver) = mpsc::unbounded_channel::<BackgroundEvent>();
    let handle = BackgroundReceiver::from_unbounded(receiver);
    drop(sender);

    let mut stream = background_stream(&handle);
    assert!(stream.next().await.is_none());
}

#[tokio::test]
async fn test_stream_delivers_events_in_publish_order() {
    let bus: Bus<FileEvent> = Bus::new();
    let handle = SharedReceiver::from_bus(&bus);
    let paths: Vec<PathBuf> = (0..4).map(|i| PathBuf::from(format!("f{i}.md"))).collect();
    for path in &paths {
        bus.publish(FileEvent::discovered_one(path.clone()));
    }

    // Everything queued before the stream wakes is coalesced into **one**
    // batch ([FMD-060]) that preserves publication order ([FMD-066]).
    let mut stream = file_stream(&handle);
    assert_eq!(
        stream.next().await.expect("a coalesced batch"),
        Message::Inbound(InboundMessage::FileChangedBatch(
            paths
                .iter()
                .map(|path| FileEvent::discovered_one(path.clone()))
                .collect()
        )),
        "publication order per publisher must be preserved ([FMD-066])"
    );
}

#[tokio::test]
async fn test_file_stream_coalesces_events_queued_while_it_sleeps() {
    let bus: Bus<FileEvent> = Bus::new();
    let handle = SharedReceiver::from_bus(&bus);

    // One event wakes the stream...
    let mut stream = file_stream(&handle);
    let first = FileEvent::discovered_one(PathBuf::from("first.md"));
    bus.publish(first.clone());
    assert_eq!(
        stream.next().await.expect("the lone event"),
        Message::Inbound(InboundMessage::FileChanged(first)),
        "a lone event must ride the single-event arm, not a batch"
    );

    // ...and a burst published while the stream sleeps arrives as one batch.
    let burst: Vec<FileEvent> = (0..3)
        .map(|i| FileEvent::updated_one(PathBuf::from(format!("burst{i}.md"))))
        .collect();
    for event in &burst {
        bus.publish(event.clone());
    }
    assert_eq!(
        stream.next().await.expect("the coalesced burst"),
        Message::Inbound(InboundMessage::FileChangedBatch(burst)),
        "a queued burst must cost exactly one message ([FMD-060])"
    );
}

#[tokio::test]
async fn test_background_stream_coalesces_a_queued_burst() {
    let (sender, receiver) = mpsc::unbounded_channel::<BackgroundEvent>();
    let handle = BackgroundReceiver::from_unbounded(receiver);
    let burst = vec![
        BackgroundEvent::Fs(FsEvent::DirParsed {
            path: PathBuf::from("dir"),
        }),
        BackgroundEvent::Fs(FsEvent::FileParsed {
            path: PathBuf::from("a.md"),
            tags: vec![],
        }),
        BackgroundEvent::Fs(FsEvent::Finished),
    ];
    for event in burst.clone() {
        sender.send(event).expect("send");
    }

    let mut stream = background_stream(&handle);
    let message = stream.next().await.expect("the coalesced burst");
    match message {
        Message::Inbound(InboundMessage::BackgroundBatch(events)) => {
            assert_eq!(events.len(), 3, "the whole queued burst must coalesce");
            assert!(
                matches!(&*events[0], BackgroundEvent::Fs(FsEvent::DirParsed { .. })),
                "order must be preserved ([FMD-066])"
            );
            assert!(
                matches!(&*events[2], BackgroundEvent::Fs(FsEvent::Finished)),
                "order must be preserved ([FMD-066])"
            );
        }
        other => panic!("expected Inbound(BackgroundBatch), got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// Subscription construction
// ---------------------------------------------------------------------------

#[test]
fn test_subscription_builds_from_receivers() {
    let (sender, receiver) = std::sync::mpsc::channel::<BackgroundEvent>();
    drop(sender);
    let receivers = BusReceivers::new(
        &Bus::<ConfigArrived>::new(),
        &Bus::<FileEvent>::new(),
        &Bus::<AgentEvent>::new(),
        receiver,
    );

    // The subscription itself is only handed to the runtime; what must be
    // `Send` is the stream the runtime builds from it, asserted below.
    let built = subscription(&receivers);
    drop(built);

    // Building twice must be safe — the runtime calls `subscription()` per
    // update and expects a stable, cheap value back.
    let again = subscription(&receivers);
    drop(again);

    assert_send_static::<SharedReceiver<ConfigArrived>>();
    assert_send_static::<SharedReceiver<FileEvent>>();
    assert_send_static::<SharedReceiver<AgentEvent>>();
    assert_send_static::<BackgroundReceiver>();
}

#[test]
fn test_bus_receivers_clone_keeps_every_identity_stable() {
    let (sender, receiver) = std::sync::mpsc::channel::<BackgroundEvent>();
    drop(sender);
    let receivers = BusReceivers::new(
        &Bus::<ConfigArrived>::new(),
        &Bus::<FileEvent>::new(),
        &Bus::<AgentEvent>::new(),
        receiver,
    );
    let clone = receivers.clone();

    assert_eq!(hash_of(&receivers.config), hash_of(&clone.config));
    assert_eq!(hash_of(&receivers.file), hash_of(&clone.file));
    assert_eq!(hash_of(&receivers.agent), hash_of(&clone.agent));
    assert_eq!(hash_of(&receivers.background), hash_of(&clone.background));
}

// --- winit window lane ([TASK-021], DEC-006) --------------------------------

/// Regression: the launch chain sets `exit_on_close_request: false`,
/// so nothing else in the runtime handles the title-bar close button.
/// The `window::close_requests` lane must map onto exactly the
/// `Message::Window(WindowMessage::CloseRequested(id))` report the
/// DEC-006 `reduce_window` save-and-close arm matches on — the former
/// build had the reducer but no subscription feeding it, so clicking
/// the close button did nothing.
#[test]
fn test_close_request_maps_to_the_window_message() {
    let id = iced::window::Id::unique();
    assert_eq!(
        close_request_to_message(id),
        Message::Window(super::WindowMessage::CloseRequested(id)),
        "the subscription must carry the WindowMessage variant the reducer matches"
    );
}

/// Regression: `AppState::viewport` only follows real window resizes
/// through the `window::resize_events` lane, which the reducer arm was
/// written against but nothing delivered before.
#[test]
fn test_resize_event_maps_to_the_window_message() {
    let id = iced::window::Id::unique();
    let size = iced::Size::new(800.0, 600.0);
    assert_eq!(
        resize_event_to_message((id, size)),
        Message::Window(super::WindowMessage::Resized { id, size })
    );
}

// --- keyboard lane (UI-056 Esc dismissal, UI-022 Shift tracking) ------------

/// A `KeyPressed` runtime event for `key` carrying `modifiers`.
fn pressed(key: iced::keyboard::Key, modifiers: iced::keyboard::Modifiers) -> iced::Event {
    iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: iced::keyboard::Location::Standard,
        modifiers,
        text: None,
        repeat: false,
    })
}

/// A `KeyReleased` runtime event for `key` carrying `modifiers`.
fn released(key: iced::keyboard::Key, modifiers: iced::keyboard::Modifiers) -> iced::Event {
    iced::Event::Keyboard(iced::keyboard::Event::KeyReleased {
        key: key.clone(),
        modified_key: key,
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: iced::keyboard::Location::Standard,
        modifiers,
    })
}

fn escape() -> iced::keyboard::Key {
    iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape)
}

fn letter_a() -> iced::keyboard::Key {
    iced::keyboard::Key::Character("a".into())
}

/// UI-056: `Esc` with nothing under it focused must dismiss.
#[test]
fn test_escape_ignored_maps_to_the_dismiss_message() {
    assert_eq!(
        escape_to_message(
            pressed(escape(), iced::keyboard::Modifiers::empty()),
            iced::event::Status::Ignored,
            iced::window::Id::unique(),
        ),
        Some(Message::Overlay(OverlayMessage::EscapePressed)),
        "an ignored Escape must still reach the overlay reducer"
    );
}

/// UI-056: `Esc` **captured by a focused `text_editor`** must still dismiss.
/// This is the case `iced::event::listen` cannot serve — it filters to
/// `Status::Ignored` only (`iced_futures-0.14.0/src/event.rs:12-17`) — and
/// is the whole reason the lane uses `listen_with`.
#[test]
fn test_escape_captured_by_a_focused_widget_still_maps_to_the_dismiss_message() {
    assert_eq!(
        escape_to_message(
            pressed(escape(), iced::keyboard::Modifiers::empty()),
            iced::event::Status::Captured,
            iced::window::Id::unique(),
        ),
        Some(Message::Overlay(OverlayMessage::EscapePressed)),
        "a captured Escape must not be swallowed by the focused editor"
    );
}

/// UI-056: only Escape dismisses; any other key press is discarded so the
/// lane stays silent for ordinary typing.
#[test]
fn test_non_escape_key_press_maps_to_none() {
    for status in [iced::event::Status::Ignored, iced::event::Status::Captured] {
        assert_eq!(
            escape_to_message(
                pressed(letter_a(), iced::keyboard::Modifiers::SHIFT),
                status,
                iced::window::Id::unique(),
            ),
            None,
            "a non-Escape press must produce no dismiss under {status:?}"
        );
    }
}

/// UI-056: releasing `Esc` is not a dismissal — mapping it would close a
/// layer reopened while the key was still down.
#[test]
fn test_escape_release_maps_to_none() {
    for status in [iced::event::Status::Ignored, iced::event::Status::Captured] {
        assert_eq!(
            escape_to_message(
                released(escape(), iced::keyboard::Modifiers::empty()),
                status,
                iced::window::Id::unique(),
            ),
            None,
            "a KeyReleased Escape must produce no dismiss under {status:?}"
        );
    }
}

/// UI-056: non-keyboard runtime events are not dismisses either.
#[test]
fn test_escape_ignores_non_keyboard_events() {
    let events = [
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
        iced::Event::Window(iced::window::Event::CloseRequested),
    ];
    for event in events {
        assert_eq!(
            escape_to_message(
                event.clone(),
                iced::event::Status::Ignored,
                iced::window::Id::unique()
            ),
            None,
            "a non-keyboard event must produce no dismiss: {event:?}"
        );
    }
}

/// UI-022: a key event with Shift held reports `shift: true`.
#[test]
fn test_modifier_mapper_reports_shift_held() {
    let event = pressed(letter_a(), iced::keyboard::Modifiers::SHIFT);
    assert_eq!(
        modifiers_to_message(
            event,
            iced::event::Status::Ignored,
            iced::window::Id::unique()
        ),
        Some(Message::Overlay(OverlayMessage::ModifiersChanged {
            shift: true
        })),
        "Shift held must be reported as shift: true"
    );
}

/// UI-022: the same mapper on a key event without Shift reports `false`,
/// which is what clears the tracked bit on release.
#[test]
fn test_modifier_mapper_reports_shift_not_held() {
    let event = pressed(letter_a(), iced::keyboard::Modifiers::empty());
    assert_eq!(
        modifiers_to_message(
            event,
            iced::event::Status::Ignored,
            iced::window::Id::unique()
        ),
        Some(Message::Overlay(OverlayMessage::ModifiersChanged {
            shift: false
        })),
        "no Shift must be reported as shift: false"
    );
}

/// UI-022: releases are mapped too, and report the event's own bit rather
/// than an edge — that is what makes the tracked value self-correcting
/// after a missed release.
#[test]
fn test_modifier_mapper_tracks_releases_with_their_own_bit() {
    assert_eq!(
        modifiers_to_message(
            released(letter_a(), iced::keyboard::Modifiers::SHIFT),
            iced::event::Status::Captured,
            iced::window::Id::unique(),
        ),
        Some(Message::Overlay(OverlayMessage::ModifiersChanged {
            shift: true
        })),
        "a release still carrying SHIFT must report true"
    );
    assert_eq!(
        modifiers_to_message(
            released(escape(), iced::keyboard::Modifiers::empty()),
            iced::event::Status::Ignored,
            iced::window::Id::unique(),
        ),
        Some(Message::Overlay(OverlayMessage::ModifiersChanged {
            shift: false
        })),
        "even an Escape release refreshes the tracked bit"
    );
}

/// UI-022: only keyboard events carry a modifier report.
#[test]
fn test_modifier_mapper_ignores_non_keyboard_events() {
    let events = [
        iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left)),
        iced::Event::Window(iced::window::Event::CloseRequested),
    ];
    for event in events {
        assert_eq!(
            modifiers_to_message(
                event.clone(),
                iced::event::Status::Ignored,
                iced::window::Id::unique()
            ),
            None,
            "a non-keyboard event carries no modifier report: {event:?}"
        );
    }
}

/// The keyboard lane must be constructible and mergeable into the main
/// subscription without the two `listen_with` recipes collapsing into one
/// identity (they are keyed by their `fn` pointer, so they stay distinct).
#[test]
fn test_keyboard_lane_builds_and_merges_into_the_main_subscription() {
    let lane = keyboard_events();
    drop(lane);

    let (sender, receiver) = std::sync::mpsc::channel::<BackgroundEvent>();
    drop(sender);
    let receivers = BusReceivers::new(
        &Bus::<ConfigArrived>::new(),
        &Bus::<FileEvent>::new(),
        &Bus::<AgentEvent>::new(),
        receiver,
    );
    // Adding the keyboard lane must not break the whole-subscription build.
    let merged = subscription(&receivers);
    drop(merged);
}
