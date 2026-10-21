//! Tests for the initial recursive scanner.

use super::*;
use crate::bus::events::directory::DirectoryChangedKind;
use crate::bus::events::file::FileChangedKind;
use crate::bus::events::typed::{BackgroundEvent, BackgroundEventSender, FsEvent};
use crate::config::{AppConfig, ContentLibrary};
use tempfile::tempdir;

fn scan(config: AppConfig, file_bus: Bus<FileChanged>, dir_bus: Bus<DirectoryChanged>) {
    let (tx, _rx) = tokio::sync::broadcast::channel(64);
    let indexer = Indexer::new(
        config,
        BackgroundEventSender::new(tx),
        file_bus,
        dir_bus,
        Arc::new(AtomicBool::new(false)),
    );
    #[cfg(feature = "image-library")]
    {
        let (tx_img, _rx_img) = std::sync::mpsc::channel();
        indexer.scan_libraries(&tx_img);
    }
    #[cfg(not(feature = "image-library"))]
    indexer.scan_libraries();
}

#[test]
fn scan_publishes_supported_text_files_as_added_events() {
    let directory = tempdir().unwrap();
    std::fs::write(directory.path().join("a.md"), "test").unwrap();
    std::fs::write(directory.path().join("b.markdown"), "test").unwrap();
    std::fs::write(directory.path().join("c.txt"), "test").unwrap();
    let mut config = AppConfig::default();
    config.content_libraries.push(ContentLibrary {
        name: "test".to_string(),
        kind: "text".to_string(),
        root_folder: directory.path().to_string_lossy().to_string(),
        readonly: true,
        priority: 0,
    });

    let file_bus = Bus::new();
    let reader = file_bus.subscribe();
    scan(config, file_bus, Bus::new());

    let mut paths = Vec::new();
    while let Ok(event) = reader.recv_timeout(std::time::Duration::from_millis(100)) {
        assert_eq!(event.kind, FileChangedKind::Added);
        paths.extend(event.paths());
    }
    assert_eq!(paths.len(), 3);
    assert!(paths.contains(&directory.path().join("a.md")));
    assert!(paths.contains(&directory.path().join("b.markdown")));
    assert!(paths.contains(&directory.path().join("c.txt")));
}

#[test]
fn scan_publishes_one_directory_event_per_enumerated_directory() {
    let directory = tempdir().unwrap();
    let child = directory.path().join("docs");
    std::fs::create_dir_all(&child).unwrap();
    std::fs::write(child.join("note.md"), "test").unwrap();
    let mut config = AppConfig::default();
    config.content_libraries.push(ContentLibrary {
        name: "test".to_string(),
        kind: "text".to_string(),
        root_folder: directory.path().to_string_lossy().to_string(),
        readonly: true,
        priority: 0,
    });

    let file_bus = Bus::new();
    let dir_bus = Bus::new();
    let reader = dir_bus.subscribe();
    let (tx, mut events) = tokio::sync::broadcast::channel(64);
    let indexer = Indexer::new(
        config,
        BackgroundEventSender::new(tx),
        file_bus,
        dir_bus,
        Arc::new(AtomicBool::new(false)),
    );
    #[cfg(feature = "image-library")]
    {
        let (tx_img, _rx_img) = std::sync::mpsc::channel();
        indexer.scan_libraries(&tx_img);
    }
    #[cfg(not(feature = "image-library"))]
    indexer.scan_libraries();

    let mut directories = Vec::new();
    while let Ok(event) = reader.recv_timeout(std::time::Duration::from_millis(100)) {
        assert_eq!(event.kind, DirectoryChangedKind::Added);
        directories.push(event.path);
    }
    assert_eq!(directories.len(), 2);
    assert!(directories.contains(&directory.path().to_path_buf()));
    assert!(directories.contains(&child));

    while let Ok(event) = events.try_recv() {
        assert!(!matches!(
            event,
            BackgroundEvent::Fs(FsEvent::DirParsed { .. })
        ));
    }
}

#[test]
fn scan_skips_git_directories() {
    let directory = tempdir().unwrap();
    std::fs::create_dir_all(directory.path().join(".git")).unwrap();
    std::fs::write(directory.path().join(".git/secret.md"), "secret").unwrap();
    std::fs::write(directory.path().join("visible.md"), "visible").unwrap();
    let mut config = AppConfig::default();
    config.content_libraries.push(ContentLibrary {
        name: "test".to_string(),
        kind: "text".to_string(),
        root_folder: directory.path().to_string_lossy().to_string(),
        readonly: true,
        priority: 0,
    });

    let file_bus = Bus::new();
    let reader = file_bus.subscribe();
    scan(config, file_bus, Bus::new());

    let mut paths = Vec::new();
    while let Ok(event) = reader.recv_timeout(std::time::Duration::from_millis(100)) {
        paths.extend(event.paths());
    }
    assert_eq!(paths, vec![directory.path().join("visible.md")]);
}

#[test]
fn scan_publishes_pdf_for_background_manager_routing() {
    let directory = tempdir().unwrap();
    let pdf_path = directory.path().join("report.pdf");
    std::fs::write(&pdf_path, b"pdf").unwrap();
    let mut config = AppConfig::default();
    config.content_libraries.push(ContentLibrary {
        name: "test".to_string(),
        kind: "text".to_string(),
        root_folder: directory.path().to_string_lossy().to_string(),
        readonly: true,
        priority: 0,
    });

    let file_bus = Bus::new();
    let reader = file_bus.subscribe();
    scan(config, file_bus, Bus::new());

    let event = reader
        .recv_timeout(std::time::Duration::from_millis(500))
        .expect("scanner publishes PDFs for the PDF manager");
    assert_eq!(event.kind, FileChangedKind::Added);
    assert_eq!(event.paths(), vec![pdf_path]);
}
