//! Unit tests for the `notify`-based filesystem watcher (sidecar of
//! `file_watcher.rs`).

use super::*;
use crate::bus::events::directory::DirectoryChanged;
use crate::bus::events::directory::DirectoryChangedKind;
use crate::bus::events::file::{FileChanged, FileChangedKind};
use crate::bus::events::typed::BackgroundEvent;
use crate::bus::events::typed::BackgroundEventSender;
use crate::config::{AppConfig, ContentLibrary};
use tempfile::tempdir;

/// Predicate helper: does `ev` carry `[FileChanged]` with `kind` and `path`?
fn is_file_event_of_kind(ev: &FileChanged, kind: FileChangedKind, path: &std::path::Path) -> bool {
    ev.kind == kind && ev.files.iter().any(|f| f.path == path)
}

/// Predicate helper: does `ev` carry `[DirectoryChanged]` with `kind` and `path`?
fn is_dir_event_of_kind(
    ev: &DirectoryChanged,
    kind: DirectoryChangedKind,
    path: &std::path::Path,
) -> bool {
    ev.kind == kind && ev.path == path
}

#[test]
fn test_watcher_new_creates_struct() {
    let config = AppConfig::default();
    let (tx, _rx) = tokio::sync::broadcast::channel(16);
    let tx = BackgroundEventSender::new(tx);
    let file_bus = Bus::new();
    let dir_bus = Bus::new();
    let (tx_pdf, _rx_pdf) = std::sync::mpsc::channel();
    #[cfg(feature = "image-library")]
    let (tx_img, _rx_img) = std::sync::mpsc::channel();
    let slot = Arc::new(Mutex::new(None));

    #[cfg(feature = "image-library")]
    let _watcher = FileWatcher::new(config, tx, file_bus, dir_bus, tx_pdf, tx_img, slot);
    #[cfg(not(feature = "image-library"))]
    let _watcher = FileWatcher::new(config, tx, file_bus, dir_bus, tx_pdf, slot);
}

#[tokio::test]
async fn test_watcher_start_sends_finished() {
    let mut config = AppConfig::default();
    let dir = tempdir().unwrap();
    config.content_libraries.push(ContentLibrary {
        name: "test".to_string(),
        kind: "text".to_string(),
        root_folder: dir.path().to_string_lossy().to_string(),
        readonly: true,
        priority: 0,
    });

    let (tx, mut rx) = tokio::sync::broadcast::channel(16);
    let tx = BackgroundEventSender::new(tx);
    let file_bus = Bus::new();
    let dir_bus = Bus::new();
    let (tx_pdf, _rx_pdf) = std::sync::mpsc::channel();
    #[cfg(feature = "image-library")]
    let (tx_img, _rx_img) = std::sync::mpsc::channel();
    let slot = Arc::new(Mutex::new(None));

    #[cfg(feature = "image-library")]
    let mut watcher = FileWatcher::new(config, tx, file_bus, dir_bus, tx_pdf, tx_img, slot.clone());
    #[cfg(not(feature = "image-library"))]
    let mut watcher = FileWatcher::new(config, tx, file_bus, dir_bus, tx_pdf, slot.clone());
    watcher.start();

    let event = rx.recv().await;
    assert!(event.is_ok());
    match event.unwrap() {
        BackgroundEvent::Fs(FsEvent::Finished) => {
            // The watcher handle should now be in the slot.
            assert!(slot.lock().unwrap().is_some());
        }
        BackgroundEvent::Fs(FsEvent::FinishedWithoutWatcher) => {
            // Slot stays empty when watcher init failed.
            assert!(slot.lock().unwrap().is_none());
        }
        other => panic!(
            "Expected Finished or FinishedWithoutWatcher, got {:?}",
            other
        ),
    }
}

/// Spawns a watcher over `dir`; returns the watcher plus **both** event
/// buses (one bus per event type).
fn spawn_watcher_on(
    dir: &std::path::Path,
) -> (FileWatcher, Bus<FileChanged>, Bus<DirectoryChanged>) {
    let mut config = AppConfig::default();
    config.content_libraries.push(ContentLibrary {
        name: "test".to_string(),
        kind: "text".to_string(),
        root_folder: dir.to_string_lossy().to_string(),
        readonly: true,
        priority: 0,
    });
    let (tx, _rx) = tokio::sync::broadcast::channel(16);
    let tx = BackgroundEventSender::new(tx);
    let file_bus = Bus::new();
    let dir_bus = Bus::new();
    let (tx_pdf, _rx_pdf) = std::sync::mpsc::channel();
    #[cfg(feature = "image-library")]
    let (tx_img, _rx_img) = std::sync::mpsc::channel();
    let slot = Arc::new(Mutex::new(None));

    #[cfg(feature = "image-library")]
    let watcher = FileWatcher::new(
        config,
        tx,
        file_bus.clone(),
        dir_bus.clone(),
        tx_pdf,
        tx_img,
        slot,
    );
    #[cfg(not(feature = "image-library"))]
    let watcher = FileWatcher::new(config, tx, file_bus.clone(), dir_bus.clone(), tx_pdf, slot);
    (watcher, file_bus, dir_bus)
}

fn wait_for_bus_event<T: Clone + Send + 'static>(
    reader: &crate::bus::core::BusReader<T>,
    pred: impl Fn(&T) -> bool,
    timeout: std::time::Duration,
    label: &str,
) -> T {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        match reader.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(ev) if pred(&ev) => return ev,
            Ok(_) | Err(_) => continue,
        }
    }
    panic!("no matching {label} observed within timeout");
}

#[test]
fn test_watcher_routes_md_create_modify_remove() {
    let dir = tempdir().unwrap();
    let (mut watcher, file_bus, _dir_bus) = spawn_watcher_on(dir.path());
    let reader = file_bus.subscribe();
    watcher.start();

    let file = dir.path().join("notes.md");
    std::fs::write(&file, "hello").unwrap();

    // Create -> `[FileChanged][Added]` on the file bus (REQ-332).
    wait_for_bus_event(
        &reader,
        |e| is_file_event_of_kind(e, FileChangedKind::Added, &file),
        std::time::Duration::from_secs(5),
        "FileChanged[Added]",
    );

    // Modify the file -> another FileModified.
    std::thread::sleep(std::time::Duration::from_millis(50));
    std::fs::write(&file, "hello world").unwrap();
    wait_for_bus_event(
        &reader,
        |e| is_file_event_of_kind(e, FileChangedKind::Changed, &file),
        std::time::Duration::from_secs(5),
        "FileChanged[Changed]",
    );

    // Remove -> `[FileChanged][Deleted]`.
    std::fs::remove_file(&file).unwrap();
    wait_for_bus_event(
        &reader,
        |e| is_file_event_of_kind(e, FileChangedKind::Deleted, &file),
        std::time::Duration::from_secs(5),
        "FileChanged[Deleted]",
    );
}

#[test]
fn test_watcher_publishes_directory_events() {
    // [REQ-402]/[REQ-322] Directory create/remove emit kind-homogeneous
    // `[DirectoryChanged]` events on the directory bus with the fully
    // qualified path ([REQ-321]).
    let dir = tempdir().unwrap();
    let (mut watcher, _file_bus, dir_bus) = spawn_watcher_on(dir.path());
    let reader = dir_bus.subscribe();
    watcher.start();

    let subdir = dir.path().join("newdocs");
    std::fs::create_dir_all(&subdir).unwrap();
    wait_for_bus_event(
        &reader,
        |e| is_dir_event_of_kind(e, DirectoryChangedKind::Added, &subdir),
        std::time::Duration::from_secs(5),
        "DirectoryChanged[Added]",
    );

    std::fs::remove_dir(&subdir).unwrap();
    wait_for_bus_event(
        &reader,
        |e| is_dir_event_of_kind(e, DirectoryChangedKind::Deleted, &subdir),
        std::time::Duration::from_secs(5),
        "DirectoryChanged[Deleted]",
    );
}

#[test]
fn test_watcher_rename_is_delete_then_create() {
    // [REQ-404] A rename must surface as a Deleted(old) followed by an
    // Added(new), regardless of how the backend delivers the notification.
    let dir = tempdir().unwrap();
    let (mut watcher, file_bus, _dir_bus) = spawn_watcher_on(dir.path());
    let reader = file_bus.subscribe();
    watcher.start();

    let old = dir.path().join("before.md");
    let new = dir.path().join("after.md");
    std::fs::write(&old, "hello").unwrap();
    wait_for_bus_event(
        &reader,
        |e| is_file_event_of_kind(e, FileChangedKind::Added, &old),
        std::time::Duration::from_secs(5),
        "FileChanged[Added] for old",
    );

    std::fs::rename(&old, &new).unwrap();
    wait_for_bus_event(
        &reader,
        |e| is_file_event_of_kind(e, FileChangedKind::Deleted, &old),
        std::time::Duration::from_secs(5),
        "FileChanged[Deleted] for old",
    );
    wait_for_bus_event(
        &reader,
        |e| is_file_event_of_kind(e, FileChangedKind::Added, &new),
        std::time::Duration::from_secs(5),
        "FileChanged[Added] for new",
    );
}

#[test]
fn test_watcher_ignores_git_directory_and_non_workspace_files() {
    let dir = tempdir().unwrap();
    let (mut watcher, file_bus, dir_bus) = spawn_watcher_on(dir.path());
    let reader = file_bus.subscribe();
    let dir_reader = dir_bus.subscribe();
    watcher.start();

    let git_dir = dir.path().join(".git");
    std::fs::create_dir_all(&git_dir).unwrap();
    std::fs::write(git_dir.join("config"), "x").unwrap();

    let bin = dir.path().join("program.exe");
    std::fs::write(&bin, "not text").unwrap();

    // The watcher must not surface .git or non-workspace extensions on
    // either event bus.
    std::thread::sleep(std::time::Duration::from_millis(700));
    let mut got = 0;
    while reader.try_recv().is_ok() {
        got += 1;
    }
    while dir_reader.try_recv().is_ok() {
        got += 1;
    }
    assert_eq!(got, 0, "unexpected file-system events published");
}

#[test]
fn test_watcher_routes_pdf_to_converter_queue() {
    let dir = tempdir().unwrap();
    let mut config = AppConfig::default();
    config.content_libraries.push(ContentLibrary {
        name: "test".to_string(),
        kind: "text".to_string(),
        root_folder: dir.path().to_string_lossy().to_string(),
        readonly: true,
        priority: 0,
    });
    let (tx, _rx) = tokio::sync::broadcast::channel(16);
    let tx = BackgroundEventSender::new(tx);
    let file_bus = Bus::new();
    let dir_bus = Bus::new();
    let (tx_pdf, rx_pdf) = std::sync::mpsc::channel();
    #[cfg(feature = "image-library")]
    let (tx_img, _rx_img) = std::sync::mpsc::channel();
    let slot = Arc::new(Mutex::new(None));

    #[cfg(feature = "image-library")]
    let mut watcher = FileWatcher::new(config, tx, file_bus, dir_bus, tx_pdf, tx_img, slot);
    #[cfg(not(feature = "image-library"))]
    let mut watcher = FileWatcher::new(config, tx, file_bus, dir_bus, tx_pdf, slot);
    watcher.start();

    // Minimal valid PDF so `should_convert` does not depend on metadata.
    let pdf = dir.path().join("doc.pdf");
    std::fs::write(&pdf, b"%PDF-1.4\n%%EOF\n").unwrap();

    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(5) {
        match rx_pdf.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(p) if p == pdf => return,
            Ok(_) | Err(_) => continue,
        }
    }
    panic!("PDF path never reached the converter queue");
}
