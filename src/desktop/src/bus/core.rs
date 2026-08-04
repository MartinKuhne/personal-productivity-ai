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

use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::broadcast;

/// Default capacity for the underlying broadcast channel.
const BUS_CAPACITY: usize = 8192;

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
}
