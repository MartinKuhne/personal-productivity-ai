//! Bus → iced subscription bridge ([TASK-019], FMD-065..FMD-067).
//!
//! Turns the application's existing producer channels into one
//! [`iced::Subscription<Message>`] the [TASK-022] runtime can hand to
//! `.subscription(...)`. The producers are unchanged: background work keeps
//! notifying, and this module is the single place the iced layer ever
//! touches a live receiver ([FMD-065] — notification runs one way,
//! background → UI; outbound user intent rides [`Message::Command`] and is
//! not this module's concern).
//!
//! # Verified iced 0.14 primitives
//!
//! | Primitive | Signature | Source |
//! | --- | --- | --- |
//! | `Subscription::run` | `pub fn run<S>(builder: fn() -> S) -> Self where S: Stream<Item = T> + MaybeSend + 'static` | `iced_futures-0.14.0/src/subscription.rs:182-191` |
//! | `Subscription::run_with` | `pub fn run_with<D, S>(data: D, builder: fn(&D) -> S) -> Self where D: Hash + 'static, S: Stream<Item = T> + MaybeSend + 'static` | `iced_futures-0.14.0/src/subscription.rs:198-208` |
//! | `Subscription::batch` | `pub fn batch(subscriptions: impl IntoIterator<Item = Subscription<T>>) -> Self` | `iced_futures-0.14.0/src/subscription.rs:212-221` |
//! | `Task::run` | `pub fn run<A>(stream: impl Stream<Item = A> + MaybeSend + 'static, f: impl Fn(A) -> T + MaybeSend + 'static) -> Self` | `iced_runtime-0.14.0/src/task.rs:61-69` |
//!
//! `Subscription::run` takes a **`fn() -> S` pointer**, so it cannot carry
//! captured state — a live receiver cannot be fed to it. `run_with` is
//! therefore the only usable form: the receiver handle is the hashed `data`
//! and the builder is a non-capturing `fn`. `D: Hash` is what lets the
//! runtime identify a subscription and build its stream **exactly once**
//! instead of respawning it on every `subscription()` call, which is why
//! [`SharedReceiver`] hashes by its `Arc` address: clones of the same
//! handle keep the identity stable across frames.
//!
//! Each builder returns a **boxed** stream
//! (`futures::stream::BoxStream<'static, Message>`, boxed with
//! `StreamExt::boxed`): an `impl Stream` return type cannot be coerced to
//! iced's `fn(&D) -> S` pointer, because the opaque type is "more general
//! than" the single `S` the call site has to infer.
//!
//! # Why no new crate
//!
//! `futures` is reachable through iced itself (`iced::futures`,
//! `iced-0.14.0/src/lib.rs:484`; `iced::stream`, `:485`), so the streams
//! here are built with `futures::stream::unfold` and no `async-stream` /
//! `tokio-stream` dependency was added.
//!
//! Note that iced's default executor for this build is
//! `futures::executor::ThreadPool` (`iced_futures-0.14.0/src/backend/native/thread_pool.rs:4`),
//! **not** tokio — iced's `tokio` feature is off. The `tokio::sync::*`
//! primitives used here (broadcast, mutex, unbounded mpsc) are
//! reactor-free and poll correctly on any executor, but `tokio::task::*`
//! would panic, so the `std::sync::mpsc` background channel is bridged by a
//! dedicated forwarding thread rather than `spawn_blocking`.
//!
//! # Event → `Message` mapping
//!
//! [`bus_event_to_message`] is the pure half; the stream builders below are
//! the only impure half.
//!
//! | Bus | Event | `Message` |
//! | --- | --- | --- |
//! | `Bus<ConfigArrived>` | `ConfigArrived { config }` | `Message::Inbound(InboundMessage::ConfigArrived(Arc<AppConfig>))` |
//! | `Bus<FileEvent>` | `FileEvent { kind, paths }` (all five `FileEventKind`s) | `Message::Inbound(InboundMessage::FileChanged(event))` |
//! | `Bus<AgentEvent>` | all 11 `AgentEvent` variants | `Message::Inbound(InboundMessage::Agent(Arc<AgentEvent>))` |
//! | `std::sync::mpsc<BackgroundEvent>` | `Fs(..)` / `Process(..)` / `McpAuth(..)` | `Message::Inbound(InboundMessage::Background(Arc<BackgroundEvent>))` |
//! | any bus, on `Lagged(n)` | dropped-event report | `Message::Inbound(InboundMessage::EventsDropped { source, count })` |
//! | winit window events | `CloseRequested(id)` / `Resized(id, size)` | `Message::Window(WindowMessage::CloseRequested(id))` / `Message::Window(WindowMessage::Resized { id, size })` — the `close_requests()` / `resize_events()` lanes; with `exit_on_close_request: false` the close lane is the only path to the DEC-006 save-and-close reduction |
//!
//! The bus payload enums do not implement `PartialEq` (see
//! [`InboundMessage`]'s docs), which is why the mapping lands in one
//! dedicated inbound channel instead of spreading across the `Agent`,
//! `Logs` and `Tree` panel channels.
//!
//! # Dropped events ([FMD-067], [FMD-072])
//!
//! The legacy UI build read the dropped-event count from
//! `BusReader::try_recv_exposing_lag() → BroadcastRecvError::Lagged(n)`
//! (`src/app/bus/core.rs:149-151`) and handled it in its drain loops
//! (`src/app/orchestrator.rs:96-103` for user commands, `:435-449` for
//! agent events). The async equivalent is
//! `broadcast::Receiver::recv() → Err(RecvError::Lagged(n))` — the same
//! count off the same tokio channel. This bridge does both things the legacy UI
//! loops did, and does them for **every** bus rather than only the agent
//! one: it emits a `tracing::warn!` carrying the count, and it forwards an
//! [`InboundMessage::EventsDropped`] so the reducer can surface the loss
//! and re-synchronise on the next valid event. The stream then keeps
//! running — a lag is not a disconnect.
//!
//! Unit tests live in the sibling `subscription_tests.rs` sidecar.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use iced::futures::StreamExt;
use tokio::sync::{Mutex as AsyncMutex, broadcast, mpsc};

use crate::bus::core::Bus;
use crate::bus::events::{AgentEvent, BackgroundEvent, ConfigArrived, FileEvent};
use crate::ui_iced::message::{BusSource, InboundMessage, Message, WindowMessage};

/// A bus event handed to the pure mapper [`bus_event_to_message`].
///
/// The union of every payload type that reaches the iced layer, so the
/// mapping is one testable function instead of four parallel ones. The
/// `Dropped` arm is not a bus payload: it is the lag report the streams
/// synthesise from `RecvError::Lagged(n)` ([FMD-072]).
#[derive(Debug, Clone)]
pub enum InboundBusEvent {
    /// The one-shot configuration handoff on `Bus<ConfigArrived>`
    /// (`src/app/bus/events/config.rs:10`, DEC-009). Boxed: `AppConfig`
    /// is ~800 bytes and would otherwise dominate the enum's layout.
    Config(Box<ConfigArrived>),
    /// A filesystem mutation on `Bus<FileEvent>`
    /// (`src/app/bus/events/file.rs:28`).
    File(FileEvent),
    /// An agent turn event on `Bus<AgentEvent>`
    /// (`src/app/bus/events/agent.rs:21`).
    Agent(AgentEvent),
    /// A typed background event on the `std::sync::mpsc` channel
    /// (`src/app/bus/events/typed.rs:119`).
    Background(BackgroundEvent),
    /// `count` events were discarded because a subscriber fell behind
    /// ([FMD-067]).
    Dropped {
        /// Which bus discarded them.
        source: BusSource,
        /// How many were discarded.
        count: u64,
    },
}

/// Pure event → message mapping: the whole notification vocabulary of the
/// bridge, testable without a live stream.
///
/// Returns `None` when an event must not reach the UI. Every payload
/// currently published on the four buses *is* UI-observable — the legacy UI
/// build reacted to each one — so the mapping is total today and `None` is
/// never produced. The `Option` is kept deliberately: it is the seam where
/// a future filter (throttling a chatty producer, suppressing another
/// session's events) can drop a class of events without touching the stream
/// plumbing, and the streams drop `None`s on their way through.
#[must_use]
pub fn bus_event_to_message(event: InboundBusEvent) -> Option<Message> {
    match event {
        InboundBusEvent::Config(arrived) => Some(Message::Inbound(InboundMessage::ConfigArrived(
            Arc::new(arrived.config),
        ))),
        InboundBusEvent::File(event) => Some(Message::Inbound(InboundMessage::FileChanged(event))),
        InboundBusEvent::Agent(event) => {
            Some(Message::Inbound(InboundMessage::Agent(Arc::new(event))))
        }
        InboundBusEvent::Background(event) => Some(Message::Inbound(InboundMessage::Background(
            Arc::new(event),
        ))),
        InboundBusEvent::Dropped { source, count } => {
            Some(Message::Inbound(InboundMessage::EventsDropped {
                source,
                count,
            }))
        }
    }
}

/// One eagerly-created, shared handle to a bus's broadcast receiver.
///
/// **Eager** matters: `Bus::publish` delivers to nobody if no subscriber
/// exists yet (`src/app/bus/core.rs:87-91`), and `ConfigArrived` is
/// published exactly once at startup. Subscribing at handle-construction
/// time — before the producer runs — is what keeps that event from vanishing.
///
/// Cloning shares the same underlying receiver, and [`Hash`] uses the `Arc`
/// address so every clone of the same handle keys the *same* iced
/// `Subscription`. The runtime therefore builds the stream once and does
/// not respawn it each time `subscription()` is called.
#[derive(Clone)]
pub struct SharedReceiver<T>
where
    T: Clone + Send + 'static,
{
    inner: Arc<AsyncMutex<broadcast::Receiver<T>>>,
}

impl<T: Clone + Send + 'static> SharedReceiver<T> {
    /// Wraps an existing broadcast receiver.
    pub fn new(receiver: broadcast::Receiver<T>) -> Self {
        Self {
            inner: Arc::new(AsyncMutex::new(receiver)),
        }
    }

    /// Subscribes to `bus` **now** and returns the shared handle.
    pub fn from_bus(bus: &Bus<T>) -> Self {
        Self::new(bus.subscribe_async())
    }

    /// Async lock on the underlying receiver.
    pub async fn lock(&self) -> tokio::sync::MutexGuard<'_, broadcast::Receiver<T>> {
        self.inner.lock().await
    }
}

/// Address-based identity for [`SharedReceiver`] — see the type docs.
///
/// Deliberately *not* value equality: the handle's identity is the channel
/// it reads, not the events inside it.
impl<T: Clone + Send + 'static> Hash for SharedReceiver<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.inner).hash(state);
    }
}

/// Handle to the `std::sync::mpsc` background-event channel, bridged onto
/// an async stream.
///
/// `std::sync::mpsc::Receiver::recv()` blocks, and this build's iced
/// executor is a `futures` thread pool with no `spawn_blocking`
/// (`iced_futures-0.14.0/src/backend/native/thread_pool.rs:4`). Blocking a
/// pool thread for the life of the app would starve it, so one dedicated
/// OS thread owns the blocking receive and forwards into a
/// reactor-free `tokio` unbounded channel that the stream polls.
///
/// The forwarding thread exits when the producer drops its sender
/// (`recv()` errors) or the bridge's sending half is closed.
#[derive(Clone)]
pub struct BackgroundReceiver {
    inner: Arc<AsyncMutex<mpsc::UnboundedReceiver<BackgroundEvent>>>,
}

impl BackgroundReceiver {
    /// Bridges the blocking `source` receiver onto an async channel.
    pub fn from_std(source: std::sync::mpsc::Receiver<BackgroundEvent>) -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        std::thread::spawn(move || {
            while let Ok(event) = source.recv() {
                if sender.send(event).is_err() {
                    break;
                }
            }
        });
        Self {
            inner: Arc::new(AsyncMutex::new(receiver)),
        }
    }

    /// Wraps an already-async unbounded receiver (used by tests to avoid a
    /// forwarding thread).
    pub fn from_unbounded(receiver: mpsc::UnboundedReceiver<BackgroundEvent>) -> Self {
        Self {
            inner: Arc::new(AsyncMutex::new(receiver)),
        }
    }

    /// Async lock on the underlying receiver.
    pub async fn lock(
        &self,
    ) -> tokio::sync::MutexGuard<'_, mpsc::UnboundedReceiver<BackgroundEvent>> {
        self.inner.lock().await
    }
}

/// Address-based identity for [`BackgroundReceiver`] — see
/// [`SharedReceiver`]'s `Hash` docs for the same reasoning.
impl Hash for BackgroundReceiver {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.inner).hash(state);
    }
}

/// Every live receiver the bridge reads, built once at startup.
///
/// Clone it into the app state and pass a reference to [`subscription`];
/// cloning is four `Arc` bumps and keeps every subscription identity
/// stable across frames.
#[derive(Clone)]
pub struct BusReceivers {
    /// `Bus<ConfigArrived>` — the DEC-009 configuration handoff.
    pub config: SharedReceiver<ConfigArrived>,
    /// `Bus<FileEvent>` — watcher / scan / tool / agent file mutations.
    pub file: SharedReceiver<FileEvent>,
    /// `Bus<AgentEvent>` — the agent's streaming turn events.
    pub agent: SharedReceiver<AgentEvent>,
    /// The `std::sync::mpsc<BackgroundEvent>` channel.
    pub background: BackgroundReceiver,
}

impl BusReceivers {
    /// Subscribes to all four producer channels eagerly.
    pub fn new(
        config_bus: &Bus<ConfigArrived>,
        file_bus: &Bus<FileEvent>,
        agent_bus: &Bus<AgentEvent>,
        background_rx: std::sync::mpsc::Receiver<BackgroundEvent>,
    ) -> Self {
        Self {
            config: SharedReceiver::from_bus(config_bus),
            file: SharedReceiver::from_bus(file_bus),
            agent: SharedReceiver::from_bus(agent_bus),
            background: BackgroundReceiver::from_std(background_rx),
        }
    }
}

/// Builds the single [`iced::Subscription`] that carries every inbound
/// background notification, one sub-subscription per bus merged with
/// [`iced::Subscription::batch`]
/// (`iced_futures-0.14.0/src/subscription.rs:212-221`).
///
/// Ready to be handed straight to `.subscription(...)` in [TASK-022]:
///
/// ```ignore
/// iced::application(App::boot, App::update, App::view)
///     .subscription(|app: &App| ui_iced::subscription::subscription(&app.bus_receivers))
/// ```
///
/// Each sub-subscription is `Subscription::run_with(handle, builder)` — the
/// only 0.14 form that can carry a live receiver, because
/// `Subscription::run` takes a bare `fn() -> S` pointer that cannot close
/// over one.
pub fn subscription(receivers: &BusReceivers) -> iced::Subscription<Message> {
    iced::Subscription::batch([
        iced::Subscription::run_with(receivers.config.clone(), config_stream),
        iced::Subscription::run_with(receivers.file.clone(), file_stream),
        iced::Subscription::run_with(receivers.agent.clone(), agent_stream),
        iced::Subscription::run_with(receivers.background.clone(), background_stream),
        // The winit window lane — the subscriptions the window reducers
        // ([TASK-009], DEC-006) were written against. The launch chain sets
        // `exit_on_close_request: false` ([APP] `ui_iced/app.rs`
        // `window_settings`), so *nothing* exists that handles the title-bar
        // close button unless these subscriptions deliver
        // [`WindowMessage::CloseRequested`] to
        // [`crate::ui_iced::update::reduce_window`]; they also keep
        // `AppState::viewport` in step with real window resizes. Both map
        // through `iced::Subscription::map` (`iced_futures-0.14.0/src/
        // subscription.rs:224-231`).
        window_events(),
    ])
}

/// The winit window-event lane ([TASK-021], DEC-006): close requests and
/// resizes, delivered by `iced::window::close_requests` /
/// `iced::window::resize_events`
/// (`iced_runtime-0.14.0/src/window.rs:249-268`) — the producers the
/// [`WindowMessage`](crate::ui_iced::message::WindowMessage) variant docs
/// name. Both map onto `Message::Window` via `Subscription::map`
/// (`iced_futures-0.14.0/src/subscription.rs:224-231`).
fn window_events() -> iced::Subscription<Message> {
    iced::Subscription::batch([
        iced::window::close_requests().map(close_request_to_message),
        iced::window::resize_events().map(resize_event_to_message),
    ])
}

/// Pure mapping of one `iced::window::close_requests` item onto its
/// [`Message::Window`] report. Testable without a live window: the id
/// is only carried, never addressed, so a throwaway [`Id::unique()`]
/// value drives the test.
fn close_request_to_message(id: iced::window::Id) -> Message {
    Message::Window(WindowMessage::CloseRequested(id))
}

/// Pure mapping of one `iced::window::resize_events` item onto its
/// [`Message::Window`] report. Testable without a live window because
/// `iced::Size` is a plain struct.
fn resize_event_to_message((id, size): (iced::window::Id, iced::Size)) -> Message {
    Message::Window(WindowMessage::Resized { id, size })
}

/// Reports a lag on `source`: a `tracing::warn!` carrying the dropped
/// count ([FMD-072]) — the same shape the legacy UI drain loops used
/// (`src/app/orchestrator.rs:97-101`, `:436-440`).
fn warn_dropped(source: BusSource, count: u64) {
    tracing::warn!(
        name = "ui_iced.subscription.bus_lagged",
        source = ?source,
        dropped_events = count,
        "Subscription bridge fell behind a bus; {count} event(s) dropped"
    );
}

/// The `Bus<ConfigArrived>` lane.
///
/// Returns a boxed stream: `Subscription::run_with` wants a `fn(&D) -> S`
/// pointer, and an `impl Stream` return type cannot be coerced to one (the
/// opaque type is "more general than" the fn-pointer's single inferred `S`),
/// so the stream is boxed to a concrete `S`.
fn config_stream(
    handle: &SharedReceiver<ConfigArrived>,
) -> iced::futures::stream::BoxStream<'static, Message> {
    let inner = Arc::clone(&handle.inner);
    iced::futures::stream::unfold(inner, |inner| async move {
        let received = {
            let mut receiver = inner.lock().await;
            receiver.recv().await
        };
        match received {
            Ok(event) => Some((
                bus_event_to_message(InboundBusEvent::Config(Box::new(event))),
                inner,
            )),
            Err(broadcast::error::RecvError::Closed) => None,
            Err(broadcast::error::RecvError::Lagged(count)) => {
                warn_dropped(BusSource::Config, count);
                Some((
                    bus_event_to_message(InboundBusEvent::Dropped {
                        source: BusSource::Config,
                        count,
                    }),
                    inner,
                ))
            }
        }
    })
    .filter_map(iced::futures::future::ready)
    .boxed()
}

/// The `Bus<FileEvent>` lane.
fn file_stream(
    handle: &SharedReceiver<FileEvent>,
) -> iced::futures::stream::BoxStream<'static, Message> {
    let inner = Arc::clone(&handle.inner);
    iced::futures::stream::unfold(inner, |inner| async move {
        let received = {
            let mut receiver = inner.lock().await;
            receiver.recv().await
        };
        match received {
            Ok(event) => Some((bus_event_to_message(InboundBusEvent::File(event)), inner)),
            Err(broadcast::error::RecvError::Closed) => None,
            Err(broadcast::error::RecvError::Lagged(count)) => {
                warn_dropped(BusSource::File, count);
                Some((
                    bus_event_to_message(InboundBusEvent::Dropped {
                        source: BusSource::File,
                        count,
                    }),
                    inner,
                ))
            }
        }
    })
    .filter_map(iced::futures::future::ready)
    .boxed()
}

/// The `Bus<AgentEvent>` lane — the highest-volume channel of the four.
fn agent_stream(
    handle: &SharedReceiver<AgentEvent>,
) -> iced::futures::stream::BoxStream<'static, Message> {
    let inner = Arc::clone(&handle.inner);
    iced::futures::stream::unfold(inner, |inner| async move {
        let received = {
            let mut receiver = inner.lock().await;
            receiver.recv().await
        };
        match received {
            Ok(event) => Some((bus_event_to_message(InboundBusEvent::Agent(event)), inner)),
            Err(broadcast::error::RecvError::Closed) => None,
            Err(broadcast::error::RecvError::Lagged(count)) => {
                warn_dropped(BusSource::Agent, count);
                Some((
                    bus_event_to_message(InboundBusEvent::Dropped {
                        source: BusSource::Agent,
                        count,
                    }),
                    inner,
                ))
            }
        }
    })
    .filter_map(iced::futures::future::ready)
    .boxed()
}

/// The `BackgroundEvent` lane (indexer / watcher progress, worker logs,
/// file-load results, MCP OAuth completions).
fn background_stream(
    handle: &BackgroundReceiver,
) -> iced::futures::stream::BoxStream<'static, Message> {
    let inner = Arc::clone(&handle.inner);
    iced::futures::stream::unfold(inner, |inner| async move {
        let received = {
            let mut receiver = inner.lock().await;
            receiver.recv().await
        };
        received.map(|event| {
            (
                bus_event_to_message(InboundBusEvent::Background(event)),
                inner,
            )
        })
    })
    .filter_map(iced::futures::future::ready)
    .boxed()
}

#[cfg(test)]
#[path = "subscription_tests.rs"]
mod tests;
