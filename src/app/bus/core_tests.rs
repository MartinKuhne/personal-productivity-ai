//! Tests for [`Bus`] and [`BusReader`] (`bus::core`).

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
