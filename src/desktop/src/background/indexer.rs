//! Initial recursive scanner — walks content-library directories emitting `FileEvent::Discovered` for each entry.

use crate::background::models::{BackgroundLogEntry, LogCategory};
use crate::background::PdfConversionJob;
use crate::config::AppConfig;
use crate::file_events::{Bus, FileEvent};
use crate::messages::BackgroundMessage;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

pub struct Indexer {
    config: AppConfig,
    tx: Sender<BackgroundMessage>,
    bus: Bus<FileEvent>,
    cancel: Arc<AtomicBool>,
}

impl Indexer {
    pub fn new(
        config: AppConfig,
        tx: Sender<BackgroundMessage>,
        bus: Bus<FileEvent>,
        cancel: Arc<AtomicBool>,
    ) -> Self {
        Self {
            config,
            tx,
            bus,
            cancel,
        }
    }

    pub fn spawn_workers(
        num: usize,
        rx_work: Arc<Mutex<Receiver<PathBuf>>>,
        tx_gui: Sender<BackgroundMessage>,
    ) -> Vec<std::thread::JoinHandle<()>> {
        let mut workers = Vec::new();
        for _ in 0..num {
            let rx = rx_work.clone();
            let tx_clone = tx_gui.clone();
            let handle = std::thread::spawn(move || loop {
                let path = {
                    let rx = match rx.lock() {
                        Ok(guard) => guard,
                        Err(_) => break,
                    };
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
                let _ = tx_clone.send(BackgroundMessage::FileParsed { path, tags });
                std::thread::yield_now();
            });
            workers.push(handle);
        }
        workers
    }

    /// Walk every content library and emit a single `FileEvent::Discovered`
    /// per directory containing **all** files found inside it (Markdown,
    /// PDF, image, etc.), batching them into the `paths` vec.  This keeps
    /// the event count low during the initial scan — downstream consumers
    /// iterate `event.paths` regardless, so the behaviour is identical.
    ///
    /// Each library is walked independently with its own batch; PDF and
    /// image files are also forwarded on their respective channels for
    /// background conversion / vision processing.
    pub fn scan_libraries(
        &self,
        tx_work: &Sender<PathBuf>,
        tx_pdf: &Sender<PathBuf>,
        tx_img: &Sender<PathBuf>,
    ) {
        let mut files_scanned = 0;
        let mut pdfs_queued = 0;
        let mut images_queued = 0;
        let mut last_log_time = std::time::Instant::now();

        for lib in &self.config.content_libraries {
            let is_image_lib = lib.kind == "image";
            let root_path = PathBuf::from(&lib.root_folder);
            let walker = walkdir::WalkDir::new(&root_path)
                .into_iter()
                .filter_entry(|e| e.file_name() != ".git");

            let mut batch_paths: Vec<PathBuf> = Vec::new();
            let mut current_parent: Option<PathBuf> = None;

            let flush_batch = |batch: &mut Vec<PathBuf>, bus: &Bus<FileEvent>| {
                if !batch.is_empty() {
                    bus.publish(FileEvent::discovered(std::mem::take(batch)));
                }
            };

            for entry in walker.filter_map(|e| e.ok()) {
                if self.cancel.load(Ordering::SeqCst) {
                    flush_batch(&mut batch_paths, &self.bus);
                    return;
                }
                files_scanned += 1;
                let path = entry.path();
                if path.is_file() {
                    let parent = path.parent().map(|p| p.to_path_buf());
                    if parent != current_parent {
                        flush_batch(&mut batch_paths, &self.bus);
                        current_parent = parent;
                    }
                    if let Some(ext) = path.extension() {
                        let ext_str = ext.to_string_lossy().to_lowercase();
                        if ext_str == "md" || ext_str == "markdown" || ext_str == "txt" {
                            batch_paths.push(path.to_path_buf());
                            let _ = tx_work.send(path.to_path_buf());
                        } else if ext_str == "pdf" {
                            batch_paths.push(path.to_path_buf());
                            let job = PdfConversionJob::new(path.to_path_buf());
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
                    let _ = self.tx.send(BackgroundMessage::DirParsed {
                        path: path.to_path_buf(),
                    });
                }

                if files_scanned % 500 == 0 || last_log_time.elapsed().as_secs() >= 5 {
                    let _ = self
                        .tx
                        .send(BackgroundMessage::LogEntry(BackgroundLogEntry::new(
                            LogCategory::Indexer,
                            format!(
                                "Scanned {} files, queued {} PDFs, queued {} images",
                                files_scanned, pdfs_queued, images_queued
                            ),
                        )));
                    last_log_time = std::time::Instant::now();
                }
                if files_scanned % 50 == 0 {
                    std::thread::yield_now();
                }
            }

            flush_batch(&mut batch_paths, &self.bus);
        }

        let _ = self
            .tx
            .send(BackgroundMessage::LogEntry(BackgroundLogEntry::new(
                LogCategory::Indexer,
                format!(
                "Initial indexing complete. Scanned {} files, queued {} PDFs, queued {} images.",
                files_scanned, pdfs_queued, images_queued
            ),
            )));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let (tx_img, _rx_img) = std::sync::mpsc::channel();

        indexer.scan_libraries(&tx_work, &tx_pdf, &tx_img);

        let mut discovered = Vec::new();
        while let Ok(ev) = reader.recv_timeout(std::time::Duration::from_millis(100)) {
            if ev.kind == crate::file_events::FileEventKind::Discovered {
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
        let (tx_img, _rx_img) = std::sync::mpsc::channel();

        indexer.scan_libraries(&tx_work, &tx_pdf, &tx_img);

        let mut discovered = Vec::new();
        while let Ok(ev) = reader.recv_timeout(std::time::Duration::from_millis(100)) {
            if ev.kind == crate::file_events::FileEventKind::Discovered {
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
        let (tx_img, _rx_img) = std::sync::mpsc::channel();

        indexer.scan_libraries(&tx_work, &tx_pdf, &tx_img);

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
}
