//! Background task orchestrator — spawns and owns all worker threads (watcher, indexer, PDF converter, vision processor, bus router).

use crate::background::bus_router::BusRouter;
use crate::background::indexer::Indexer;
use crate::background::pdf_converter::PdfConverterWorker;
use crate::background::vision_processor::ImageVisionWorker;
use crate::background::watcher::FileWatcher;
use crate::file_events::{Bus, FileEvent};
use crate::messages::BackgroundMessage;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

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
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_clone = cancel.clone();

        std::thread::spawn(move || {
            Self::run_indexing(config_clone, tx_clone, bus_clone, cancel_clone);
        });

        Self {
            rx,
            tx,
            file_event_bus,
            _watcher: None,
        }
    }

    pub fn cancel(&self) {
        // This is a no-op placeholder. The actual cancel logic would need
        // to store the AtomicBool in Task. For now, we keep the field private.
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
        let reader = task.file_event_bus.subscribe();

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

        let mut events = Vec::new();
        while let Ok(ev) = reader.recv_timeout(std::time::Duration::from_millis(100)) {
            events.push(ev);
        }

        let discovered: Vec<_> = events
            .iter()
            .filter(|e| e.kind == crate::file_events::FileEventKind::Discovered)
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

        let task = Task::new(config);
        let tag_reader = task.file_event_bus.subscribe();
        let tree_reader = task.file_event_bus.subscribe();

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
            crate::file_events::FileEventKind::Discovered
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

        let task = Task::new(config);
        let reader = task.file_event_bus.subscribe();

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

        let mut events = Vec::new();
        while let Ok(ev) = reader.recv_timeout(std::time::Duration::from_millis(100)) {
            events.push(ev);
        }

        let pdf_discovered = events
            .iter()
            .find(|e| {
                e.kind == crate::file_events::FileEventKind::Discovered
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

        let task = Task::new(config);

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

        std::thread::sleep(std::time::Duration::from_millis(200));

        let pdf_path = dir.path().join("dropped.pdf");
        std::fs::write(&pdf_path, b"dummy").unwrap();
        task.file_event_bus
            .publish(crate::file_events::FileEvent::discovered_one(
                pdf_path.clone(),
            ));

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

        let bus_reader = task.file_event_bus.subscribe();
        std::thread::sleep(std::time::Duration::from_millis(200));

        let pdf_path = dir.path().join("dropped.pdf");
        std::fs::write(&pdf_path, b"dummy").unwrap();
        task.file_event_bus
            .publish(crate::file_events::FileEvent::discovered_one(
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
                    if event.kind == crate::file_events::FileEventKind::Discovered
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

        let task = Task::new(config);

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

        std::thread::sleep(std::time::Duration::from_millis(200));

        let img_path = dir.path().join("dropped.png");
        std::fs::write(&img_path, b"dummy image data").unwrap();
        task.file_event_bus
            .publish(crate::file_events::FileEvent::discovered_one(
                img_path.clone(),
            ));

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
