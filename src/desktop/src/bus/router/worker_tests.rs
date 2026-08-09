use super::*;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn test_spawn_path_worker_processes_items() {
    let (tx, rx) = std::sync::mpsc::channel();
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    spawn_path_worker(rx, move |path| {
        let c = counter_clone.clone();
        async move {
            if path == Path::new("test") {
                c.fetch_add(1, Ordering::SeqCst);
            }
        }
    });

    tx.send(PathBuf::from("test")).unwrap();
    tx.send(PathBuf::from("test")).unwrap();
    tx.send(PathBuf::from("test")).unwrap();

    // Drop the sender to signal termination
    drop(tx);

    // Give it a moment to process
    std::thread::sleep(std::time::Duration::from_millis(100));

    assert_eq!(counter.load(Ordering::SeqCst), 3);
}

#[test]
fn test_channel_worker_marker_struct() {
    let _worker = ChannelWorker::<fn()>::new();
    let _worker2 = ChannelWorker::<fn()>::default();
    // Mostly just verifying it compiles and can be constructed
}
