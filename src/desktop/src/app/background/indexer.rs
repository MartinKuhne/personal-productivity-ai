//! Initial recursive scanner — walks content-library directories emitting `FileEvent::Discovered` for each entry.
//!
//! Unit tests live in the sibling `indexer_tests.rs` sidecar.

use crate::app::background::PdfConversionJob;
use crate::app::background::models::{BackgroundLogEntry, LogCategory};
use crate::bus::core::Bus;
use crate::bus::events::file::FileEvent;
use crate::bus::events::typed::{BackgroundEvent, FsEvent};
use crate::config::AppConfig;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

pub struct Indexer {
    config: AppConfig,
    tx: Sender<BackgroundEvent>,
    bus: Bus<FileEvent>,
    cancel: Arc<AtomicBool>,
}

impl Indexer {
    pub fn new(
        config: AppConfig,
        tx: Sender<BackgroundEvent>,
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
        tx_gui: Sender<BackgroundEvent>,
    ) -> Vec<std::thread::JoinHandle<()>> {
        let mut workers = Vec::new();
        for _ in 0..num {
            let rx = rx_work.clone();
            let tx_clone = tx_gui.clone();
            let handle = std::thread::spawn(move || {
                loop {
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
                    let _ = tx_clone.send(FsEvent::FileParsed { path, tags }.into());
                    std::thread::yield_now();
                }
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
                        ) && is_image_lib
                        {
                            let job =
                                crate::app::background::models::ImageJob::new(path.to_path_buf());
                            if job.should_process() {
                                images_queued += 1;
                                let _ = tx_img.send(path.to_path_buf());
                            }
                        }
                    }
                } else if path.is_dir() {
                    let _ = self.tx.send(
                        FsEvent::DirParsed {
                            path: path.to_path_buf(),
                        }
                        .into(),
                    );
                }

                if files_scanned % 500 == 0 || last_log_time.elapsed().as_secs() >= 5 {
                    let _ = self.tx.send(
                        BackgroundLogEntry::new(
                            LogCategory::Indexer,
                            format!(
                                "Scanned {} files, queued {} PDFs, queued {} images",
                                files_scanned, pdfs_queued, images_queued
                            ),
                        )
                        .into(),
                    );
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
            .send(BackgroundLogEntry::new(
                LogCategory::Indexer,
                format!(
                "Initial indexing complete. Scanned {} files, queued {} PDFs, queued {} images.",
                files_scanned, pdfs_queued, images_queued
            ),
            ).into());
    }
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `indexer_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "indexer_tests.rs"]
mod tests;
