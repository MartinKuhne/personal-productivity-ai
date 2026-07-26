//! File event bus for decoupling producers (initial scan, file system
//! notification watchers) from consumers (tag manager/indexer, directory
//! tree).
//!
//! The bus implements a multi-producer / multi-consumer broadcast pattern
//! using `tokio::sync::broadcast` under the hood. Every consumer that
//! calls `subscribe()` gets its own receiver from the broadcast sender.
//!
//! Producers (the initial scan and the notify watcher) clone the bus
//! cheaply (it's wrapped in an `Arc` internally).
//!
//! Consumers (the tag manager and the directory tree) call `subscribe()`
//! once at startup to get a `BusReader` they can iterate over.
//!
//! A dropped consumer is detected lazily: when `publish()` calls `send()`
//! on the broadcast channel, the subscriber count automatically reflects
//! how many consumers are still alive.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::broadcast;

/// Default capacity for the underlying broadcast channel.
const BUS_CAPACITY: usize = 8192;

/// What happened to a file-system entry in one of the configured content
/// libraries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileEventKind {
    Discovered,
    Updated,
    Removed,
    DirDiscovered,
    DirRemoved,
}

/// A single file-system event published to the bus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEvent {
    pub kind: FileEventKind,
    pub paths: Vec<PathBuf>,
}

impl FileEvent {
    pub fn discovered(paths: Vec<PathBuf>) -> Self {
        Self {
            kind: FileEventKind::Discovered,
            paths,
        }
    }

    pub fn discovered_one(path: PathBuf) -> Self {
        Self::discovered(vec![path])
    }

    pub fn updated(paths: Vec<PathBuf>) -> Self {
        Self {
            kind: FileEventKind::Updated,
            paths,
        }
    }

    pub fn updated_one(path: PathBuf) -> Self {
        Self::updated(vec![path])
    }

    pub fn removed(paths: Vec<PathBuf>) -> Self {
        Self {
            kind: FileEventKind::Removed,
            paths,
        }
    }

    pub fn removed_one(path: PathBuf) -> Self {
        Self::removed(vec![path])
    }

    pub fn dir_discovered(paths: Vec<PathBuf>) -> Self {
        Self {
            kind: FileEventKind::DirDiscovered,
            paths,
        }
    }

    pub fn dir_discovered_one(path: PathBuf) -> Self {
        Self::dir_discovered(vec![path])
    }

    pub fn dir_removed(paths: Vec<PathBuf>) -> Self {
        Self {
            kind: FileEventKind::DirRemoved,
            paths,
        }
    }

    pub fn dir_removed_one(path: PathBuf) -> Self {
        Self::dir_removed(vec![path])
    }
}

/// A thread-safe, multi-producer / multi-consumer event bus backed by
/// `tokio::sync::broadcast`.
///
/// Cloning a `Bus` is cheap (it's an `Arc` of the sender internally) and
/// produces a new handle that shares the same broadcast channel.
#[derive(Clone)]
pub struct Bus<T: Clone + Send + 'static> {
    sender: broadcast::Sender<T>,
}

impl<T: Clone + Send + 'static> Bus<T> {
    /// Create a new bus with a fixed-capacity broadcast channel.
    pub fn new() -> Self {
        let (sender, _) = broadcast::channel(BUS_CAPACITY);
        Self { sender }
    }

    /// Register a new consumer. Each consumer gets its own receiver;
    /// events sent to the bus are delivered to every registered consumer.
    pub fn subscribe(&self) -> BusReader<T> {
        BusReader {
            inner: Mutex::new(self.sender.subscribe()),
        }
    }

    /// Publish an event to every registered consumer.
    ///
    /// Returns the number of consumers the event was successfully
    /// delivered to. Consumers that are lagging behind may miss
    /// events (the broadcast channel drops the oldest events when
    /// full).
    pub fn publish(&self, event: T) -> usize {
        self.sender.send(event).unwrap_or(0)
    }

    /// Number of currently registered consumers.
    pub fn subscriber_count(&self) -> usize {
        self.sender.receiver_count()
    }
}

impl<T: Clone + Send + 'static> Default for Bus<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// The receive end of a bus subscription. Backed by a
/// `tokio::sync::broadcast::Receiver` wrapped in a `Mutex` for
/// interior mutability (all methods take `&self`).
pub struct BusReader<T: Clone> {
    inner: Mutex<broadcast::Receiver<T>>,
}

impl<T: Clone> BusReader<T> {
    /// Create a BusReader from an existing broadcast receiver.
    pub fn new(rx: broadcast::Receiver<T>) -> Self {
        Self {
            inner: Mutex::new(rx),
        }
    }

    /// Try to receive an event without blocking.
    pub fn try_recv(&self) -> Result<T, std::sync::mpsc::TryRecvError> {
        self.inner.lock().unwrap().try_recv().map_err(|e| match e {
            broadcast::error::TryRecvError::Closed => std::sync::mpsc::TryRecvError::Disconnected,
            broadcast::error::TryRecvError::Empty => std::sync::mpsc::TryRecvError::Empty,
            broadcast::error::TryRecvError::Lagged(_) => std::sync::mpsc::TryRecvError::Empty,
        })
    }

    /// Block until an event is available, or the channel is closed.
    /// Uses a spin-wait with short sleeps since the underlying broadcast
    /// receiver has no blocking synchronous API.
    pub fn recv(&self) -> Result<T, std::sync::mpsc::RecvError> {
        loop {
            match self.inner.lock().unwrap().try_recv() {
                Ok(val) => return Ok(val),
                Err(broadcast::error::TryRecvError::Closed) => {
                    return Err(std::sync::mpsc::RecvError);
                }
                Err(_) => std::thread::sleep(Duration::from_millis(10)),
            }
        }
    }

    /// Block for at most `timeout` waiting for an event.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, std::sync::mpsc::RecvTimeoutError> {
        let start = std::time::Instant::now();
        loop {
            match self.inner.lock().unwrap().try_recv() {
                Ok(val) => return Ok(val),
                Err(broadcast::error::TryRecvError::Closed) => {
                    return Err(std::sync::mpsc::RecvTimeoutError::Disconnected);
                }
                Err(_) => {
                    if start.elapsed() >= timeout {
                        return Err(std::sync::mpsc::RecvTimeoutError::Timeout);
                    }
                    std::thread::sleep(Duration::from_millis(10));
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
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
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
        // Dropping the reader immediately drops the receiver, so the
        // subscriber count should update right away.
        assert_eq!(bus.subscriber_count(), 1);

        let delivered = bus.publish(7);
        assert_eq!(delivered, 1);
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
        let d = FileEvent::discovered_one(p.clone());
        assert_eq!(d.kind, FileEventKind::Discovered);
        assert_eq!(d.paths, vec![p]);

        let p = PathBuf::from("docs/notes.md");
        let u = FileEvent::updated_one(p.clone());
        assert_eq!(u.kind, FileEventKind::Updated);

        let p = PathBuf::from("docs/notes.md");
        let r = FileEvent::removed_one(p.clone());
        assert_eq!(r.kind, FileEventKind::Removed);
    }

    #[test]
    fn test_dir_event_constructors() {
        let p = PathBuf::from("docs/subdir");
        let d = FileEvent::dir_discovered_one(p.clone());
        assert_eq!(d.kind, FileEventKind::DirDiscovered);
        assert_eq!(d.paths, vec![p]);

        let p = PathBuf::from("docs/subdir");
        let r = FileEvent::dir_removed_one(p.clone());
        assert_eq!(r.kind, FileEventKind::DirRemoved);
        assert_eq!(r.paths, vec![p]);
    }

    #[test]
    fn test_file_event_bus_delivery() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let path = PathBuf::from("a/b/c.md");
        bus.publish(FileEvent::discovered_one(path.clone()));
        bus.publish(FileEvent::updated_one(path.clone()));
        bus.publish(FileEvent::removed_one(path.clone()));

        let e1 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(e1.kind, FileEventKind::Discovered);
        assert_eq!(e1.paths, vec![path]);
        let e2 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(e2.kind, FileEventKind::Updated);
        let e3 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(e3.kind, FileEventKind::Removed);
    }

    // -- FileEventProducer tests --

    #[test]
    fn test_producer_publishes_discovered_for_new_file() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(&bus);
        let path = PathBuf::from("/tmp/new.md");

        producer.publish_discovered(&path);

        let event = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(event.kind, FileEventKind::Discovered);
        assert_eq!(event.paths, vec![path]);
    }

    #[test]
    fn test_producer_publishes_updated_for_existing_file() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(&bus);
        let path = PathBuf::from("/tmp/existing.md");

        producer.publish_updated(&path);

        let event = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(event.kind, FileEventKind::Updated);
        assert_eq!(event.paths, vec![path]);
    }

    #[test]
    fn test_producer_publishes_removed() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(&bus);
        let path = PathBuf::from("/tmp/gone.md");

        producer.publish_removed(&path);

        let event = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(event.kind, FileEventKind::Removed);
        assert_eq!(event.paths, vec![path]);
    }

    #[test]
    fn test_producer_publishes_rename_as_removed_plus_discovered() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(&bus);
        let old = PathBuf::from("/tmp/old.md");
        let new = PathBuf::from("/tmp/new.md");

        producer.publish_rename(&old, &new);

        let e1 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(e1.kind, FileEventKind::Removed);
        assert_eq!(e1.paths, vec![old]);
        let e2 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(e2.kind, FileEventKind::Discovered);
        assert_eq!(e2.paths, vec![new]);
    }

    #[test]
    fn test_producer_publishes_dir_discovered() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(&bus);
        let path = PathBuf::from("/tmp/newdir");

        producer.publish_dir_discovered(&path);

        let event = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(event.kind, FileEventKind::DirDiscovered);
        assert_eq!(event.paths, vec![path]);
    }

    #[test]
    fn test_producer_publishes_dir_removed() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(&bus);
        let path = PathBuf::from("/tmp/olddir");

        producer.publish_dir_removed(&path);

        let event = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(event.kind, FileEventKind::DirRemoved);
        assert_eq!(event.paths, vec![path]);
    }

    #[test]
    fn test_producer_publishes_dir_rename() {
        let bus: Bus<FileEvent> = Bus::new();
        let reader = bus.subscribe();
        let producer = FileEventProducer::new(&bus);
        let old = PathBuf::from("/tmp/olddir");
        let new = PathBuf::from("/tmp/newdir");

        producer.publish_dir_rename(&old, &new);

        let e1 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(e1.kind, FileEventKind::DirRemoved);
        assert_eq!(e1.paths, vec![old]);
        let e2 = reader.recv_timeout(Duration::from_millis(100)).unwrap();
        assert_eq!(e2.kind, FileEventKind::DirDiscovered);
        assert_eq!(e2.paths, vec![new]);
    }
}

// =====================================================================
// FileEventProducer
// =====================================================================

/// A thin handle for publishing [`FileEvent`]s from code that mutates
/// the filesystem (UI handlers, tool implementations, the agent, etc.).
pub struct FileEventProducer<'a> {
    bus: &'a Bus<FileEvent>,
}

impl<'a> FileEventProducer<'a> {
    pub fn new(bus: &'a Bus<FileEvent>) -> Self {
        Self { bus }
    }

    pub fn publish_discovered(&self, path: &std::path::Path) {
        self.bus
            .publish(FileEvent::discovered_one(path.to_path_buf()));
    }

    pub fn publish_updated(&self, path: &std::path::Path) {
        self.bus.publish(FileEvent::updated_one(path.to_path_buf()));
    }

    pub fn publish_removed(&self, path: &std::path::Path) {
        self.bus.publish(FileEvent::removed_one(path.to_path_buf()));
    }

    pub fn publish_rename(&self, old: &std::path::Path, new: &std::path::Path) {
        self.bus.publish(FileEvent::removed_one(old.to_path_buf()));
        self.bus
            .publish(FileEvent::discovered_one(new.to_path_buf()));
    }

    pub fn publish_dir_discovered(&self, path: &std::path::Path) {
        self.bus
            .publish(FileEvent::dir_discovered_one(path.to_path_buf()));
    }

    pub fn publish_dir_removed(&self, path: &std::path::Path) {
        self.bus
            .publish(FileEvent::dir_removed_one(path.to_path_buf()));
    }

    pub fn publish_dir_rename(&self, old: &std::path::Path, new: &std::path::Path) {
        self.bus
            .publish(FileEvent::dir_removed_one(old.to_path_buf()));
        self.bus
            .publish(FileEvent::dir_discovered_one(new.to_path_buf()));
    }
}
