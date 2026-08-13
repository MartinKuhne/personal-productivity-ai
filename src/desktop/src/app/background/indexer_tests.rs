//! Tests for `background/indexer.rs`.

use super::*;
use crate::bus::events::file::FileEventKind;
use crate::config::{AppConfig, ContentLibrary};
use tempfile::tempdir;

#[test]
fn test_scan_libraries_discovers_md() {
    let mut config = AppConfig::default();
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("a.md"), "test").unwrap();
    std::fs::write(dir.path().join("b.md"), "test").unwrap();

    config.content_libraries.push(ContentLibrary {
        name: "test".to_string(),
        kind: "text".to_string(),
        root_folder: dir.path().to_string_lossy().to_string(),
        readonly: true,
        priority: 0,
    });

    let (tx, _rx) = std::sync::mpsc::channel();
    let bus = Bus::new();
    let reader = bus.subscribe();
    let cancel = Arc::new(AtomicBool::new(false));
    let indexer = Indexer::new(config, tx, bus.clone(), cancel);

    let (tx_work, _rx_work) = std::sync::mpsc::channel();
    let (tx_pdf, _rx_pdf) = std::sync::mpsc::channel();
    #[cfg(feature = "image-library")]
    let (tx_img, _rx_img) = std::sync::mpsc::channel();

    #[cfg(feature = "image-library")]
    indexer.scan_libraries(&tx_work, &tx_pdf, &tx_img);
    #[cfg(not(feature = "image-library"))]
    indexer.scan_libraries(&tx_work, &tx_pdf);

    let mut discovered = Vec::new();
    while let Ok(ev) = reader.recv_timeout(std::time::Duration::from_millis(100)) {
        if ev.kind == FileEventKind::Discovered {
            discovered.extend(ev.paths);
        }
    }
    assert_eq!(discovered.len(), 2);
}

#[test]
fn test_scan_libraries_skips_git() {
    let mut config = AppConfig::default();
    let dir = tempdir().unwrap();
    let git_dir = dir.path().join(".git");
    std::fs::create_dir_all(&git_dir).unwrap();
    std::fs::write(git_dir.join("secret.md"), "secret").unwrap();
    std::fs::write(dir.path().join("visible.md"), "visible").unwrap();

    config.content_libraries.push(ContentLibrary {
        name: "test".to_string(),
        kind: "text".to_string(),
        root_folder: dir.path().to_string_lossy().to_string(),
        readonly: true,
        priority: 0,
    });

    let (tx, _rx) = std::sync::mpsc::channel();
    let bus = Bus::new();
    let reader = bus.subscribe();
    let cancel = Arc::new(AtomicBool::new(false));
    let indexer = Indexer::new(config, tx, bus, cancel);

    let (tx_work, _rx_work) = std::sync::mpsc::channel();
    let (tx_pdf, _rx_pdf) = std::sync::mpsc::channel();
    #[cfg(feature = "image-library")]
    let (tx_img, _rx_img) = std::sync::mpsc::channel();

    #[cfg(feature = "image-library")]
    indexer.scan_libraries(&tx_work, &tx_pdf, &tx_img);
    #[cfg(not(feature = "image-library"))]
    indexer.scan_libraries(&tx_work, &tx_pdf);

    let mut discovered = Vec::new();
    while let Ok(ev) = reader.recv_timeout(std::time::Duration::from_millis(100)) {
        if ev.kind == FileEventKind::Discovered {
            discovered.extend(ev.paths);
        }
    }
    assert_eq!(discovered.len(), 1);
    assert!(discovered[0].ends_with("visible.md"));
}

#[test]
fn test_scan_libraries_queues_pdf() {
    let mut config = AppConfig::default();
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("report.pdf"), b"pdf").unwrap();

    config.content_libraries.push(ContentLibrary {
        name: "test".to_string(),
        kind: "text".to_string(),
        root_folder: dir.path().to_string_lossy().to_string(),
        readonly: true,
        priority: 0,
    });

    let (tx, _rx) = std::sync::mpsc::channel();
    let bus = Bus::new();
    let cancel = Arc::new(AtomicBool::new(false));
    let indexer = Indexer::new(config, tx, bus, cancel);

    let (tx_work, _rx_work) = std::sync::mpsc::channel();
    let (tx_pdf, rx_pdf) = std::sync::mpsc::channel();
    #[cfg(feature = "image-library")]
    let (tx_img, _rx_img) = std::sync::mpsc::channel();

    #[cfg(feature = "image-library")]
    indexer.scan_libraries(&tx_work, &tx_pdf, &tx_img);
    #[cfg(not(feature = "image-library"))]
    indexer.scan_libraries(&tx_work, &tx_pdf);

    let pdf = rx_pdf.recv_timeout(std::time::Duration::from_millis(500));
    assert!(pdf.is_ok());
}

#[test]
fn test_spawn_workers_creates_correct_number() {
    let (tx_work, rx_work) = std::sync::mpsc::channel();
    let rx_work = Arc::new(Mutex::new(rx_work));
    let (tx_gui, _rx_gui) = std::sync::mpsc::channel();
    let workers = Indexer::spawn_workers(4, rx_work, tx_gui);
    assert_eq!(workers.len(), 4);
    drop(tx_work);
    for w in workers {
        let _ = w.join();
    }
}

#[test]
fn test_spawn_workers_processes_all_items() {
    // Functional correctness: every item pushed into the work
    // channel must produce a matching `FileParsed` message.
    // The existing `test_spawn_workers_creates_correct_number`
    // only checked the handle count, not the actual work.
    let dir = tempdir().unwrap();
    let mut paths = Vec::new();
    for i in 0..16 {
        let p = dir.path().join(format!("file_{}.md", i));
        std::fs::write(&p, "---\ntags: [a]\n---\nbody").unwrap();
        paths.push(p);
    }

    let (tx_work, rx_work) = std::sync::mpsc::channel();
    let rx_work = Arc::new(Mutex::new(rx_work));
    let (tx_gui, rx_gui) = std::sync::mpsc::channel();

    let workers = Indexer::spawn_workers(4, rx_work, tx_gui);
    for p in &paths {
        tx_work.send(p.clone()).unwrap();
    }
    // Closing the sender signals workers to exit once the queue
    // is drained.
    drop(tx_work);

    let mut parsed_paths: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();
    while let Ok(ev) = rx_gui.recv_timeout(std::time::Duration::from_secs(5)) {
        if let BackgroundEvent::Fs(FsEvent::FileParsed { path, .. }) = ev {
            parsed_paths.insert(path);
        }
    }
    for w in workers {
        let _ = w.join();
    }

    let expected: std::collections::HashSet<PathBuf> = paths.into_iter().collect();
    assert_eq!(parsed_paths, expected);
}

/// Drop-shutdown: workers must terminate promptly when the
/// sender is dropped with no pending work. The previous tests
/// always sent work before dropping the sender; a regression
/// that breaks the channel-close signal would still pass
/// those tests because the queue is non-empty at shutdown.
/// Here we drop with an empty queue and assert every worker
/// join completes within a short budget.
#[test]
fn test_spawn_workers_terminates_when_sender_dropped_with_empty_queue() {
    let (_tx_work, rx_work) = std::sync::mpsc::channel();
    let rx_work = Arc::new(Mutex::new(rx_work));
    let (tx_gui, _rx_gui) = std::sync::mpsc::channel();

    let workers = Indexer::spawn_workers(4, rx_work, tx_gui);
    // Sender is in scope, but no items are sent. Drop the
    // sender now; workers must see the channel close and exit.
    drop(_tx_work);

    let start = std::time::Instant::now();
    for w in workers {
        // join timeout: workers should exit in well under a
        // second on a quiet queue. If a worker hangs, this
        // surfaces as a hang rather than a silent infinite loop.
        let join_budget = std::time::Duration::from_secs(2);
        assert!(
            start.elapsed() < join_budget,
            "workers did not shut down within {join_budget:?}"
        );
        let _ = w.join();
    }
}

/// A path that points at a non-existent file must not panic;
/// the worker should produce a `FileParsed` message with an
/// empty tag vector and move on. The existing tests only
/// cover happy paths where the file exists; a missing-file
/// path would surface as a panic that crashes the worker
/// thread and the channel goes silent.
#[test]
fn test_spawn_workers_handles_missing_file() {
    let (tx_work, rx_work) = std::sync::mpsc::channel();
    let rx_work = Arc::new(Mutex::new(rx_work));
    let (tx_gui, rx_gui) = std::sync::mpsc::channel();

    let workers = Indexer::spawn_workers(1, rx_work, tx_gui);
    let missing = PathBuf::from("definitely/does/not/exist/note.md");
    tx_work.send(missing.clone()).unwrap();
    drop(tx_work);

    let mut got_file_parsed = false;
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(2) {
        if let Ok(BackgroundEvent::Fs(FsEvent::FileParsed { path, tags })) =
            rx_gui.recv_timeout(std::time::Duration::from_millis(100))
        {
            assert_eq!(path, missing);
            assert!(
                tags.is_empty(),
                "missing file must produce an empty tag list, got {tags:?}"
            );
            got_file_parsed = true;
            break;
        }
    }
    assert!(
        got_file_parsed,
        "worker must produce a FileParsed message for a missing file"
    );
    for w in workers {
        let _ = w.join();
    }
}
