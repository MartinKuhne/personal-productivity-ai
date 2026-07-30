//! Background task orchestrator — spawns and owns all worker threads (watcher, indexer, PDF converter, vision processor, bus router).

use crate::app::messages::BackgroundMessage;
use crate::app::watcher::events::{Bus, FileEvent};
use crate::app::watcher::file_watcher::FileWatcher;
use crate::background::bus_router::BusRouter;
use crate::background::indexer::Indexer;
use crate::background::pdf_converter::PdfConverterWorker;
use crate::background::vision_processor::ImageVisionWorker;
use crate::config::{AppConfig, CONFIG_ARRIVAL_TIMEOUT, ConfigArrived};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

pub struct Task {
    pub rx: Receiver<BackgroundMessage>,
    pub tx: Sender<BackgroundMessage>,
    pub file_event_bus: Bus<FileEvent>,
    pub _watcher: Option<notify::RecommendedWatcher>,
    /// Cancellation signal shared with the indexer. Set to `true` via
    /// [`Task::cancel`] to ask the initial library scan to stop early.
    cancel: Arc<AtomicBool>,
}

impl Task {
    /// Build a background task that waits for a [`ConfigArrived`] event
    /// on `config_bus` before spawning its worker threads. The
    /// subscription is registered before this returns, so callers may
    /// publish any time afterwards and the spawned thread will
    /// observe the first arrival (or fall back to
    /// [`AppConfig::default`] if no event arrives within
    /// [`CONFIG_ARRIVAL_TIMEOUT`]).
    pub fn new(config_bus: Bus<ConfigArrived>) -> Self {
        let (tx, rx) = channel();
        let tx_clone = tx.clone();
        let file_event_bus = Bus::new();
        let bus_clone = file_event_bus.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = cancel.clone();

        // Subscribe before spawning so the thread's reader is in place
        // by the time the caller publishes.
        let config_reader = config_bus.subscribe();

        std::thread::spawn(move || {
            let config = match config_reader.recv_timeout(CONFIG_ARRIVAL_TIMEOUT) {
                Ok(event) => {
                    tracing::info!(
                        name = "config.arrived",
                        "Background task received configuration, spawning workers"
                    );
                    event.config
                }
                Err(_) => {
                    tracing::error!(
                        name = "config.arrived.timeout",
                        timeout_ms = CONFIG_ARRIVAL_TIMEOUT.as_millis() as u64,
                        "No ConfigArrived event observed within timeout; using default configuration"
                    );
                    AppConfig::default()
                }
            };
            Self::run_indexing(config, tx_clone, bus_clone, cancel_clone);
        });

        Self {
            rx,
            tx,
            file_event_bus,
            _watcher: None,
            cancel,
        }
    }

    /// Build a background task whose workers start immediately, using
    /// the supplied configuration. This is a convenience for tests
    /// that do not need the bus-driven init path.
    #[doc(hidden)]
    pub fn new_for_test(config: AppConfig) -> Self {
        // Subscribe before publishing so the broadcast delivery
        // matches the event. `Task::new` registers the subscription
        // during construction; we publish after it returns.
        let bus = Bus::new();
        let task = Self::new(bus.clone());
        bus.publish(ConfigArrived::new(config));
        task
    }

    /// Signal the initial library scan to stop. The watcher and the
    /// post-scan workers are not affected (they have their own
    /// shutdown paths). Calling this after the scan has already
    /// completed is a no-op.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }

    fn run_indexing(
        config: crate::config::AppConfig,
        tx: Sender<BackgroundMessage>,
        file_event_bus: Bus<FileEvent>,
        cancel: Arc<AtomicBool>,
    ) {
        let (tx_work, rx_work) = channel::<PathBuf>();
        let rx_work = Arc::new(Mutex::new(rx_work));
        let (tx_pdf, rx_pdf) = channel::<PathBuf>();
        let (tx_img, rx_img) = channel::<PathBuf>();

        let cmd_template = config.pdf_converter_command.clone();
        PdfConverterWorker::new(rx_pdf, tx.clone(), file_event_bus.clone(), cmd_template).spawn();

        ImageVisionWorker::new(rx_img, tx.clone(), config.clone(), file_event_bus.clone()).spawn();

        let workers = Indexer::spawn_workers(4, rx_work, tx.clone());

        let indexer = Indexer::new(config.clone(), tx.clone(), file_event_bus.clone(), cancel);
        indexer.scan_libraries(&tx_work, &tx_pdf, &tx_img);

        drop(tx_work);
        for worker in workers {
            let _ = worker.join();
        }

        let mut file_watcher = FileWatcher::new(
            config.clone(),
            tx.clone(),
            file_event_bus.clone(),
            tx_pdf.clone(),
            tx_img.clone(),
        );
        file_watcher.start();

        BusRouter::new(file_event_bus.clone(), tx_pdf, tx_img).spawn();
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
        let task = Task::new_for_test(config);

        let mut got_finished = false;
        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < 5 {
            if let Ok(msg) = task.rx.recv_timeout(std::time::Duration::from_millis(100))
                && matches!(
                    msg,
                    BackgroundMessage::Finished(_) | BackgroundMessage::FinishedWithoutWatcher
                )
            {
                got_finished = true;
                break;
            }
        }
        assert!(got_finished, "Should complete initialization");
    }

    #[test]
    fn test_cancel_signals_indexer_without_panicking() {
        // Regression for the no-op `Task::cancel` stub: after the fix the
        // method must store into the shared `Arc<AtomicBool>` and not
        // panic on repeated or out-of-order calls.
        let config = AppConfig::default();
        let task = Task::new_for_test(config);
        task.cancel();
        task.cancel(); // second call must also be safe
        // Drive the background to completion so the test exits cleanly.
        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < 5 {
            if let Ok(msg) = task.rx.recv_timeout(std::time::Duration::from_millis(100))
                && matches!(
                    msg,
                    BackgroundMessage::Finished(_) | BackgroundMessage::FinishedWithoutWatcher
                )
            {
                break;
            }
        }
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

        let task = Task::new_for_test(config);

        let mut got_finished = false;
        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < 5 {
            if let Ok(msg) = task.rx.recv_timeout(std::time::Duration::from_millis(100))
                && matches!(
                    msg,
                    BackgroundMessage::Finished(_) | BackgroundMessage::FinishedWithoutWatcher
                )
            {
                got_finished = true;
                break;
            }
        }
        assert!(got_finished, "Should complete indexing");
    }

    #[test]
    fn test_initial_scan_publishes_discovered_events() {
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

        let task = Task::new_for_test(config);
        let reader = task.file_event_bus.subscribe();

        let mut got_finished = false;
        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < 5 {
            if let Ok(msg) = task.rx.recv_timeout(std::time::Duration::from_millis(100))
                && matches!(
                    msg,
                    BackgroundMessage::Finished(_) | BackgroundMessage::FinishedWithoutWatcher
                )
            {
                got_finished = true;
                break;
            }
        }
        assert!(got_finished, "Should complete initialization");

        let mut events = Vec::new();
        while let Ok(ev) = reader.recv_timeout(std::time::Duration::from_millis(100)) {
            events.push(ev);
        }

        let discovered: Vec<_> = events
            .iter()
            .filter(|e| e.kind == crate::app::watcher::events::FileEventKind::Discovered)
            .collect();
        let total: usize = discovered.iter().map(|e| e.paths.len()).sum();
        assert_eq!(total, 3);
        let mut names: Vec<String> = discovered
            .iter()
            .flat_map(|e| {
                e.paths
                    .iter()
                    .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            })
            .collect();
        names.sort();
        assert_eq!(names, vec!["a.md", "b.md", "c.txt"]);
    }

    #[test]
    fn test_bus_subscribers_see_discovered_events() {
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

        let task = Task::new_for_test(config);
        let tag_reader = task.file_event_bus.subscribe();
        let tree_reader = task.file_event_bus.subscribe();

        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < 5 {
            if let Ok(msg) = task.rx.recv_timeout(std::time::Duration::from_millis(100))
                && matches!(
                    msg,
                    BackgroundMessage::Finished(_) | BackgroundMessage::FinishedWithoutWatcher
                )
            {
                break;
            }
        }

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
        assert_eq!(tag_events[0].paths[0], tree_events[0].paths[0]);
        assert_eq!(
            tag_events[0].kind,
            crate::app::watcher::events::FileEventKind::Discovered
        );
    }

    #[test]
    fn test_initial_scan_publishes_pdf_discovered_to_bus() {
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

        let task = Task::new_for_test(config);
        let reader = task.file_event_bus.subscribe();

        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < 5 {
            if let Ok(msg) = task.rx.recv_timeout(std::time::Duration::from_millis(100))
                && matches!(
                    msg,
                    BackgroundMessage::Finished(_) | BackgroundMessage::FinishedWithoutWatcher
                )
            {
                break;
            }
        }

        let mut events = Vec::new();
        while let Ok(ev) = reader.recv_timeout(std::time::Duration::from_millis(100)) {
            events.push(ev);
        }

        let pdf_discovered = events
            .iter()
            .find(|e| {
                e.kind == crate::app::watcher::events::FileEventKind::Discovered
                    && e.paths[0].extension().and_then(|x| x.to_str()) == Some("pdf")
            })
            .expect("initial scan should publish Discovered for PDFs");
        assert_eq!(pdf_discovered.paths[0], dir.path().join("report.pdf"));
    }

    #[test]
    fn test_bus_published_pdf_triggers_conversion_via_subscriber() {
        use crate::background::LogCategory;

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

        let task = Task::new_for_test(config);

        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < 5 {
            if let Ok(msg) = task.rx.recv_timeout(std::time::Duration::from_millis(100))
                && matches!(
                    msg,
                    BackgroundMessage::Finished(_) | BackgroundMessage::FinishedWithoutWatcher
                )
            {
                break;
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(200));

        let pdf_path = dir.path().join("dropped.pdf");
        std::fs::write(&pdf_path, b"dummy").unwrap();
        task.file_event_bus
            .publish(crate::app::watcher::events::FileEvent::discovered_one(
                pdf_path.clone(),
            ));

        let mut saw_success = false;
        let start = std::time::Instant::now();
        let mut all_messages: Vec<String> = Vec::new();
        while start.elapsed().as_secs() < 5 {
            let Ok(msg) = task.rx.recv_timeout(std::time::Duration::from_millis(100)) else {
                continue;
            };
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

        let task = Task::new_for_test(config);

        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < 5 {
            if let Ok(msg) = task.rx.recv_timeout(std::time::Duration::from_millis(100))
                && matches!(
                    msg,
                    BackgroundMessage::Finished(_) | BackgroundMessage::FinishedWithoutWatcher
                )
            {
                break;
            }
        }

        let bus_reader = task.file_event_bus.subscribe();
        std::thread::sleep(std::time::Duration::from_millis(200));

        let pdf_path = dir.path().join("dropped.pdf");
        std::fs::write(&pdf_path, b"dummy").unwrap();
        task.file_event_bus
            .publish(crate::app::watcher::events::FileEvent::discovered_one(
                pdf_path.clone(),
            ));

        let expected_md = {
            let mut p = pdf_path.clone();
            p.set_extension("md");
            p
        };

        let start = std::time::Instant::now();
        let mut saw_discovered = false;
        while start.elapsed().as_secs() < 5 {
            match bus_reader.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(event) => {
                    if event.kind == crate::app::watcher::events::FileEventKind::Discovered
                        && event.paths.contains(&expected_md)
                    {
                        saw_discovered = true;
                        break;
                    }
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
        use crate::background::LogCategory;
        use crate::config::LlmConfig;

        let mut config = AppConfig::default();
        let dir = tempdir().unwrap();

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

        config.content_libraries.push(ContentLibrary {
            name: "test".to_string(),
            kind: "image".to_string(),
            root_folder: dir.path().to_string_lossy().to_string(),
            readonly: true,
            priority: 0,
        });

        let task = Task::new_for_test(config);

        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < 5 {
            if let Ok(msg) = task.rx.recv_timeout(std::time::Duration::from_millis(100))
                && matches!(
                    msg,
                    BackgroundMessage::Finished(_) | BackgroundMessage::FinishedWithoutWatcher
                )
            {
                break;
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(200));

        let img_path = dir.path().join("dropped.png");
        std::fs::write(&img_path, b"dummy image data").unwrap();
        task.file_event_bus
            .publish(crate::app::watcher::events::FileEvent::discovered_one(
                img_path.clone(),
            ));

        let start = std::time::Instant::now();
        let mut all_messages: Vec<String> = Vec::new();
        let mut saw_analyzing = false;
        while start.elapsed().as_secs() < 10 {
            if let Ok(msg) = task.rx.recv_timeout(std::time::Duration::from_millis(100))
                && let BackgroundMessage::LogEntry(entry) = msg
            {
                all_messages.push(format!("{:?}: {}", entry.category, entry.message));
                if entry.category == LogCategory::ImageVision
                    && entry.message.contains("Analyzing image")
                {
                    saw_analyzing = true;
                    break;
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

    /// Regression: `Task::new(bus)` must not start any background work
    /// until the first [`ConfigArrived`] event is observed. The
    /// observable signal is the file-event bus subscriber count:
    /// workers (which subscribe) have not been spawned yet, so the
    /// count is zero.
    ///
    /// Note: the task falls back to [`AppConfig::default`] after
    /// [`crate::config::CONFIG_ARRIVAL_TIMEOUT`] if no event
    /// arrives, so the absence of a `Finished` message is *not*
    /// a reliable assertion. The subscriber count is.
    #[test]
    fn test_task_does_not_spawn_workers_before_publish() {
        let bus = crate::config::config_bus();
        let task = Task::new(bus.clone());

        assert_eq!(
            task.file_event_bus.subscriber_count(),
            0,
            "no worker should be subscribed to the file-event bus before config arrives"
        );
    }

    /// Regression: after `publish(ConfigArrived { .. })` the workers
    /// spin up and the scan completes within a reasonable timeout.
    /// (`BusRouter` is spawned *after* the scan sends `Finished`,
    /// so we cannot rely on `subscriber_count() > 0` immediately
    /// after observing the message — the assertion is intentionally
    /// on the `Finished` delivery, which is the user-visible
    /// signal that the workers have done their first pass.)
    #[test]
    fn test_task_spawns_workers_after_publish() {
        let bus = crate::config::config_bus();
        let task = Task::new(bus.clone());

        // Publish before the worker thread has a chance to poll.
        bus.publish(ConfigArrived::new(AppConfig::default()));

        // Wait for the scan to finish so we know all workers are
        // alive and subscribed.
        let start = std::time::Instant::now();
        let mut finished = false;
        while start.elapsed() < std::time::Duration::from_secs(5) {
            if let Ok(msg) = task.rx.recv_timeout(std::time::Duration::from_millis(100))
                && matches!(
                    msg,
                    BackgroundMessage::Finished(_) | BackgroundMessage::FinishedWithoutWatcher
                )
            {
                finished = true;
                break;
            }
        }
        assert!(finished, "task should finish after config arrives");
    }
}
