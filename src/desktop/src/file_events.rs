//! File event bus for decoupling producers (initial scan, file system
//! notification watchers) from consumers (tag manager/indexer, directory
//! tree).
//!
//! The bus implements a simple multi-producer / multi-consumer pattern
//! using `std::sync::mpsc` channels under the hood. Every consumer that
//! calls `subscribe()` gets its own channel; every call to `publish()`
//! sends the event to every registered consumer.
//!
//! Producers (the initial scan and the notify watcher) clone the bus
//! cheaply (it's wrapped in an `Arc`).
//!
//! Consumers (the tag manager and the directory tree) call `subscribe()`
//! once at startup to get a `BusReader` they can iterate over.
//!
//! A dropped consumer is detected lazily: when `publish()` tries to send
//! on a disconnected channel, the receiver is removed from the bus and
//! the slot is reclaimed. This keeps memory bounded even if a consumer
//! thread panics or is shut down.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{channel, Receiver, Sender};

/// What happened to a file in one of the configured content libraries.
///
/// `Discovered` — the file was seen for the first time during the
///                 initial scan.
///
/// `Updated`    — the file was created, modified, or renamed on disk
///                 (reported by the notify watcher). Tags must be
///                 re-extracted.
///
/// `Removed`    — the file no longer exists (was deleted, or the
///                 directory it lived in was removed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileEventKind {
    Discovered,
    Updated,
    Removed,
}

/// A single file-system event published to the bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEvent {
    pub kind: FileEventKind,
    pub path: PathBuf,
}

impl FileEvent {
    pub fn discovered(path: PathBuf) -> Self {
        Self { kind: FileEventKind::Discovered, path }
    }

    pub fn updated(path: PathBuf) -> Self {
        Self { kind: FileEventKind::Updated, path }
    }

    pub fn removed(path: PathBuf) -> Self {
        Self { kind: FileEventKind::Removed, path }
    }
}

/// A thread-safe, multi-producer / multi-consumer event bus.
///
/// Cloning a `Bus` is cheap (it's an `Arc` internally) and produces a
/// new handle that shares the same subscriber list. This makes `Bus`
/// suitable for handing to background threads.
#[derive(Clone)]
pub struct Bus<T: Send + 'static + Clone> {
    inner: Arc<BusInner<T>>,
}

struct BusInner<T> {
    subscribers: Mutex<Vec<Sender<T>>>,
}

impl<T: Send + 'static + Clone> Bus<T> {
    /// Create a new, empty bus with no subscribers.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(BusInner {
                subscribers: Mutex::new(Vec::new()),
            }),
        }
    }

    /// Register a new consumer. Each consumer gets its own private
    /// channel; events sent to the bus are delivered to every
    /// registered consumer.
    pub fn subscribe(&self) -> BusReader<T> {
        let (tx, rx) = channel();
        self.inner
            .subscribers
            .lock()
            .expect("Bus subscriber list poisoned")
            .push(tx);
        BusReader { rx }
    }

    /// Publish an event to every registered consumer. Consumers that
    /// have been dropped are silently removed from the bus.
    ///
    /// Returns the number of consumers the event was successfully
    /// delivered to.
    pub fn publish(&self, event: T) -> usize {
        let mut subscribers = self
            .inner
            .subscribers
            .lock()
            .expect("Bus subscriber list poisoned");
        subscribers.retain(|tx| tx.send(event.clone()).is_ok());
        subscribers.len()
    }

    /// Number of currently registered consumers. Mainly useful for
    /// tests and diagnostics.
    pub fn subscriber_count(&self) -> usize {
        self.inner
            .subscribers
            .lock()
            .expect("Bus subscriber list poisoned")
            .len()
    }
}

impl<T: Send + 'static + Clone> Default for Bus<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// The receive end of a bus subscription. Iterate with
/// `try_recv` / `recv` / `recv_timeout` exactly like a regular
/// `std::sync::mpsc::Receiver`.
pub struct BusReader<T> {
    rx: Receiver<T>,
}

impl<T> BusReader<T> {
    /// Try to receive an event without blocking. Returns immediately
    /// with `Err(TryRecvError::Empty)` if no event is available.
    pub fn try_recv(&self) -> Result<T, std::sync::mpsc::TryRecvError> {
        self.rx.try_recv()
    }

    /// Block until an event is available, or the channel is closed.
    pub fn recv(&self) -> Result<T, std::sync::mpsc::RecvError> {
        self.rx.recv()
    }

    /// Block for at most `timeout` waiting for an event.
    pub fn recv_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> Result<T, std::sync::mpsc::RecvTimeoutError> {
        self.rx.recv_timeout(timeout)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_publish_delivers_to_every_subscriber() {
        let bus: Bus<i32> = Bus::new();
        let r1 = bus.subscribe();
        let r2 = bus.subscribe();
        let r3 = bus.subscribe();

        let delivered = bus.publish(42);
        assert_eq!(delivered, 3);
        assert_eq!(r1.recv_timeout(Duration::from_millis(100)).unwrap(), 42);
        assert_eq!(r2.recv_timeout(Duration::from_millis(100)).unwrap(), 42);
        assert_eq!(r3.recv_timeout(Duration::from_millis(100)).unwrap(), 42);
    }

    #[test]
    fn test_subscriber_count_tracks_subscriptions() {
        let bus: Bus<i32> = Bus::new();
        assert_eq!(bus.subscriber_count(), 0);
        let _a = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 1);
        let _b = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 2);
    }

    #[test]
    fn test_dropped_consumer_is_cleaned_up() {
        let bus: Bus<i32> = Bus::new();
        let r1 = bus.subscribe();
        let r2 = bus.subscribe();
        assert_eq!(bus.subscriber_count(), 2);

        drop(r1);
        // Lazy cleanup: subscriber still in list until publish is called.
        assert_eq!(bus.subscriber_count(), 2);

        let delivered = bus.publish(7);
        // r1 was dropped, so the publish should only succeed for r2.
        assert_eq!(delivered, 1);
        assert_eq!(bus.subscriber_count(), 1);
        assert_eq!(r2.recv_timeout(Duration::from_millis(100)).unwrap(), 7);
    }

    #[test]
    fn test_bus_clone_shares_subscriber_list() {
        let bus: Bus<&'static str> = Bus::new();
        let bus_clone = bus.clone();
        let reader = bus_clone.subscribe();
        assert_eq!(bus.subscriber_count(), 1);
        bus.publish("shared");
        assert_eq!(
            reader.recv_timeout(Duration::from_millis(100)).unwrap(),
            "shared"
        );
    }

    #[test]
    fn test_publish_with_no_subscribers_does_not_panic() {
        let bus: Bus<i32> = Bus::new();
        let delivered = bus.publish(123);
        assert_eq!(delivered, 0);
    }

    #[test]
    fn test_multiple_events_delivered_in_order() {
        let bus: Bus<i32> = Bus::new();
        let reader = bus.subscribe();
        for i in 0..10 {
            bus.publish(i);
        }
        for i in 0..10 {
            assert_eq!(reader.recv_timeout(Duration::from_millis(100)).unwrap(), i);
        }
    }

    #[test]
    fn test_concurrent_publishers_and_subscribers() {
        let bus: Bus<usize> = Bus::new();
        let received = Arc::new(Mutex::new(HashSet::new()));
        let counter = Arc::new(AtomicUsize::new(0));

        let mut readers = Vec::new();
        for _ in 0..4 {
            let r = bus.subscribe();
            let received = Arc::clone(&received);
            let counter = Arc::clone(&counter);
            readers.push(thread::spawn(move || {
                while let Ok(v) = r.recv_timeout(Duration::from_millis(500)) {
                    received.lock().unwrap().insert(v);
                    counter.fetch_add(1, Ordering::SeqCst);
                }
            }));
        }

        let mut publishers = Vec::new();
        for t in 0..4 {
            let bus = bus.clone();
            publishers.push(thread::spawn(move || {
                for i in 0..25 {
                    bus.publish(t * 100 + i);
                }
            }));
        }
        for p in publishers {
            p.join().unwrap();
        }

        // Give the consumers a moment to drain.
        thread::sleep(Duration::from_millis(100));
        drop(bus); // close all receivers

        for h in readers {
            h.join().unwrap();
        }

        // Every consumer should have seen every event (4 publishers * 25 events).
        assert_eq!(counter.load(Ordering::SeqCst), 4 * 4 * 25);
        // Every value was received by at least one consumer.
        assert_eq!(received.lock().unwrap().len(), 100);
    }

    // -- FileEvent-specific tests --

    #[test]
    fn test_file_event_constructors() {
        let p = PathBuf::from("docs/notes.md");
        let d = FileEvent::discovered(p.clone());
        assert_eq!(d.kind, FileEventKind::Discovered);
        assert_eq!(d.path, p);

        let u = FileEvent::updated(p.clone());
        assert_eq!(u.kind, FileEventKind::Updated);

        let r = FileEvent::removed(p.clone());
        assert_eq!(r.kind, FileEventKind::Removed);
    }

    #[test]
    fn test_file_event_bus_delivery() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let path = PathBuf::from("a/b/c.md");
        bus.publish(FileEvent::discovered(path.clone()));
        bus.publish(FileEvent::updated(path.clone()));
        bus.publish(FileEvent::removed(path.clone()));

        let e1 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(e1.kind, FileEventKind::Discovered);
        assert_eq!(e1.path, path);
        let e2 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(e2.kind, FileEventKind::Updated);
        let e3 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(e3.kind, FileEventKind::Removed);
    }
}
