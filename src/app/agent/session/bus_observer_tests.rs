//! Tests for `bus_observer.rs`.

use super::*;
use crate::bus::events::file::{FileChanged, FileChangedKind};
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
        .expect("must receive [FileChanged][Changed] event");
    assert!(
        matches!(
            &ev,
            FileEvent::FileChanged(FileChanged {
                kind: FileChangedKind::Changed,
                files,
            }) if files.len() == 1 && files[0].path == path
        ),
        "on_file_changed publishes [FileChanged][Changed]"
    );
}

#[test]
fn on_file_changed_is_trait_object_safe() {
    let bus: Bus<FileEvent> = Bus::new();
    let reader = bus.subscribe();
    let obs: Box<dyn OnFileChanged> = Box::new(AppFileObserver::new(bus));
    let path = PathBuf::from("/tmp/trait.md");

    obs.on_file_changed(&path);

    let ev = reader.recv_timeout(Duration::from_millis(100)).unwrap();
    assert!(
        matches!(
            &ev,
            FileEvent::FileChanged(FileChanged {
                kind: FileChangedKind::Changed,
                files,
            }) if files.len() == 1 && files[0].path == path
        ),
        "trait object publishes [FileChanged][Changed]"
    );
}
