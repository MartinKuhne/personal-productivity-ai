//! Initial recursive scanner — walks content-library directories emitting `FileEvent::Discovered` for each entry.
//!
//! Unit tests live in the sibling `indexer_tests.rs` sidecar.

use crate::background::PdfConversionJob;
use crate::background::models::{BackgroundLogEntry, LogCategory};
use crate::bus::core::Bus;
use crate::bus::events::file::FileEvent;
use crate::bus::events::typed::{BackgroundEventSender, FsEvent};
use crate::config::AppConfig;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use tracing::instrument;

pub struct Indexer {
    config: AppConfig,
    tx: BackgroundEventSender,
    bus: Bus<FileEvent>,
    cancel: Arc<AtomicBool>,
}

impl Indexer {
    pub fn new(
        config: AppConfig,
        tx: BackgroundEventSender,
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
        tx_gui: BackgroundEventSender,
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
                    let tags = {
                        let _span = tracing::debug_span!(
                            "indexer.parse_file",
                            path = %path.display()
                        )
                        .entered();
                        crate::agent::utils::tags::extract_tags_from_file(&path)
                    };
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
    /// background conversion / vision processing. The `tx_img` parameter
    /// is only present when the `image-library` Cargo feature is enabled.
    #[instrument(skip_all, name = "indexer.scan_libraries")]
    pub fn scan_libraries(
        &self,
        tx_work: &Sender<PathBuf>,
        tx_pdf: &Sender<PathBuf>,
        #[cfg(feature = "image-library")] tx_img: &Sender<PathBuf>,
    ) {
        let mut files_scanned = 0;
        let mut pdfs_queued = 0;
        #[cfg(feature = "image-library")]
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
                            // Image-vision branch. Only compiled when
                            // the `image-library` Cargo feature is on;
                            // without it, image files still appear on
                            // the `FileEvent` bus (via `batch_paths`
                            // above) but never get processed for
                            // vision.
                            #[cfg(feature = "image-library")]
                            {
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
                    let _ = self.tx.send(
                        FsEvent::DirParsed {
                            path: path.to_path_buf(),
                        }
                        .into(),
                    );
                }

                if files_scanned % 500 == 0 || last_log_time.elapsed().as_secs() >= 5 {
                    #[cfg(feature = "image-library")]
                    let msg = format!(
                        "Scanned {} files, queued {} PDFs, queued {} images",
                        files_scanned, pdfs_queued, images_queued
                    );
                    #[cfg(not(feature = "image-library"))]
                    let msg = format!(
                        "Scanned {} files, queued {} PDFs",
                        files_scanned, pdfs_queued
                    );
                    let _ = self
                        .tx
                        .send(BackgroundLogEntry::new(LogCategory::Indexer, msg).into());
                    last_log_time = std::time::Instant::now();
                }
                if files_scanned % 50 == 0 {
                    std::thread::yield_now();
                }
            }

            flush_batch(&mut batch_paths, &self.bus);
        }

        #[cfg(feature = "image-library")]
        let final_msg = format!(
            "Initial indexing complete. Scanned {} files, queued {} PDFs, queued {} images.",
            files_scanned, pdfs_queued, images_queued
        );
        #[cfg(not(feature = "image-library"))]
        let final_msg = format!(
            "Initial indexing complete. Scanned {} files, queued {} PDFs.",
            files_scanned, pdfs_queued
        );
        let _ = self
            .tx
            .send(BackgroundLogEntry::new(LogCategory::Indexer, final_msg).into());
    }
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `indexer_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "indexer_tests.rs"]
mod tests;
