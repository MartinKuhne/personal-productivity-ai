//! Bus → iced subscription bridge.
//!
//! Turns the application's producer channels into one
//! [`iced::Subscription<Message>`] the runtime hands to `.subscription(...)`.
//! Notification runs one way, background → UI; outbound user intent rides
//! [`Message::Command`] and is not this module's concern.
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
//! | `Bus<FileChanged>` | `[FileChanged] { kind, files }` (all three `FileChangedKind`s) | `Message::Inbound(InboundMessage::FileChanged(event))` |
//! | `Bus<DirectoryChanged>` | `[DirectoryChanged] { path, kind }` | `Message::Inbound(InboundMessage::DirectoryChanged(event))` |
//! | `Bus<AgentEvent>` | all 11 `AgentEvent` variants | `Message::Inbound(InboundMessage::Agent(Arc<AgentEvent>))` |
//! | `Bus<AgentEvent>` | coalesced burst | `Message::Inbound(InboundMessage::AgentBatch(Vec<Arc<AgentEvent>>))` |
//! | `std::sync::mpsc<BackgroundEvent>` | `Fs(..)` / `Process(..)` / `McpAuth(..)` | `Message::Inbound(InboundMessage::Background(Arc<BackgroundEvent>))` |
//! | any bus, on `Lagged(n)` | dropped-event report | `Message::Inbound(InboundMessage::EventsDropped { source, count })` |
//! | winit window events | `CloseRequested(id)` / `Resized(id, size)` | `Message::Window(WindowMessage::CloseRequested(id))` / `Message::Window(WindowMessage::Resized { id, size })` — the `close_requests()` / `resize_events()` lanes; with `exit_on_close_request: false` the close lane is the only path to the save-and-close reduction |
//! | runtime keyboard events | `KeyPressed { Escape }` | `Message::Overlay(OverlayMessage::EscapePressed)` — the `escape_to_message` lane; accepted under **both** `Status::Ignored` and `Status::Captured` so `Esc` still dismisses a modal while a `text_editor` holds focus (UI-056) |
//! | runtime keyboard events | `KeyPressed` / `KeyReleased` `modifiers.shift()` | `Message::Overlay(OverlayMessage::ModifiersChanged { shift })` — the `modifiers_to_message` lane, the UI-022 multi-select input |
//!
//! The keyboard lane is the one part of this bridge that reads the iced
//! **event stream** rather than a producer channel; see the
//! `keyboard_events` builder below for why it needs `listen_with` and not
//! `listen`.
//!
//! The bus payload enums do not implement `PartialEq` (see
//! [`InboundMessage`]'s docs), which is why the mapping lands in one
//! dedicated inbound channel instead of spreading across the `Agent`,
//! `Logs` and `Tree` panel channels.
//!
//! # Dropped events
//!
//! A lagging subscriber surfaces as
//! `broadcast::Receiver::recv() → Err(RecvError::Lagged(n))`. For **every**
//! bus the bridge reports the loss instead of swallowing it: it emits a
//! `tracing::warn!` carrying the count, and it forwards an
//! [`InboundMessage::EventsDropped`] so the reducer can surface the loss
//! and re-synchronise on the next valid event. The stream then keeps
//! running — a lag is not a disconnect.
//!
//! Unit tests live in the sibling `subscription_tests.rs` sidecar.

use futures::channel::mpsc::UnboundedReceiver;
use futures::lock::Mutex as FuturesMutex;
use futures::stream::StreamExt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use tokio::sync::{Mutex as AsyncMutex, broadcast};

use crate::bus::core::Bus;
use crate::bus::events::directory::DirectoryChanged;
use crate::bus::events::file::FileChanged;
use crate::bus::events::{AgentEvent, BackgroundEvent, ConfigArrived};
use crate::ui_iced::message::{
    BusSource, InboundMessage, MenuMessage, Message, OverlayMessage, WindowMessage,
};
use iced_pro::surface::menu::cascade::MenuKey;

/// A bus event handed to the pure mapper [`bus_event_to_message`].
///
/// The union of every payload type that reaches the iced layer, so the
/// mapping is one testable function instead of five parallel ones. The
/// `Dropped` arm is not a bus payload: it is the lag report the streams
/// synthesise from `RecvError::Lagged(n)`.
#[derive(Debug, Clone)]
pub enum InboundBusEvent {
    /// The one-shot configuration handoff on `Bus<ConfigArrived>`
    /// (`src/app/bus/events/config.rs:10`). Boxed: `AppConfig`
    /// is ~800 bytes and would otherwise dominate the enum's layout.
    Config(Box<ConfigArrived>),
    /// A file change on `Bus<FileChanged>`
    /// (`src/app/bus/events/file.rs`).
    File(FileChanged),
    /// A coalesced burst of file changes drained off
    /// `Bus<FileChanged>` in one wake-up.
    FileBatch(Vec<FileChanged>),
    /// A directory lifecycle change on `Bus<DirectoryChanged>`
    /// (`src/app/bus/events/directory.rs`). One directory per event.
    Directory(DirectoryChanged),
    /// A coalesced burst of directory changes drained off
    /// `Bus<DirectoryChanged>` in one wake-up.
    DirectoryBatch(Vec<DirectoryChanged>),
    /// An agent turn event on `Bus<AgentEvent>`
    /// (`src/app/bus/events/agent.rs:21`).
    Agent(AgentEvent),
    /// A coalesced burst of agent events drained off
    /// `Bus<AgentEvent>` in one wake-up.
    AgentBatch(Vec<AgentEvent>),
    /// A typed background event on the `std::sync::mpsc` channel
    /// (`src/app/bus/events/typed.rs:119`).
    Background(BackgroundEvent),
    /// A coalesced burst of typed background events drained in one
    /// wake-up.
    BackgroundBatch(Vec<BackgroundEvent>),
    /// `count` events were discarded because a subscriber fell behind.
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
/// currently published on the five buses *is* UI-observable, so the mapping
/// is total today and `None` is never produced. The `Option` is kept
/// deliberately: it is the seam where
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
        InboundBusEvent::FileBatch(events) => {
            Some(Message::Inbound(InboundMessage::FileChangedBatch(events)))
        }
        InboundBusEvent::Directory(event) => {
            Some(Message::Inbound(InboundMessage::DirectoryChanged(event)))
        }
        InboundBusEvent::DirectoryBatch(events) => Some(Message::Inbound(
            InboundMessage::DirectoryChangedBatch(events),
        )),
        InboundBusEvent::Agent(event) => {
            Some(Message::Inbound(InboundMessage::Agent(Arc::new(event))))
        }
        InboundBusEvent::AgentBatch(events) => Some(Message::Inbound(InboundMessage::AgentBatch(
            events.into_iter().map(Arc::new).collect(),
        ))),
        InboundBusEvent::Background(event) => Some(Message::Inbound(InboundMessage::Background(
            Arc::new(event),
        ))),
        InboundBusEvent::BackgroundBatch(events) => Some(Message::Inbound(
            InboundMessage::BackgroundBatch(events.into_iter().map(Arc::new).collect()),
        )),
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

/// Handle to the futures-compatible background-event channel.
///
/// Background workers send via `tokio::sync::broadcast::Sender` (never blocks).
/// A dedicated tokio bridge task forwards events to a `futures::channel::mpsc` channel.
/// The subscription polls the `UnboundedReceiver` on the iced executor (futures executor).
/// Uses `Arc<FuturesMutex<...>>` so that clones share the same receiver.
pub struct BackgroundReceiver {
    inner: Arc<FuturesMutex<UnboundedReceiver<BackgroundEvent>>>,
}

impl BackgroundReceiver {
    /// Wraps a futures-compatible receiver.
    pub fn from_futures(receiver: UnboundedReceiver<BackgroundEvent>) -> Self {
        Self {
            inner: Arc::new(FuturesMutex::new(receiver)),
        }
    }
}

/// We need Clone for Subscription::run_with - clones share the same receiver.
impl Clone for BackgroundReceiver {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

/// Address-based identity for [`BackgroundReceiver`] — see
/// [`SharedReceiver`]'s `Hash` docs for the same reasoning.
impl Hash for BackgroundReceiver {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Hash based on the receiver's internal state
        // Since we can't get a stable pointer, use TypeId
        std::any::TypeId::of::<BackgroundEvent>().hash(state);
    }
}

/// Every live receiver the bridge reads, built once at startup.
///
/// Clone it into the app state and pass a reference to [`subscription`];
/// cloning is four `Arc` bumps and keeps every subscription identity
/// stable across frames.
#[derive(Clone)]
pub struct BusReceivers {
    /// `Bus<ConfigArrived>` — the configuration handoff.
    pub config: SharedReceiver<ConfigArrived>,
    /// `Bus<FileChanged>` — watcher / scan / tool / agent file mutations.
    pub file: SharedReceiver<FileChanged>,
    /// `Bus<DirectoryChanged>` — watcher / scan directory lifecycle events.
    pub dir: SharedReceiver<DirectoryChanged>,
    /// `Bus<AgentEvent>` — the agent's streaming turn events.
    pub agent: SharedReceiver<AgentEvent>,
    /// The `futures::channel::mpsc<BackgroundEvent>` channel.
    pub background: BackgroundReceiver,
}

impl BusReceivers {
    /// Subscribes to all five producer channels eagerly.
    pub fn new(
        config_bus: &Bus<ConfigArrived>,
        file_bus: &Bus<FileChanged>,
        dir_bus: &Bus<DirectoryChanged>,
        agent_bus: &Bus<AgentEvent>,
        background_rx: UnboundedReceiver<BackgroundEvent>,
    ) -> Self {
        Self {
            config: SharedReceiver::from_bus(config_bus),
            file: SharedReceiver::from_bus(file_bus),
            dir: SharedReceiver::from_bus(dir_bus),
            agent: SharedReceiver::from_bus(agent_bus),
            background: BackgroundReceiver::from_futures(background_rx),
        }
    }
}

/// Builds the single [`iced::Subscription`] that carries every inbound
/// background notification, one sub-subscription per bus merged with
/// [`iced::Subscription::batch`]
/// (`iced_futures-0.14.0/src/subscription.rs:212-221`).
///
/// Ready to be handed straight to `.subscription(...)`:
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
///
/// Alongside the four bus lanes this also merges the winit window lane
/// (`window_events`) and the runtime keyboard lane (`keyboard_events`),
/// the two sources that are iced event streams rather than producer
/// channels.
pub fn subscription(receivers: &BusReceivers) -> iced::Subscription<Message> {
    iced::Subscription::batch([
        iced::Subscription::run_with(receivers.config.clone(), config_stream),
        iced::Subscription::run_with(receivers.file.clone(), file_stream),
        iced::Subscription::run_with(receivers.dir.clone(), dir_stream),
        iced::Subscription::run_with(receivers.agent.clone(), agent_stream),
        iced::Subscription::run_with(receivers.background.clone(), background_stream),
        // The winit window lane — the subscriptions the window reducers
        // were written against. The launch chain sets
        // `exit_on_close_request: false` ([APP] `ui_iced/app.rs`
        // `window_settings`), so *nothing* exists that handles the title-bar
        // close button unless these subscriptions deliver
        // [`WindowMessage::CloseRequested`] to
        // [`crate::ui_iced::update::reduce_window`]; they also keep
        // `AppState::viewport` in step with real window resizes. Both map
        // through `iced::Subscription::map` (`iced_futures-0.14.0/src/
        // subscription.rs:224-231`).
        window_events(),
        // The keyboard lane — `Esc` dismissal (UI-056) and Shift tracking
        // (UI-022), the two facts no widget callback can hand us.
        keyboard_events(),
    ])
}

/// The winit window-event lane: close requests and
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

/// The runtime keyboard lane ([UI-056], [UI-022]).
///
/// Two `listen_with` lanes, not one, because each iced subscription is
/// identified by its **`fn` pointer** (`listen_with` hashes
/// `(EventsWith, f)` — `iced_futures-0.14.0/src/event.rs:38-47`) and each
/// pure mapper answers exactly one question about an event. Keeping them
/// apart is what lets the sidecar test each rule in isolation instead of
/// unpicking a combined mapper's precedence, and it mirrors the
/// [`bus_event_to_message`] pure-half pattern this module already uses.
///
/// `listen_with` and **not** `listen` is mandatory here: `listen` filters to
/// `Status::Ignored` only (`iced_futures-0.14.0/src/event.rs:12-17`), so
/// an `Esc` pressed while a `text_editor` holds focus would never reach the
/// reducer and the modal would be undismissable. `listen_with` delivers
/// every event with its [`iced::event::Status`]
/// (`iced_futures-0.14.0/src/event.rs:26-47`), which is what lets
/// [`escape_to_message`] accept `Captured` too.
fn keyboard_events() -> iced::Subscription<Message> {
    iced::Subscription::batch([
        iced::event::listen_with(escape_to_message),
        iced::event::listen_with(modifiers_to_message),
    ])
}

/// The menu-navigation keyboard lane (INT-3a, `UIF-CMENU-003`).
///
/// Mounted **only while the hamburger tree is open**, by the
/// `.subscription(...)` builder in `app.rs`. That conditionality is the
/// whole safety story: an always-on lane mapping `Enter`/`Space`/arrows
/// would emit a message on every such keypress in the app, waking the
/// reducer to do nothing. Gating on the open state means the lane exists
/// exactly when something can answer it, and iced's subscription diff
/// drops the listener the frame the menu closes.
pub fn menu_keyboard_events() -> iced::Subscription<Message> {
    iced::event::listen_with(menu_key_to_message)
}

/// Pure arrow/Enter/Space → [`MenuKey`] mapping for the menu keyboard lane.
///
/// * **`Status::Ignored` only.** A key a widget consumed stays consumed.
///   This is what keeps the lane from stealing arrow keys from a focused
///   text editor or `Enter` from a focused button: those arrive
///   `Captured` and are not ours. When the menu is open the cascade has
///   moved focus onto a menu row, and a
///   [`MenuRow`](iced_pro::surface::menu::view::MenuRow) handles no key
///   events of its own, so the navigation keys arrive `Ignored` — which
///   is the case this lane exists to catch.
/// * **`Escape` is deliberately absent.** Dismissal belongs to
///   `escape_to_message` → `close_topmost`, so the `UIF-WIN-003` band
///   walk remains the app's only Escape path. Mapping `Escape` to
///   [`MenuKey::Escape`] here would put a second, menu-local dismissal
///   rule alongside the registry's and let them disagree about how many
///   `Esc`s a nested menu takes.
/// * **`KeyPressed` only.** A release is not a navigation step.
///
/// The window id is carried but unused: the app is single-window.
///
/// [RUST-020]: pure.
#[must_use]
pub fn menu_key_to_message(
    event: iced::Event,
    status: iced::event::Status,
    _window: iced::window::Id,
) -> Option<Message> {
    if status != iced::event::Status::Ignored {
        return None;
    }
    let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) = event else {
        return None;
    };
    let menu_key = match key {
        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowUp) => MenuKey::Up,
        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowDown) => MenuKey::Down,
        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowLeft) => MenuKey::Left,
        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowRight) => MenuKey::Right,
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter) => MenuKey::Enter,
        iced::keyboard::Key::Named(iced::keyboard::key::Named::Space) => MenuKey::Space,
        _ => return None,
    };
    Some(Message::Menu(MenuMessage::Key(menu_key)))
}

/// Pure `Esc` → dismiss mapping for the keyboard lane ([UI-056]).
///
/// A plain `fn` pointer because `iced::event::listen_with` takes one
/// (`fn(Event, event::Status, window::Id) -> Option<Message>`,
/// `iced_futures-0.14.0/src/event.rs:26-31`).
///
/// * **Both statuses accepted.** `Status::Ignored` is the ordinary case
///   (nothing under the pointer wanted the key); `Status::Captured` is the
///   case that matters — a focused `text_editor` swallows the key, and
///   `Esc` must still close the modal over it.
/// * **`KeyPressed` only.** A release is not a dismissal; mapping it would
///   close a freshly reopened layer the instant the user lets go of `Esc`.
/// * **Everything else `None`**, which tells the runtime to discard the
///   event rather than deliver a message.
///
/// The window id is carried by the callback but not used: the app is
/// single-window and the dismissal rule is not window-relative.
///
/// [RUST-020]: pure.
#[must_use]
fn escape_to_message(
    event: iced::Event,
    status: iced::event::Status,
    _window: iced::window::Id,
) -> Option<Message> {
    match status {
        iced::event::Status::Ignored | iced::event::Status::Captured => match event {
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                ..
            }) => Some(Message::Overlay(OverlayMessage::EscapePressed)),
            _ => None,
        },
    }
}

/// Pure Shift-modifier tracking for the keyboard lane ([UI-022]).
///
/// The reason this lane exists at all: iced's `MouseArea::on_press` takes a
/// fixed `Message` and cannot carry the modifier state
/// (`iced_widget-0.14.2/src/mouse_area.rs:68-71`), and
/// `mouse::Event::ButtonPressed` carries none
/// (`iced_core-0.14.0/src/mouse/event.rs:26`). The only place the Shift
/// bit is observable is the keyboard event's own `modifiers` field
/// (`iced_core-0.14.0/src/keyboard/event.rs:12-53`), so it is mirrored
/// onto [`OverlayState::modifiers`](crate::ui_iced::app::OverlayState::modifiers)
/// here.
///
/// **Both** `KeyPressed` and `KeyReleased` are mapped, and both report the
/// event's *current* Shift bit rather than an edge. That makes the tracked
/// value self-correcting: a missed release (focus stolen mid-press) is
/// repaired by the next key event instead of leaving the tree stuck in
/// multi-select.
///
/// **Ordering note (why this is correct for a Shift+click):** the Shift
/// keydown is delivered and reduced before the row-click message produced by
/// the subsequent mouse press, so `overlay.modifiers` already reads
/// `SHIFT` by the time `ViewModels::from_app` projects it into
/// `LeftPanelViewModel::multi_select`.
///
/// The status and window id are irrelevant to the modifier state — a key
/// the widgets captured still changed the keyboard — so both are ignored.
///
/// [RUST-020]: pure.
#[must_use]
fn modifiers_to_message(
    event: iced::Event,
    _status: iced::event::Status,
    _window: iced::window::Id,
) -> Option<Message> {
    let shift = match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { modifiers, .. })
        | iced::Event::Keyboard(iced::keyboard::Event::KeyReleased { modifiers, .. }) => {
            modifiers.shift()
        }
        _ => return None,
    };
    Some(Message::Overlay(OverlayMessage::ModifiersChanged { shift }))
}

/// Reports a lag on `source`: a `tracing::warn!` carrying the dropped
/// count, so the loss is visible rather than silent.
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

/// The `Bus<FileChanged>` lane.
///
/// Every event already queued when the stream wakes is coalesced into one
/// [`InboundMessage::FileChangedBatch`]: a watcher storm then
/// costs the reducer a single pass — one tree rebuild, one view-model
/// rebuild — instead of one per event. A lone event still rides the plain
/// [`InboundMessage::FileChanged`] arm, so the common case is unchanged.
fn file_stream(
    handle: &SharedReceiver<FileChanged>,
) -> iced::futures::stream::BoxStream<'static, Message> {
    coalesced_bus_lane(Arc::clone(&handle.inner), BusSource::File, map_file_batch)
}

/// The `Bus<DirectoryChanged>` lane.
///
/// Same coalescing behaviour as the file lane: a directory-event storm is
/// coalesced into one [`InboundMessage::DirectoryChangedBatch`] per
/// wake-up.
fn dir_stream(
    handle: &SharedReceiver<DirectoryChanged>,
) -> iced::futures::stream::BoxStream<'static, Message> {
    coalesced_bus_lane(
        Arc::clone(&handle.inner),
        BusSource::Directory,
        map_directory_batch,
    )
}

/// The `Bus<AgentEvent>` lane — the highest-volume channel of the buses.
///
/// Every event already queued when the stream wakes is coalesced into one
/// [`InboundMessage::AgentBatch`]: a burst of thinking deltas,
/// tool calls, or status updates then costs the reducer a single pass
/// instead of one per event. A lone event still rides the plain
/// [`InboundMessage::Agent`] arm, so the common case is unchanged.
fn agent_stream(
    handle: &SharedReceiver<AgentEvent>,
) -> iced::futures::stream::BoxStream<'static, Message> {
    coalesced_bus_lane(Arc::clone(&handle.inner), BusSource::Agent, map_agent_batch)
}

/// Pure batch → message mapping for the file lane, fed to
/// [`coalesced_bus_lane`]: a one-element batch rides the plain
/// [`InboundMessage::FileChanged`] arm so the common case keeps its shape.
#[must_use]
fn map_file_batch(mut batch: Vec<FileChanged>) -> Message {
    let event = if batch.len() == 1 {
        InboundBusEvent::File(batch.remove(0))
    } else {
        InboundBusEvent::FileBatch(batch)
    };
    bus_event_to_message(event).expect("file mapping is total")
}

/// Pure batch → message mapping for the directory lane, fed to
/// [`coalesced_bus_lane`].
#[must_use]
fn map_directory_batch(mut batch: Vec<DirectoryChanged>) -> Message {
    let event = if batch.len() == 1 {
        InboundBusEvent::Directory(batch.remove(0))
    } else {
        InboundBusEvent::DirectoryBatch(batch)
    };
    bus_event_to_message(event).expect("directory mapping is total")
}

/// Pure batch → message mapping for the agent lane, fed to
/// [`coalesced_bus_lane`].
#[must_use]
fn map_agent_batch(mut batch: Vec<AgentEvent>) -> Message {
    let event = if batch.len() == 1 {
        InboundBusEvent::Agent(batch.remove(0))
    } else {
        InboundBusEvent::AgentBatch(batch)
    };
    bus_event_to_message(event).expect("agent mapping is total")
}

/// The one streaming loop shared by every broadcast-bus lane: reads one
/// event, drains everything already queued behind it into a
/// batch, and maps the batch through `map` (which collapses a
/// single-element batch back to the plain per-event message).
///
/// A `Lagged(n)` from the broadcast receiver is surfaced as
/// [`InboundBusEvent::Dropped`] for the lane's `source` ([FMD-072]) and the
/// stream keeps running — a lag is not a disconnect.
///
/// The concrete lane fns above pass **`fn` pointers** for `map` because
/// `Subscription::run_with` needs a non-capturing builder; this helper can
/// still by generic because the pointer carries no per-lane state.
fn coalesced_bus_lane<T>(
    inner: Arc<AsyncMutex<broadcast::Receiver<T>>>,
    source: BusSource,
    map: fn(Vec<T>) -> Message,
) -> iced::futures::stream::BoxStream<'static, Message>
where
    T: Send + Clone + 'static,
{
    iced::futures::stream::unfold(inner, move |inner| async move {
        let received = {
            let mut receiver = inner.lock().await;
            receiver.recv().await.map(|first| {
                let mut batch = vec![first];
                while let Ok(extra) = receiver.try_recv() {
                    batch.push(extra);
                }
                batch
            })
        };
        match received {
            Ok(batch) => Some((map(batch), inner)),
            Err(broadcast::error::RecvError::Closed) => None,
            Err(broadcast::error::RecvError::Lagged(count)) => {
                warn_dropped(source, count);
                Some((
                    bus_event_to_message(InboundBusEvent::Dropped { source, count })
                        .expect("dropped mapping is total"),
                    inner,
                ))
            }
        }
    })
    .boxed()
}

/// The `BackgroundEvent` lane (indexer / watcher progress, worker logs,
/// file-load results, MCP OAuth completions).
///
/// Like the file lane, every event already queued when the stream wakes is
/// coalesced into one [`InboundMessage::BackgroundBatch`] so indexer
/// progress storms cost one reduction instead of one per event.
fn background_stream(
    handle: &BackgroundReceiver,
) -> iced::futures::stream::BoxStream<'static, Message> {
    let inner = Arc::clone(&handle.inner);
    iced::futures::stream::unfold(inner, |inner| async move {
        // next() returns Option; on channel closed (None), return None to end stream
        let first = {
            let mut receiver = inner.lock().await;
            match receiver.next().await {
                Some(event) => event,
                None => return None, // Channel closed, end the stream
            }
        };
        let mut batch = vec![first];
        {
            let mut receiver = inner.lock().await;
            // Try to drain additional events that are already queued
            loop {
                match receiver.try_recv() {
                    Ok(extra) => batch.push(extra),
                    Err(e) if e.is_empty() => break,
                    Err(_) => break, // Disconnected or other error
                }
            }
        }
        let event = if batch.len() == 1 {
            InboundBusEvent::Background(batch.remove(0))
        } else {
            InboundBusEvent::BackgroundBatch(batch)
        };
        Some((bus_event_to_message(event), inner))
    })
    .filter_map(iced::futures::future::ready)
    .boxed()
}

#[cfg(test)]
#[path = "subscription_tests.rs"]
mod tests;
