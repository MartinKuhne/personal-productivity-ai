use crate::file_events::{Bus, FileEvent};
use crate::messages::BackgroundMessage;
use notify::Watcher;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

/// Handle to the background indexing task.
///
/// The task is split into two producer roles (initial scan, file system
/// watcher) that publish [`FileEvent`]s to a shared [`Bus<FileEvent>`]
/// which the rest of the application (tag manager, directory tree) can
/// subscribe to. The bus is exposed here so callers can wire up
/// consumers before the task is spawned.
pub struct Task {
    pub rx: Receiver<BackgroundMessage>,
    pub tx: Sender<BackgroundMessage>,
    pub file_event_bus: Bus<FileEvent>,
    pub _watcher: Option<notify::RecommendedWatcher>,
}

impl Task {
    pub fn new(config: crate::config::AppConfig) -> Self {
        let (tx, rx) = channel();
        let tx_clone = tx.clone();
        let file_event_bus = Bus::new();

        let config_clone = config.clone();
        let bus_clone = file_event_bus.clone();
        std::thread::spawn(move || {
            Self::run_indexing(config_clone, tx_clone, bus_clone);
        });

        Self {
            rx,
            tx,
            file_event_bus,
            _watcher: None,
        }
    }

    fn run_indexing(
        config: crate::config::AppConfig,
        tx: Sender<BackgroundMessage>,
        file_event_bus: Bus<FileEvent>,
    ) {
        let (tx_work, rx_work) = channel::<PathBuf>();
        let rx_work = std::sync::Arc::new(std::sync::Mutex::new(rx_work));

        let (tx_pdf, rx_pdf) = channel::<PathBuf>();
        let cmd_template = config.pdf_converter_command.clone();
        let tx_gui_pdf = tx.clone();
        std::thread::spawn(move || {
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                rt.block_on(async {
                    while let Ok(path) = rx_pdf.recv() {
                        let job = crate::background::PdfConversionJob::new(path);
                        if job.should_convert() {
                            let _ = job.execute(cmd_template.clone(), tx_gui_pdf.clone()).await;
                        }
                    }
                });
            }
        });

        let (tx_img, rx_img) = channel::<PathBuf>();
        let tx_gui_img = tx.clone();
        let config_img = config.clone();
        std::thread::spawn(move || {
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                rt.block_on(async {
                    while let Ok(path) = rx_img.recv() {
                        let job = crate::background::models::ImageJob::new(path);
                        if job.should_process() {
                            let _ = crate::background::vision_processor::process_image(job, config_img.clone(), tx_gui_img.clone()).await;
                        }
                    }
                });
            }
        });

        let mut workers = Vec::new();
        for _ in 0..4 {
            let rx_work_clone = rx_work.clone();
            let tx_gui_clone = tx.clone();
            let handle = std::thread::spawn(move || {
                loop {
                    let path = {
                        let rx = rx_work_clone.lock().unwrap();
                        match rx.recv() {
                            Ok(p) => p,
                            Err(e) => {
                                tracing::info!(
                                    name = "background_task.worker_shutdown",
                                    error = %e,
                                    "Worker channel closed. Shutting down worker thread."
                                );
                                break;
                            }
                        }
                    };

                    let tags = crate::utils::tags::extract_tags_from_file(&path);
                    let _ = tx_gui_clone.send(BackgroundMessage::FileParsed { path, tags });
                    std::thread::yield_now();
                }
            });
            workers.push(handle);
        }

        let mut files_scanned = 0;
        let mut pdfs_queued = 0;
        let mut images_queued = 0;
        let mut last_log_time = std::time::Instant::now();

        // ----------------------------------------------------------------
        // Initial scan — produces FileEvent::Discovered to the bus.
        // ----------------------------------------------------------------
        for lib in &config.content_libraries {
            let is_image_lib = lib.kind == "image";
            let root_path = PathBuf::from(&lib.root_folder);
            let walker = walkdir::WalkDir::new(&root_path)
                .into_iter()
                .filter_entry(|e| e.file_name() != ".git");

            for entry in walker.filter_map(|e| e.ok()) {
                files_scanned += 1;
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        let ext_str = ext.to_string_lossy().to_lowercase();
                        if ext_str == "md" || ext_str == "markdown" || ext_str == "txt" {
                            // Producer: emit a Discovered event to the bus.
                            file_event_bus
                                .publish(FileEvent::discovered(path.to_path_buf()));
                            let _ = tx_work.send(path.to_path_buf());
                        } else if ext_str == "pdf" {
                            let job = crate::background::PdfConversionJob::new(path.to_path_buf());
                            if job.should_convert() {
                                pdfs_queued += 1;
                                let _ = tx_pdf.send(path.to_path_buf());
                            }
                        } else if matches!(ext_str.as_str(), "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tiff" | "avif") {
                            if is_image_lib {
                                let job = crate::background::models::ImageJob::new(path.to_path_buf());
                                if job.should_process() {
                                    images_queued += 1;
                                    let _ = tx_img.send(path.to_path_buf());
                                }
                            }
                        }
                    }
                } else if path.is_dir() {
                    let _ = tx.send(BackgroundMessage::DirParsed { path: path.to_path_buf() });
                }

                if files_scanned % 500 == 0 || last_log_time.elapsed().as_secs() >= 5 {
                    let _ = tx.send(BackgroundMessage::LogEntry(crate::background::BackgroundLogEntry::new(
                        crate::background::LogCategory::Indexer,
                        format!("Scanned {} files, queued {} PDFs, queued {} images", files_scanned, pdfs_queued, images_queued)
                    )));
                    last_log_time = std::time::Instant::now();
                }
                if files_scanned % 50 == 0 {
                    std::thread::yield_now();
                }
            }
        }

        let _ = tx.send(BackgroundMessage::LogEntry(crate::background::BackgroundLogEntry::new(
            crate::background::LogCategory::Indexer,
            format!("Initial indexing complete. Scanned {} files, queued {} PDFs, queued {} images.", files_scanned, pdfs_queued, images_queued)
        )));

        drop(tx_work);

        for worker in workers {
            let _ = worker.join();
        }

        // ----------------------------------------------------------------
        // File system watcher — produces FileEvent::Updated and
        // FileEvent::Removed to the bus.
        // ----------------------------------------------------------------
        let tx_notify = tx.clone();
        let config_watcher = config.clone();
        let tx_pdf_watcher = tx_pdf.clone();
        let tx_img_watcher = tx_img.clone();
        let bus_watcher = file_event_bus.clone();
        let watcher_result = notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                for path in event.paths {
                    if path.components().any(|c| c.as_os_str() == ".git") {
                        continue;
                    }

                    let event_type = match event.kind {
                        notify::EventKind::Create(_) => "created",
                        notify::EventKind::Modify(_) => "modified",
                        notify::EventKind::Remove(_) => "deleted",
                        _ => "changed",
                    };

                    let mut is_image_lib = false;
                    for lib in &config_watcher.content_libraries {
                        let lib_path = std::path::PathBuf::from(&lib.root_folder);
                        if lib.kind == "image" && path.starts_with(&lib_path) {
                            is_image_lib = true;
                            break;
                        }
                    }

                    let mut is_md = false;
                    let mut is_pdf = false;
                    let mut is_img = false;
                    if let Some(ext) = path.extension() {
                        let ext_str = ext.to_string_lossy().to_lowercase();
                        if ext_str == "md" || ext_str == "markdown" || ext_str == "txt" {
                            is_md = true;
                        } else if ext_str == "pdf" {
                            is_pdf = true;
                        } else if matches!(ext_str.as_str(), "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tiff" | "avif") {
                            if is_image_lib {
                                is_img = true;
                            }
                        }
                    }

                    if is_md || is_pdf || is_img {
                        let _ = tx_notify.send(BackgroundMessage::LogEntry(crate::background::BackgroundLogEntry::new(
                            crate::background::LogCategory::Watcher,
                            format!("File {} {:?}", event_type, path.file_name().unwrap_or_default())
                        )));
                    }

                    if is_md {
                        match event.kind {
                            notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {
                                if path.is_file() {
                                    let tags = crate::utils::tags::extract_tags_from_file(&path);
                                    let _ = tx_notify.send(
                                        BackgroundMessage::FileModified {
                                            path: path.clone(),
                                            tags,
                                        },
                                    );
                                    // Producer: emit an Updated event to the bus.
                                    bus_watcher
                                        .publish(FileEvent::updated(path.clone()));
                                }
                            }
                            notify::EventKind::Remove(_) => {
                                let _ =
                                    tx_notify.send(BackgroundMessage::FileDeleted {
                                        path: path.clone(),
                                    });
                                // Producer: emit a Removed event to the bus.
                                bus_watcher
                                    .publish(FileEvent::removed(path.clone()));
                            }
                            _ => {}
                        }
                    } else if is_pdf {
                        match event.kind {
                            notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {
                                let job = crate::background::PdfConversionJob::new(path.clone());
                                if job.should_convert() {
                                    let _ = tx_pdf_watcher.send(path.clone());
                                }
                            }
                            _ => {}
                        }
                    } else if is_img {
                        match event.kind {
                            notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {
                                let job = crate::background::models::ImageJob::new(path.clone());
                                if job.should_process() {
                                    let _ = tx_img_watcher.send(path.clone());
                                }
                            }
                            _ => {}
                        }
                    } else if !path.exists() {
                        let _ = tx_notify
                            .send(BackgroundMessage::FileDeleted { path: path.clone() });
                        // Producer: emit a Removed event to the bus.
                        bus_watcher
                            .publish(FileEvent::removed(path.clone()));
                    }
                }
            }
        });

        if let Ok(mut watcher) = watcher_result {
            for lib in &config.content_libraries {
                let root_path = PathBuf::from(&lib.root_folder);
                if let Err(e) = watcher.watch(&root_path, notify::RecursiveMode::Recursive) {
                    tracing::error!(
                        name = "background_task.watch_dir_failed",
                        path = %root_path.display(),
                        error = %e,
                        "Failed to watch directory. File changes in this directory will not be detected. Likely cause: permissions or missing directory. Operator should check directory permissions."
                    );
                }
            }
            let _ = tx.send(BackgroundMessage::Finished(watcher));
        } else if let Err(e) = watcher_result {
            tracing::error!(
                name = "background_task.watcher_init_failed",
                error = %e,
                "Failed to initialize file system watcher. Changes will not be detected. Likely cause: OS limits on open files or permissions."
            );
            let _ = tx.send(BackgroundMessage::FinishedWithoutWatcher);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, ContentLibrary};
    use tempfile::tempdir;

    #[test]
    fn test_background_task_new_no_libraries() {
        let config = AppConfig::default();
        let task = Task::new(config);

        let mut got_finished = false;
        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < 5 {
            if let Ok(msg) = task.rx.recv_timeout(std::time::Duration::from_millis(100)) {
                if let BackgroundMessage::Finished(_) | BackgroundMessage::FinishedWithoutWatcher = msg {
                    got_finished = true;
                    break;
                }
            }
        }
        assert!(got_finished, "Should complete initialization");
    }

    #[test]
    fn test_background_task_indexing() {
        let mut config = AppConfig::default();
        let dir = tempdir().unwrap();
        let md = dir.path().join("test.md");
        std::fs::write(&md, "test").unwrap();
        let pdf = dir.path().join("test.pdf");
        std::fs::write(&pdf, "pdf").unwrap();

        config.content_libraries.push(ContentLibrary {
            name: "test".to_string(),
            kind: "text".to_string(),
            root_folder: dir.path().to_string_lossy().to_string(),
            readonly: true,
            priority: 0,
        });

        let task = Task::new(config);

        let mut got_finished = false;
        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < 5 {
            if let Ok(msg) = task.rx.recv_timeout(std::time::Duration::from_millis(100)) {
                match msg {
                    BackgroundMessage::Finished(_) | BackgroundMessage::FinishedWithoutWatcher => {
                        got_finished = true;
                        break;
                    }
                    _ => {}
                }
            }
        }
        assert!(got_finished, "Should complete indexing");
    }

    #[test]
    fn test_initial_scan_publishes_discovered_events() {
        // The initial scan must publish FileEvent::Discovered for every
        // markdown file in the configured library.
        let mut config = AppConfig::default();
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.md"), "test").unwrap();
        std::fs::write(dir.path().join("b.md"), "test").unwrap();
        std::fs::write(dir.path().join("c.txt"), "test").unwrap();

        config.content_libraries.push(ContentLibrary {
            name: "test".to_string(),
            kind: "text".to_string(),
            root_folder: dir.path().to_string_lossy().to_string(),
            readonly: true,
            priority: 0,
        });

        let task = Task::new(config);

        // Subscribe to the bus BEFORE the initial scan finishes so we
        // don't miss the Discovered events. The scan publishes them on
        // a worker thread, so we need to wait for the scan to complete
        // before checking the reader.
        let reader = task.file_event_bus.subscribe();

        // Wait for the initial scan + workers to finish.
        let mut got_finished = false;
        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < 5 {
            if let Ok(msg) = task.rx.recv_timeout(std::time::Duration::from_millis(100)) {
                if let BackgroundMessage::Finished(_) | BackgroundMessage::FinishedWithoutWatcher = msg {
                    got_finished = true;
                    break;
                }
            }
        }
        assert!(got_finished, "Should complete initialization");

        // Drain the bus and collect events.
        let mut events = Vec::new();
        while let Ok(ev) = reader.recv_timeout(std::time::Duration::from_millis(100)) {
            events.push(ev);
        }

        // The indexer queues .md, .markdown, and .txt files for tag
        // extraction, so all three are reported on the bus as
        // Discovered. Consumers that only care about Markdown (the
        // directory tree) can filter on extension.
        let discovered: Vec<_> = events
            .iter()
            .filter(|e| e.kind == crate::file_events::FileEventKind::Discovered)
            .collect();
        assert_eq!(discovered.len(), 3);
        let mut names: Vec<String> = discovered
            .iter()
            .map(|e| e.path.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        names.sort();
        assert_eq!(names, vec!["a.md", "b.md", "c.txt"]);
    }

    #[test]
    fn test_bus_subscribers_see_discovered_events() {
        // The directory tree and the tag manager are both consumers
        // of the bus. Make sure both subscribers receive the same
        // events from the initial scan.
        let mut config = AppConfig::default();
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.md"), "test").unwrap();

        config.content_libraries.push(ContentLibrary {
            name: "test".to_string(),
            kind: "text".to_string(),
            root_folder: dir.path().to_string_lossy().to_string(),
            readonly: true,
            priority: 0,
        });

        let task = Task::new(config);

        // Two consumers: tag manager and directory tree.
        let tag_reader = task.file_event_bus.subscribe();
        let tree_reader = task.file_event_bus.subscribe();

        // Drain initial events on both consumers.
        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < 5 {
            if let Ok(msg) = task.rx.recv_timeout(std::time::Duration::from_millis(100)) {
                if let BackgroundMessage::Finished(_) | BackgroundMessage::FinishedWithoutWatcher = msg {
                    break;
                }
            }
        }

        // Both consumers should have received at least one Discovered
        // event for `a.md`.
        let mut tag_events = Vec::new();
        while let Ok(ev) = tag_reader.recv_timeout(std::time::Duration::from_millis(100)) {
            tag_events.push(ev);
        }
        let mut tree_events = Vec::new();
        while let Ok(ev) = tree_reader.recv_timeout(std::time::Duration::from_millis(100)) {
            tree_events.push(ev);
        }

        assert_eq!(tag_events.len(), 1);
        assert_eq!(tree_events.len(), 1);
        assert_eq!(tag_events[0].path, tree_events[0].path);
        assert_eq!(
            tag_events[0].kind,
            crate::file_events::FileEventKind::Discovered
        );
    }
}
