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
    #[cfg(feature = "image-library")]
    let (tx_img, _rx_img) = std::sync::mpsc::channel();
    let slot = Arc::new(Mutex::new(None));

    #[cfg(feature = "image-library")]
    let _watcher = FileWatcher::new(config, tx, file_bus, dir_bus, tx_img, slot);
    #[cfg(not(feature = "image-library"))]
    let _watcher = FileWatcher::new(config, tx, file_bus, dir_bus, slot);
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
    #[cfg(feature = "image-library")]
    let (tx_img, _rx_img) = std::sync::mpsc::channel();
    let slot = Arc::new(Mutex::new(None));

    #[cfg(feature = "image-library")]
    let mut watcher = FileWatcher::new(config, tx, file_bus, dir_bus, tx_img, slot.clone());
    #[cfg(not(feature = "image-library"))]
    let mut watcher = FileWatcher::new(config, tx, file_bus, dir_bus, slot.clone());
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
    #[cfg(feature = "image-library")]
    let (tx_img, _rx_img) = std::sync::mpsc::channel();
    let slot = Arc::new(Mutex::new(None));

    #[cfg(feature = "image-library")]
    let watcher = FileWatcher::new(config, tx, file_bus.clone(), dir_bus.clone(), tx_img, slot);
    #[cfg(not(feature = "image-library"))]
    let watcher = FileWatcher::new(config, tx, file_bus.clone(), dir_bus.clone(), slot);
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
fn test_watcher_publishes_pdf_to_file_event_bus() {
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
    let file_reader = file_bus.subscribe();
    let dir_bus = Bus::new();
    #[cfg(feature = "image-library")]
    let (tx_img, _rx_img) = std::sync::mpsc::channel();
    let slot = Arc::new(Mutex::new(None));

    #[cfg(feature = "image-library")]
    let mut watcher = FileWatcher::new(config, tx, file_bus, dir_bus, tx_img, slot);
    #[cfg(not(feature = "image-library"))]
    let mut watcher = FileWatcher::new(config, tx, file_bus, dir_bus, slot);
    watcher.start();

    // A PDF is published as a file event; the separate PDF manager decides
    // whether a configured converter should receive it.
    let pdf = dir.path().join("doc.pdf");
    std::fs::write(&pdf, b"%PDF-1.4\n%%EOF\n").unwrap();

    let event = wait_for_bus_event(
        &file_reader,
        |event| {
            event.paths().contains(&pdf)
                && matches!(
                    event.kind,
                    FileChangedKind::Added | FileChangedKind::Changed
                )
        },
        std::time::Duration::from_secs(5),
        "PDF file event",
    );
    assert!(event.paths().contains(&pdf));
}

// ---------------------------------------------------------------------------
// `start()` registration paths (no OS watcher events required)
// ---------------------------------------------------------------------------

/// Build an [`AppConfig`] whose content libraries are exactly `roots`.
fn config_with_roots(roots: &[&std::path::Path]) -> AppConfig {
    let mut config = AppConfig::default();
    for (idx, root) in roots.iter().enumerate() {
        config.content_libraries.push(ContentLibrary {
            name: format!("lib{idx}"),
            kind: "text".to_string(),
            root_folder: root.to_string_lossy().to_string(),
            readonly: true,
            priority: 0,
        });
    }
    config
}

/// What [`build_watcher`] hands back: a [`FileWatcher`] plus everything the
/// caller needs to observe it — the two event buses, the typed background
/// receiver and the watcher slot.
type WatcherFixture = (
    FileWatcher,
    Bus<FileChanged>,
    Bus<DirectoryChanged>,
    tokio::sync::broadcast::Receiver<BackgroundEvent>,
    Arc<Mutex<Option<notify::RecommendedWatcher>>>,
);

/// Assemble a [`FileWatcher`] plus everything the caller needs to observe it:
/// the two event buses, the typed background receiver and the watcher slot.
fn build_watcher(config: AppConfig) -> WatcherFixture {
    let (tx, rx) = tokio::sync::broadcast::channel(16);
    let tx = BackgroundEventSender::new(tx);
    let file_bus = Bus::new();
    let dir_bus = Bus::new();
    let slot: Arc<Mutex<Option<notify::RecommendedWatcher>>> = Arc::new(Mutex::new(None));
    #[cfg(feature = "image-library")]
    let (tx_img, _rx_img) = std::sync::mpsc::channel();
    #[cfg(feature = "image-library")]
    let watcher = FileWatcher::new(
        config,
        tx,
        file_bus.clone(),
        dir_bus.clone(),
        tx_img,
        slot.clone(),
    );
    #[cfg(not(feature = "image-library"))]
    let watcher = FileWatcher::new(config, tx, file_bus.clone(), dir_bus.clone(), slot.clone());
    (watcher, file_bus, dir_bus, rx, slot)
}

#[tokio::test]
async fn test_start_with_no_content_libraries_still_finishes_and_fills_slot() {
    // The registration loop simply does not run; the watcher handle must still
    // be handed to the slot and `Finished` announced.
    let (mut watcher, _file_bus, _dir_bus, mut rx, slot) = build_watcher(AppConfig::default());
    watcher.start();

    let event = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
        .await
        .expect("start() must announce completion")
        .expect("background channel must still be open");
    assert!(
        matches!(event, BackgroundEvent::Fs(FsEvent::Finished)),
        "expected FsEvent::Finished, got {event:?}"
    );
    assert!(
        slot.lock().unwrap().is_some(),
        "the watcher handle must be transferred to the slot"
    );
}

#[tokio::test]
async fn test_start_tolerates_a_missing_library_root() {
    // A content library pointing at a directory that does not exist must make
    // `watch()` fail and log, but must not abort the remaining registrations
    // nor prevent the watcher from being handed over.
    let tmp = tempdir().unwrap();
    let good = tmp.path().join("good");
    std::fs::create_dir(&good).unwrap();
    let missing = tmp.path().join("no-such-library");
    assert!(!missing.exists());

    let config = config_with_roots(&[&missing, &good]);
    let (mut watcher, _file_bus, _dir_bus, mut rx, slot) = build_watcher(config);
    watcher.start();

    let event = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
        .await
        .expect("start() must announce completion even when one root is unwatchable")
        .expect("background channel must still be open");
    assert!(
        matches!(event, BackgroundEvent::Fs(FsEvent::Finished)),
        "expected FsEvent::Finished, got {event:?}"
    );
    assert!(
        slot.lock().unwrap().is_some(),
        "a partially failed registration must still hand the watcher over"
    );
}

#[tokio::test]
async fn test_start_tolerates_a_library_root_that_is_a_regular_file() {
    // `watch()` on a non-directory is an error path, not a panic.
    let tmp = tempdir().unwrap();
    let blocker = tmp.path().join("not-a-directory.md");
    std::fs::write(&blocker, "x").unwrap();

    let config = config_with_roots(&[&blocker]);
    let (mut watcher, _file_bus, _dir_bus, mut rx, slot) = build_watcher(config);
    watcher.start();

    let event = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
        .await
        .expect("start() must announce completion")
        .expect("background channel must still be open");
    assert!(
        matches!(
            event,
            BackgroundEvent::Fs(FsEvent::Finished)
                | BackgroundEvent::Fs(FsEvent::FinishedWithoutWatcher)
        ),
        "expected a terminal FsEvent, got {event:?}"
    );
    let filled = slot.lock().unwrap().is_some();
    assert_eq!(
        matches!(event, BackgroundEvent::Fs(FsEvent::Finished)),
        filled,
        "the slot is only filled when a live watcher handle exists"
    );
}

#[tokio::test]
async fn test_start_twice_replaces_the_handed_over_watcher() {
    // Calling `start()` again builds a second watcher and overwrites the slot;
    // both runs must announce completion.
    let tmp = tempdir().unwrap();
    let config = config_with_roots(&[tmp.path()]);
    let (mut watcher, _file_bus, _dir_bus, mut rx, slot) = build_watcher(config);

    watcher.start();
    let first = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
        .await
        .expect("first start() must announce completion")
        .expect("background channel must still be open");
    assert!(
        matches!(first, BackgroundEvent::Fs(FsEvent::Finished)),
        "expected FsEvent::Finished from the first start, got {first:?}"
    );
    assert!(slot.lock().unwrap().is_some());

    watcher.start();
    let second = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
        .await
        .expect("second start() must announce completion")
        .expect("background channel must still be open");
    assert!(
        matches!(second, BackgroundEvent::Fs(FsEvent::Finished)),
        "expected FsEvent::Finished from the second start, got {second:?}"
    );
    assert!(
        slot.lock().unwrap().is_some(),
        "the slot must hold the most recent watcher handle"
    );
}

#[tokio::test]
async fn test_start_publishes_no_bus_events_without_filesystem_activity() {
    // Registering the watcher alone must not synthesise file or directory
    // events — the buses stay empty until the OS reports a change.
    let tmp = tempdir().unwrap();
    let config = config_with_roots(&[tmp.path()]);
    let (mut watcher, file_bus, dir_bus, mut rx, _slot) = build_watcher(config);
    let file_reader = file_bus.subscribe();
    let dir_reader = dir_bus.subscribe();

    watcher.start();
    let finished = tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
        .await
        .expect("start() must announce completion");
    assert!(finished.is_ok(), "background channel must still be open");

    assert!(
        file_reader.try_recv().is_err(),
        "no FileChanged may be published by registration alone"
    );
    assert!(
        dir_reader.try_recv().is_err(),
        "no DirectoryChanged may be published by registration alone"
    );
}
