use crate::file_events::{Bus, FileEvent};
use crate::messages::BackgroundMessage;
use notify::Watcher;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};

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
        let bus_pdf = file_event_bus.clone();
        std::thread::spawn(move || {
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                rt.block_on(async {
                    while let Ok(path) = rx_pdf.recv() {
                        let job = crate::background::PdfConversionJob::new(path);
                        if job.should_convert() {
                            // Capture the output path before execute()
                            // consumes the job. After a successful
                            // conversion the produced `.md` is on
                            // disk; publish a Discovered event so the
                            // directory tree, tag manager, and render
                            // tab see it without waiting for the
                            // notify watcher to fire.
                            let output_md = job.output_md.clone();
                            if job
                                .execute(cmd_template.clone(), tx_gui_pdf.clone())
                                .await
                                .is_ok()
                            {
                                bus_pdf.publish(FileEvent::discovered(output_md));
                            }
                        }
                    }
                });
            }
        });

        let (tx_img, rx_img) = channel::<PathBuf>();
        let tx_gui_img = tx.clone();
        let config_img = config.clone();
        let bus_img = file_event_bus.clone();
        std::thread::spawn(move || {
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                rt.block_on(async {
                    while let Ok(path) = rx_img.recv() {
                        let job = crate::background::models::ImageJob::new(path);
                        if job.should_process() {
                            // The vision processor writes a `.md`
                            // derived from the image. It takes a
                            // `FileEventProducer` so it can publish a
                            // Discovered event on success — same
                            // pattern as the PDF worker above.
                            let producer = crate::file_events::FileEventProducer::new(&bus_img);
                            let _ = crate::background::vision_processor::process_image(
                                job,
                                config_img.clone(),
                                tx_gui_img.clone(),
                                &producer,
                            )
                            .await;
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
                            file_event_bus.publish(FileEvent::discovered(path.to_path_buf()));
                            let _ = tx_work.send(path.to_path_buf());
                        } else if ext_str == "pdf" {
                            // Producer: publish the PDF to the bus so the
                            // PDF-converter consumer (subscribed below)
                            // can convert it. We still push to `tx_pdf`
                            // directly as a fast path — the converter
                            // worker deduplicates via `should_convert()`.
                            file_event_bus.publish(FileEvent::discovered(path.to_path_buf()));
                            let job = crate::background::PdfConversionJob::new(path.to_path_buf());
                            if job.should_convert() {
                                pdfs_queued += 1;
                                let _ = tx_pdf.send(path.to_path_buf());
                            }
                        } else if matches!(
                            ext_str.as_str(),
                            "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tiff" | "avif"
                        ) {
                            if is_image_lib {
                                let job =
                                    crate::background::models::ImageJob::new(path.to_path_buf());
                                if job.should_process() {
                                    images_queued += 1;
                                    let _ = tx_img.send(path.to_path_buf());
                                }
                            }
                        }
                    }
                } else if path.is_dir() {
                    let _ = tx.send(BackgroundMessage::DirParsed {
                        path: path.to_path_buf(),
                    });
                }

                if files_scanned % 500 == 0 || last_log_time.elapsed().as_secs() >= 5 {
                    let _ = tx.send(BackgroundMessage::LogEntry(
                        crate::background::BackgroundLogEntry::new(
                            crate::background::LogCategory::Indexer,
                            format!(
                                "Scanned {} files, queued {} PDFs, queued {} images",
                                files_scanned, pdfs_queued, images_queued
                            ),
                        ),
                    ));
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
        let watcher_result =
            notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
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
                            } else if matches!(
                                ext_str.as_str(),
                                "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tiff" | "avif"
                            ) {
                                if is_image_lib {
                                    is_img = true;
                                }
                            }
                        }

                        if is_md || is_pdf || is_img {
                            let _ = tx_notify.send(BackgroundMessage::LogEntry(
                                crate::background::BackgroundLogEntry::new(
                                    crate::background::LogCategory::Watcher,
                                    format!(
                                        "File {} {:?}",
                                        event_type,
                                        path.file_name().unwrap_or_default()
                                    ),
                                ),
                            ));
                        }

                        if is_md {
                            match event.kind {
                                notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {
                                    if path.is_file() {
                                        let tags =
                                            crate::utils::tags::extract_tags_from_file(&path);
                                        let _ = tx_notify.send(BackgroundMessage::FileModified {
                                            path: path.clone(),
                                            tags,
                                        });
                                        // Producer: emit an Updated event to the bus.
                                        bus_watcher.publish(FileEvent::updated(path.clone()));
                                    }
                                }
                                notify::EventKind::Remove(_) => {
                                    let _ = tx_notify.send(BackgroundMessage::FileDeleted {
                                        path: path.clone(),
                                    });
                                    // Producer: emit a Removed event to the bus.
                                    bus_watcher.publish(FileEvent::removed(path.clone()));
                                }
                                _ => {}
                            }
                        } else if is_pdf {
                            match event.kind {
                                notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {
                                    // Producer: publish to the bus so the
                                    // PDF-converter consumer (subscribed
                                    // below) can convert it. The direct
                                    // `tx_pdf_watcher.send` remains as a
                                    // fast path.
                                    bus_watcher.publish(FileEvent::updated(path.clone()));
                                    let job =
                                        crate::background::PdfConversionJob::new(path.clone());
                                    if job.should_convert() {
                                        let _ = tx_pdf_watcher.send(path.clone());
                                    }
                                }
                                _ => {}
                            }
                        } else if is_img {
                            match event.kind {
                                notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {
                                    let job =
                                        crate::background::models::ImageJob::new(path.clone());
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
                            bus_watcher.publish(FileEvent::removed(path.clone()));
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

        // ----------------------------------------------------------------
        // Bus-driven PDF-conversion trigger.
        //
        // In addition to the initial scan and the notify watcher (which
        // both push directly to `tx_pdf` for low latency), we also
        // subscribe to the bus so that PDFs created through any other
        // path — e.g. a future tool that creates a PDF, or a UI handler
        // that drops a PDF into a content library — are also converted.
        //
        // The conversion worker calls `should_convert()` before
        // invoking the external tool, so duplicate triggers are
        // de-duplicated for free.
        // ----------------------------------------------------------------
        let bus_reader_pdf = file_event_bus.clone();
        let tx_pdf_bus = tx_pdf.clone();
        std::thread::spawn(move || {
            let reader = bus_reader_pdf.subscribe();
            loop {
                let event = match reader.recv() {
                    Ok(e) => e,
                    Err(_) => break, // bus was dropped
                };
                use crate::file_events::FileEventKind;
                if !matches!(
                    event.kind,
                    FileEventKind::Discovered | FileEventKind::Updated
                ) {
                    continue;
                }
                // Only consider PDF files.
                let is_pdf = event
                    .path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.eq_ignore_ascii_case("pdf"))
                    .unwrap_or(false);
                if !is_pdf {
                    continue;
                }
                if let Err(e) = tx_pdf_bus.send(event.path) {
                    tracing::warn!(
                        name = "background_task.pdf_bus.tx_closed",
                        error = %e,
                        "PDF bus subscriber could not deliver to tx_pdf. Channel is closed."
                    );
                    break;
                }
            }
        });

        // ----------------------------------------------------------------
        // Bus-driven image-vision trigger.
        //
        // Mirror of the PDF bus subscriber above. The image-vision
        // worker consumes from `tx_img`. The initial scan and the
        // notify watcher both push directly to `tx_img` for low
        // latency; this subscriber is the safety net for any other
        // code path that drops a supported image into a content
        // library (e.g. a future tool, or a UI handler).
        //
        // The vision worker calls `should_process()` before invoking
        // the model, so duplicate triggers are de-duplicated.
        // ----------------------------------------------------------------
        let bus_reader_img = file_event_bus.clone();
        let tx_img_bus = tx_img.clone();
        std::thread::spawn(move || {
            let reader = bus_reader_img.subscribe();
            loop {
                let event = match reader.recv() {
                    Ok(e) => e,
                    Err(_) => break, // bus was dropped
                };
                use crate::file_events::FileEventKind;
                if !matches!(
                    event.kind,
                    FileEventKind::Discovered | FileEventKind::Updated
                ) {
                    continue;
                }
                // Only consider image files. Keep the extension list
                // in sync with the initial-scan branch above.
                let is_img = event
                    .path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| {
                        matches!(
                            e.to_lowercase().as_str(),
                            "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "tiff" | "avif"
                        )
                    })
                    .unwrap_or(false);
                if !is_img {
                    continue;
                }
                if let Err(e) = tx_img_bus.send(event.path) {
                    tracing::warn!(
                        name = "background_task.img_bus.tx_closed",
                        error = %e,
                        "Image bus subscriber could not deliver to tx_img. Channel is closed."
                    );
                    break;
                }
            }
        });
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
                if let BackgroundMessage::Finished(_) | BackgroundMessage::FinishedWithoutWatcher =
                    msg
                {
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
                if let BackgroundMessage::Finished(_) | BackgroundMessage::FinishedWithoutWatcher =
                    msg
                {
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
                if let BackgroundMessage::Finished(_) | BackgroundMessage::FinishedWithoutWatcher =
                    msg
                {
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

    #[test]
    fn test_initial_scan_publishes_pdf_discovered_to_bus() {
        // The PDF converter worker consumes from `tx_pdf`. To make
        // sure every path that creates a PDF can route through the
        // bus (not just the notify watcher), the initial scan must
        // publish `Discovered` events for PDFs as well.
        let mut config = AppConfig::default();
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("report.pdf"), b"dummy pdf").unwrap();

        config.content_libraries.push(ContentLibrary {
            name: "test".to_string(),
            kind: "text".to_string(),
            root_folder: dir.path().to_string_lossy().to_string(),
            readonly: true,
            priority: 0,
        });

        let task = Task::new(config);
        let reader = task.file_event_bus.subscribe();

        // Wait for the initial scan to finish.
        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < 5 {
            if let Ok(msg) = task.rx.recv_timeout(std::time::Duration::from_millis(100)) {
                if let BackgroundMessage::Finished(_) | BackgroundMessage::FinishedWithoutWatcher =
                    msg
                {
                    break;
                }
            }
        }

        // Drain events.
        let mut events = Vec::new();
        while let Ok(ev) = reader.recv_timeout(std::time::Duration::from_millis(100)) {
            events.push(ev);
        }

        // There must be a Discovered event for the PDF.
        let pdf_discovered = events
            .iter()
            .find(|e| {
                e.kind == crate::file_events::FileEventKind::Discovered
                    && e.path.extension().and_then(|x| x.to_str()) == Some("pdf")
            })
            .expect("initial scan should publish Discovered for PDFs");
        assert_eq!(pdf_discovered.path, dir.path().join("report.pdf"));
    }

    #[test]
    fn test_bus_published_pdf_triggers_conversion_via_subscriber() {
        // Regression test: a PDF published directly to the bus
        // (without going through the initial scan or the notify
        // watcher) must still be picked up by the PDF-converter
        // worker via the bus subscriber we added.
        //
        // We configure a working pdf_converter_command (the system
        // `echo`/`true`) so that `execute()` actually runs to
        // completion and emits a `Successfully converted` log
        // entry on the `rx` channel. If the bus subscriber did
        // NOT forward the event, the PDF worker would never see
        // the path and no log entry would arrive.
        use crate::background::LogCategory;

        let mut config = AppConfig::default();
        let dir = tempdir().unwrap();

        // Use a real, available shell command so the PDF worker
        // actually executes and sends a success LogEntry.
        #[cfg(windows)]
        let cmd_template = Some(vec![
            "cmd".to_string(),
            "/C".to_string(),
            "echo done".to_string(),
        ]);
        #[cfg(not(windows))]
        let cmd_template = Some(vec!["true".to_string()]);
        config.pdf_converter_command = cmd_template;

        config.content_libraries.push(ContentLibrary {
            name: "test".to_string(),
            kind: "text".to_string(),
            root_folder: dir.path().to_string_lossy().to_string(),
            readonly: true,
            priority: 0,
        });

        let task = Task::new(config);

        // Wait for Finished.
        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < 5 {
            if let Ok(msg) = task.rx.recv_timeout(std::time::Duration::from_millis(100)) {
                if let BackgroundMessage::Finished(_) | BackgroundMessage::FinishedWithoutWatcher =
                    msg
                {
                    break;
                }
            }
        }

        // The bus subscriber forwards the path to `tx_pdf`. The
        // PDF-converter worker dequeues it, calls `should_convert()`
        // (returns true), then calls `execute()`. We expect a
        // `Successfully converted` log entry on the rx channel.
        //
        // Give the subscriber a moment to spin up — `Task::new`
        // spawns the indexing thread and the bus subscriber is
        // only attached near the end of `run_indexing`. After
        // `Finished` arrives, a short sleep ensures the
        // subscriber is alive before we publish.
        std::thread::sleep(std::time::Duration::from_millis(200));

        // Create a PDF on disk and publish a Discovered event for it.
        // (This simulates a code path that bypasses the initial
        // scan and the notify watcher — for example, a future tool
        // that drops a PDF into a content library.) The publish
        // MUST happen AFTER the subscriber is up, otherwise the
        // event is delivered to zero consumers and lost.
        let pdf_path = dir.path().join("dropped.pdf");
        std::fs::write(&pdf_path, b"dummy").unwrap();
        task.file_event_bus
            .publish(crate::file_events::FileEvent::discovered(pdf_path.clone()));

        let mut saw_success = false;
        let start = std::time::Instant::now();
        let mut all_messages: Vec<String> = Vec::new();
        while start.elapsed().as_secs() < 5 {
            if let Ok(msg) = task.rx.recv_timeout(std::time::Duration::from_millis(100)) {
                match msg {
                    BackgroundMessage::LogEntry(entry) => {
                        all_messages.push(format!("{:?}: {}", entry.category, entry.message));
                        if entry.category == LogCategory::PdfConverter
                            && entry.message.contains("Successfully converted")
                        {
                            saw_success = true;
                            break;
                        }
                    }
                    other => {
                        all_messages.push(format!("other: {:?}", std::mem::discriminant(&other)));
                    }
                }
            }
        }
        if !saw_success {
            eprintln!("Test saw messages: {:?}", all_messages);
        }
        assert!(
            saw_success,
            "Bus-published PDF Discovered event should reach the PDF converter worker"
        );
    }

    #[test]
    fn test_pdf_worker_publishes_discovered_for_output_md() {
        // After the PDF worker successfully converts a PDF, it must
        // publish a `Discovered` event for the produced `.md` on the
        // bus so the directory tree, tag manager, and render tab see
        // the new file without waiting for the notify watcher to
        // fire. We use `cmd /C echo done` (or `true` on non-Windows)
        // as the converter command so `execute()` returns Ok
        // regardless of whether the external tool actually produces
        // a file. The publish logic runs on Ok, so the test only
        // cares that the bus receives the event.
        let mut config = AppConfig::default();
        let dir = tempdir().unwrap();

        #[cfg(windows)]
        let cmd_template = Some(vec![
            "cmd".to_string(),
            "/C".to_string(),
            "echo done".to_string(),
        ]);
        #[cfg(not(windows))]
        let cmd_template = Some(vec!["true".to_string()]);
        config.pdf_converter_command = cmd_template;

        config.content_libraries.push(ContentLibrary {
            name: "test".to_string(),
            kind: "text".to_string(),
            root_folder: dir.path().to_string_lossy().to_string(),
            readonly: true,
            priority: 0,
        });

        let task = Task::new(config);

        // Wait for the initial scan to finish.
        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < 5 {
            if let Ok(msg) = task.rx.recv_timeout(std::time::Duration::from_millis(100)) {
                if let BackgroundMessage::Finished(_) | BackgroundMessage::FinishedWithoutWatcher =
                    msg
                {
                    break;
                }
            }
        }

        // Subscribe a reader to the bus BEFORE we trigger the
        // conversion so we don't miss the Discovered event the
        // worker publishes on success.
        let bus_reader = task.file_event_bus.subscribe();

        // Give the PDF bus subscriber a moment to spin up — it's
        // attached near the end of `run_indexing`, after `Finished`.
        std::thread::sleep(std::time::Duration::from_millis(200));

        // Drop a PDF and publish a Discovered event for it. The
        // bus subscriber will forward the path to `tx_pdf`, the
        // PDF worker will call `should_convert()` (true because
        // no `.md` exists yet), then call `execute()` (which
        // returns Ok), then publish Discovered for the output
        // `.md`.
        let pdf_path = dir.path().join("dropped.pdf");
        std::fs::write(&pdf_path, b"dummy").unwrap();
        task.file_event_bus
            .publish(crate::file_events::FileEvent::discovered(pdf_path.clone()));

        // The corresponding `.md` path (input path with extension swapped).
        let expected_md = {
            let mut p = pdf_path.clone();
            p.set_extension("md");
            p
        };

        // Drain the bus until we see a Discovered event for the
        // expected output path, or we time out.
        let start = std::time::Instant::now();
        let mut saw_discovered = false;
        while start.elapsed().as_secs() < 5 {
            match bus_reader.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(event) => {
                    if event.kind == crate::file_events::FileEventKind::Discovered
                        && event.path == expected_md
                    {
                        saw_discovered = true;
                        break;
                    }
                    // ignore other events (the initial scan also publishes)
                }
                Err(_) => continue,
            }
        }
        assert!(
            saw_discovered,
            "PDF worker should publish a Discovered event for the output .md after a successful conversion"
        );
    }

    #[test]
    fn test_bus_published_image_triggers_vision_via_subscriber() {
        // Mirror of `test_bus_published_pdf_triggers_conversion_via_subscriber`
        // for the image bus subscriber. A PNG published directly to
        // the bus (bypassing the initial scan and the notify
        // watcher) must be picked up by the image-vision worker via
        // the bus subscriber.
        //
        // The "Analyzing image" log entry fires BEFORE the API call
        // inside `process_image`, so the test only needs to wait for
        // that log to know the worker received the path. We
        // intentionally configure a dummy vision-model URL — the API
        // call will fail, but that's fine; the test only cares that
        // the bus subscriber delivered the path.
        use crate::background::LogCategory;
        use crate::config::LlmConfig;

        let mut config = AppConfig::default();
        let dir = tempdir().unwrap();

        // Dummy vision model pointing at an unroutable address.
        config.models.insert(
            "test-vision".to_string(),
            LlmConfig {
                model: "test-vision".to_string(),
                api_key: "dummy".to_string(),
                api_url: "http://127.0.0.1:1".to_string(),
                cost: None,
                use_case: vec!["vision".to_string()],
            },
        );

        // Image content library so the initial scan + subscriber
        // know to handle image files.
        config.content_libraries.push(ContentLibrary {
            name: "test".to_string(),
            kind: "image".to_string(),
            root_folder: dir.path().to_string_lossy().to_string(),
            readonly: true,
            priority: 0,
        });

        let task = Task::new(config);

        // Wait for Finished.
        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < 5 {
            if let Ok(msg) = task.rx.recv_timeout(std::time::Duration::from_millis(100)) {
                if let BackgroundMessage::Finished(_) | BackgroundMessage::FinishedWithoutWatcher =
                    msg
                {
                    break;
                }
            }
        }

        // Give the image bus subscriber a moment to start.
        std::thread::sleep(std::time::Duration::from_millis(200));

        // Drop an image and publish a Discovered event for it.
        let img_path = dir.path().join("dropped.png");
        std::fs::write(&img_path, b"dummy image data").unwrap();
        task.file_event_bus
            .publish(crate::file_events::FileEvent::discovered(img_path.clone()));

        // The bus subscriber should forward the path to the
        // image-vision worker, which sends an "Analyzing image"
        // log entry before invoking the model.
        let start = std::time::Instant::now();
        let mut all_messages: Vec<String> = Vec::new();
        let mut saw_analyzing = false;
        while start.elapsed().as_secs() < 10 {
            if let Ok(msg) = task.rx.recv_timeout(std::time::Duration::from_millis(100)) {
                if let BackgroundMessage::LogEntry(entry) = msg {
                    all_messages.push(format!("{:?}: {}", entry.category, entry.message));
                    if entry.category == LogCategory::ImageVision
                        && entry.message.contains("Analyzing image")
                    {
                        saw_analyzing = true;
                        break;
                    }
                }
            }
        }
        if !saw_analyzing {
            eprintln!("Test saw messages: {:?}", all_messages);
        }
        assert!(
            saw_analyzing,
            "Bus-published image Discovered event should reach the image-vision worker"
        );
    }
}
