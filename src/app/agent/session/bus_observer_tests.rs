//! Tests for `bus_observer.rs`.

use super::*;
use crate::bus::events::file::FileEventKind;
use std::path::PathBuf;
use std::time::Duration;

#[test]
fn new_creates_observer() {
    let bus: Bus<FileEvent> = Bus::new();
    let _obs = AppFileObserver::new(bus);
}

#[test]
fn on_file_changed_publishes_updated_event() {
    let bus: Bus<FileEvent> = Bus::new();
    let reader = bus.subscribe();
    let obs = AppFileObserver::new(bus);
    let path = PathBuf::from("/tmp/note.md");

    obs.on_file_changed(&path);

    let ev = reader
        .recv_timeout(Duration::from_millis(100))
        .expect("must receive Updated event");
    assert_eq!(ev.kind, FileEventKind::Updated);
    assert_eq!(ev.paths, vec![path]);
}

#[test]
fn on_file_changed_is_trait_object_safe() {
    let bus: Bus<FileEvent> = Bus::new();
    let reader = bus.subscribe();
    let obs: Box<dyn OnFileChanged> = Box::new(AppFileObserver::new(bus));
    let path = PathBuf::from("/tmp/trait.md");

    obs.on_file_changed(&path);

    let ev = reader.recv_timeout(Duration::from_millis(100)).unwrap();
    assert_eq!(ev.kind, FileEventKind::Updated);
    assert_eq!(ev.paths, vec![path]);
}
