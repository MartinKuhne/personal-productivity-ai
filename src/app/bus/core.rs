//! Transport primitive — multi-producer / multi-consumer event bus backed by `tokio::sync::broadcast`.
//!
//! Cloning a [`Bus`] is cheap (it shares the underlying broadcast sender via
//! `Arc`) and produces a new handle to the same channel. Each consumer that
//! calls [`Bus::subscribe`] gets its own [`BusReader`]; events published via
//! [`Bus::publish`] are delivered to every registered consumer.
//!
//! A dropped consumer is detected lazily: when [`Bus::publish`] calls `send`
//! on the broadcast channel, the subscriber count automatically reflects how
//! many consumers are still alive.
//!
//! Unit tests live in the sibling `core_tests.rs` sidecar.

use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use tokio::sync::broadcast;

/// Default capacity for the underlying broadcast channel.
const BUS_CAPACITY: usize = 8192;

/// Error returned by [`BusReader::try_recv_exposing_lag`] — a transparent
/// wrapper around `tokio::sync::broadcast::error::TryRecvError` that exposes
/// the `Lagged(n)` signal (unlike [`std::sync::mpsc::TryRecvError`] which has
/// no lag variant). Used by UI drain loops that must react to dropped messages
/// (research.md §1, quickstart scenario 5).
#[derive(Debug)]
pub enum BroadcastRecvError {
    /// No event available (channel not closed, just empty).
    Empty,
    /// The subscriber fell behind — `n` events were dropped. The next
    /// `try_recv` will return the oldest still-buffered event (or `Empty`
    /// if the buffer was fully skipped).
    Lagged(u64),
    /// No more senders exist; the channel is closed.
    Closed,
}

impl From<broadcast::error::TryRecvError> for BroadcastRecvError {
    fn from(e: broadcast::error::TryRecvError) -> Self {
        match e {
            broadcast::error::TryRecvError::Closed => Self::Closed,
            broadcast::error::TryRecvError::Empty => Self::Empty,
            broadcast::error::TryRecvError::Lagged(n) => Self::Lagged(n),
        }
    }
}

/// A thread-safe, multi-producer / multi-consumer event bus backed by
/// `tokio::sync::broadcast` with a [`Condvar`] for zero-CPU synchronous blocking.
///
/// Cloning a `Bus` is cheap (it shares the sender and condvar via `Arc`) and
/// produces a new handle that shares the same broadcast channel.
#[derive(Clone)]
pub struct Bus<T: Clone + Send + 'static> {
    sender: broadcast::Sender<T>,
    notify: Arc<Condvar>,
}

impl<T: Clone + Send + 'static> Bus<T> {
    /// Create a new bus with a fixed-capacity broadcast channel.
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(BUS_CAPACITY);
        Self {
            sender,
            notify: Arc::new(Condvar::new()),
        }
    }

    /// Register a new consumer. Each consumer gets its own receiver;
    /// events sent to the bus are delivered to every registered consumer.
    pub fn subscribe(&self) -> BusReader<T> {
        BusReader {
            inner: Mutex::new(self.sender.subscribe()),
            notify: Arc::clone(&self.notify),
        }
    }

    /// Register a new asynchronous consumer backed directly by a Tokio broadcast receiver.
    pub fn subscribe_async(&self) -> broadcast::Receiver<T> {
        self.sender.subscribe()
    }

    /// Publish an event to every registered consumer.
    ///
    /// Returns the number of consumers the event was successfully
    /// delivered to. Consumers that are lagging behind may miss
    /// events (the broadcast channel drops the oldest events when
    /// full).
    pub fn publish(&self, event: T) -> usize {
        let count = self.sender.send(event).unwrap_or(0);
        self.notify.notify_all();
        count
    }

    /// Number of currently registered consumers.
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl<T: Clone + Send + 'static> Drop for Bus<T> {
    fn drop(&mut self) {
        self.notify.notify_all();
    }
}

impl<T: Clone + Send + 'static> Default for Bus<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// The receive end of a bus subscription. Backed by a
/// `tokio::sync::broadcast::Receiver` wrapped in a `Mutex` for
/// interior mutability, with a shared `Condvar` to avoid polling.
pub struct BusReader<T: Clone> {
    inner: Mutex<broadcast::Receiver<T>>,
    notify: Arc<Condvar>,
}

impl<T: Clone> BusReader<T> {
    /// Create a BusReader from an existing broadcast receiver.
    pub fn new(rx: broadcast::Receiver<T>) -> Self {
        Self {
            inner: Mutex::new(rx),
            notify: Arc::new(Condvar::new()),
        }
    }

    /// Unwrap into the underlying async Tokio receiver.
    pub fn into_inner(self) -> broadcast::Receiver<T> {
        self.inner.into_inner().unwrap()
    }

    /// Try to receive an event without blocking.
    pub fn try_recv(&self) -> Result<T, std::sync::mpsc::TryRecvError> {
        self.inner.lock().unwrap().try_recv().map_err(|e| match e {
            broadcast::error::TryRecvError::Closed => std::sync::mpsc::TryRecvError::Disconnected,
            broadcast::error::TryRecvError::Empty => std::sync::mpsc::TryRecvError::Empty,
            broadcast::error::TryRecvError::Lagged(_) => std::sync::mpsc::TryRecvError::Empty,
        })
    }

    /// Try to receive an event without blocking, exposing the `Lagged(n)`
    /// signal from the underlying tokio broadcast channel so callers can
    /// react to dropped messages (research.md §1, quickstart scenario 5).
    ///
    /// Unlike [`Self::try_recv`] (which silently maps `Lagged` to `Empty`),
    /// this returns `Ok(Lagged(n))` when the subscriber fell behind, so the
    /// UI drain loop can emit a visible truncation marker and re-sync.
    pub fn try_recv_exposing_lag(&self) -> Result<T, BroadcastRecvError> {
        self.inner.lock().unwrap().try_recv().map_err(Into::into)
    }

    /// Block until an event is available, or the channel is closed.
    /// Blocks the OS thread on a condvar instead of spin-waiting.
    pub fn recv(&self) -> Result<T, std::sync::mpsc::RecvError> {
        let mut guard = self.inner.lock().unwrap();
        loop {
            match guard.try_recv() {
                Ok(val) => return Ok(val),
                Err(broadcast::error::TryRecvError::Closed) => {
                    return Err(std::sync::mpsc::RecvError);
                }
                Err(_) => {
                    guard = self.notify.wait(guard).unwrap();
                }
            }
        }
    }

    /// Block for at most `timeout` waiting for an event.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, std::sync::mpsc::RecvTimeoutError> {
        let start = std::time::Instant::now();
        let mut guard = self.inner.lock().unwrap();
        loop {
            match guard.try_recv() {
                Ok(val) => return Ok(val),
                Err(broadcast::error::TryRecvError::Closed) => {
                    return Err(std::sync::mpsc::RecvTimeoutError::Disconnected);
                }
                Err(_) => {
                    let elapsed = start.elapsed();
                    if elapsed >= timeout {
                        return Err(std::sync::mpsc::RecvTimeoutError::Timeout);
                    }
                    let remaining = timeout - elapsed;
                    let (new_guard, wait_res) = self.notify.wait_timeout(guard, remaining).unwrap();
                    guard = new_guard;
                    if wait_res.timed_out() && start.elapsed() >= timeout {
                        return match guard.try_recv() {
                            Ok(val) => Ok(val),
                            Err(broadcast::error::TryRecvError::Closed) => {
                                Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
                            }
                            Err(_) => Err(std::sync::mpsc::RecvTimeoutError::Timeout),
                        };
                    }
                }
            }
        }
    }
}

impl<T: Clone> BusReader<T> {
    /// Create a detached BusReader that is not connected to any bus.
    /// Useful for initializing consumers that will later be rewired
    /// to a real bus.
    pub fn detached() -> Self {
        let (_tx, rx) = broadcast::channel(16);
        Self::new(rx)
    }
}

#[cfg(test)]
#[path = "core_tests.rs"]
mod tests;
