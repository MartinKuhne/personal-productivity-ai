//! Filesystem watcher — observes content-library directories and routes changes to PDF converter and vision processor queues.

use crate::app::background::PdfConversionJob;
use crate::app::background::models::{BackgroundLogEntry, LogCategory};
use crate::bus::core::Bus;
use crate::bus::events::file::FileEvent;
use crate::bus::events::typed::FsEvent;
use crate::config::AppConfig;
use notify::Watcher;
use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

pub struct FileWatcher {
    config: AppConfig,
    tx: crate::bus::events::typed::BackgroundEventSender,
    bus: Bus<FileEvent>,
    tx_pdf: Sender<PathBuf>,
    /// Image-vision sender. Only present when the `image-library`
    /// Cargo feature is enabled; the `is_img` detection below
    /// compiles out without the feature.
    #[cfg(feature = "image-library")]
    tx_img: Sender<PathBuf>,
    /// Slot for the `notify::RecommendedWatcher` handle. The
    /// watcher is moved into this slot by [`FileWatcher::start`]
    /// before [`FsEvent::Finished`] is sent on the typed channel,
    /// so the UI can take ownership via
    /// [`crate::app::background_task::Task::take_finished_watcher`].
    finished_watcher: Arc<Mutex<Option<notify::RecommendedWatcher>>>,
}

impl FileWatcher {
    /// The `tx_img` parameter is only present when the
    /// `image-library` Cargo feature is enabled.
    pub fn new(
        config: AppConfig,
        tx: crate::bus::events::typed::BackgroundEventSender,
        bus: Bus<FileEvent>,
        tx_pdf: Sender<PathBuf>,
        #[cfg(feature = "image-library")] tx_img: Sender<PathBuf>,
        finished_watcher: Arc<Mutex<Option<notify::RecommendedWatcher>>>,
    ) -> Self {
        Self {
            config,
            tx,
            bus,
            tx_pdf,
            #[cfg(feature = "image-library")]
            tx_img,
            finished_watcher,
        }
    }

    pub fn start(&mut self) {
        let tx_notify = self.tx.clone();
        #[cfg(feature = "image-library")]
        let config_watcher = self.config.clone();
        let tx_pdf_watcher = self.tx_pdf.clone();
        #[cfg(feature = "image-library")]
        let tx_img_watcher = self.tx_img.clone();
        let bus_watcher = self.bus.clone();
        let watcher_slot = self.finished_watcher.clone();

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

                        #[cfg(feature = "image-library")]
                        let is_image_lib = {
                            let mut flag = false;
                            for lib in &config_watcher.content_libraries {
                                let lib_path = PathBuf::from(&lib.root_folder);
                                if lib.kind == "image" && path.starts_with(&lib_path) {
                                    flag = true;
                                    break;
                                }
                            }
                            flag
                        };

                        let mut is_md = false;
                        let mut is_pdf = false;
                        #[cfg(feature = "image-library")]
                        let mut is_img = false;
                        if let Some(ext) = path.extension() {
                            let ext_str = ext.to_string_lossy().to_lowercase();
                            if ext_str == "md" || ext_str == "markdown" || ext_str == "txt" {
                                is_md = true;
                            } else if ext_str == "pdf" {
                                is_pdf = true;
                            } else {
                                #[cfg(feature = "image-library")]
                                if matches!(
                                    ext_str.as_str(),
                                    "jpg"
                                        | "jpeg"
                                        | "png"
                                        | "gif"
                                        | "webp"
                                        | "bmp"
                                        | "tiff"
                                        | "avif"
                                ) && is_image_lib
                                {
                                    is_img = true;
                                }
                            }
                        }

                        #[cfg(feature = "image-library")]
                        if is_md || is_pdf || is_img {
                            let _ = tx_notify.send(
                                BackgroundLogEntry::new(
                                    LogCategory::Watcher,
                                    format!(
                                        "File {} {:?}",
                                        event_type,
                                        path.file_name().unwrap_or_default()
                                    ),
                                )
                                .into(),
                            );
                        }
                        #[cfg(not(feature = "image-library"))]
                        if is_md || is_pdf {
                            let _ = tx_notify.send(
                                BackgroundLogEntry::new(
                                    LogCategory::Watcher,
                                    format!(
                                        "File {} {:?}",
                                        event_type,
                                        path.file_name().unwrap_or_default()
                                    ),
                                )
                                .into(),
                            );
                        }

                        if is_md {
                            match event.kind {
                                notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {
                                    if path.is_file() {
                                        let tags =
                                            crate::utils::tags::extract_tags_from_file(&path);
                                        let _ = tx_notify.send(
                                            FsEvent::FileModified {
                                                path: path.clone(),
                                                tags,
                                            }
                                            .into(),
                                        );
                                        bus_watcher.publish(FileEvent::updated_one(path.clone()));
                                    }
                                }
                                notify::EventKind::Remove(_) => {
                                    let _ = tx_notify
                                        .send(FsEvent::FileDeleted { path: path.clone() }.into());
                                    bus_watcher.publish(FileEvent::removed_one(path.clone()));
                                }
                                _ => {}
                            }
                        } else if is_pdf {
                            match event.kind {
                                notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {
                                    bus_watcher.publish(FileEvent::updated_one(path.clone()));
                                    let job = PdfConversionJob::new(path.clone());
                                    if job.should_convert() {
                                        let _ = tx_pdf_watcher.send(path.clone());
                                    }
                                }
                                _ => {}
                            }
                        } else {
                            // Image-vision dispatch. Only compiled
                            // when the `image-library` Cargo feature
                            // is enabled; without it, image files
                            // still reach the FileEvent bus (above)
                            // but never wake the (absent) vision
                            // worker.
                            #[cfg(feature = "image-library")]
                            {
                                if is_img {
                                    match event.kind {
                                        notify::EventKind::Create(_)
                                        | notify::EventKind::Modify(_) => {
                                            let job = crate::app::background::models::ImageJob::new(
                                                path.clone(),
                                            );
                                            if job.should_process() {
                                                let _ = tx_img_watcher.send(path.clone());
                                            }
                                        }
                                        _ => {}
                                    }
                                } else if !path.exists() {
                                    let _ = tx_notify
                                        .send(FsEvent::FileDeleted { path: path.clone() }.into());
                                    bus_watcher.publish(FileEvent::removed_one(path.clone()));
                                }
                            }
                            #[cfg(not(feature = "image-library"))]
                            {
                                if !path.exists() {
                                    let _ = tx_notify
                                        .send(FsEvent::FileDeleted { path: path.clone() }.into());
                                    bus_watcher.publish(FileEvent::removed_one(path.clone()));
                                }
                            }
                        }
                    }
                }
            });

        if let Ok(mut watcher) = watcher_result {
            for lib in &self.config.content_libraries {
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
            // Hand the live watcher over to the UI thread before
            // announcing that we are finished. The slot is the only
            // safe way to transfer a non-`Clone` handle across
            // threads; the typed channel only carries the
            // notification, not the handle.
            if let Ok(mut slot) = watcher_slot.lock() {
                *slot = Some(watcher);
            }
            let _ = self.tx.send(FsEvent::Finished.into());
        } else {
            tracing::error!(
                name = "background_task.watcher_init_failed",
                error = ?watcher_result.err(),
                "Failed to initialize file system watcher. Changes will not be detected. Likely cause: OS limits on open files or permissions."
            );
            let _ = self.tx.send(FsEvent::FinishedWithoutWatcher.into());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::events::typed::BackgroundEvent;
    use crate::bus::events::typed::BackgroundEventSender;
    use crate::config::{AppConfig, ContentLibrary};
    use tempfile::tempdir;

    #[test]
    fn test_watcher_new_creates_struct() {
        let config = AppConfig::default();
        let (tx, _rx) = std::sync::mpsc::channel();
        let tx = BackgroundEventSender::new(tx);
        let bus = Bus::new();
        let (tx_pdf, _rx_pdf) = std::sync::mpsc::channel();
        #[cfg(feature = "image-library")]
        let (tx_img, _rx_img) = std::sync::mpsc::channel();
        let slot = Arc::new(Mutex::new(None));

        #[cfg(feature = "image-library")]
        let _watcher = FileWatcher::new(config, tx, bus, tx_pdf, tx_img, slot);
        #[cfg(not(feature = "image-library"))]
        let _watcher = FileWatcher::new(config, tx, bus, tx_pdf, slot);
    }

    #[test]
    fn test_watcher_start_sends_finished() {
        let mut config = AppConfig::default();
        let dir = tempdir().unwrap();
        config.content_libraries.push(ContentLibrary {
            name: "test".to_string(),
            kind: "text".to_string(),
            root_folder: dir.path().to_string_lossy().to_string(),
            readonly: true,
            priority: 0,
        });

        let (tx, rx) = std::sync::mpsc::channel();
        let tx = BackgroundEventSender::new(tx);
        let bus = Bus::new();
        let (tx_pdf, _rx_pdf) = std::sync::mpsc::channel();
        #[cfg(feature = "image-library")]
        let (tx_img, _rx_img) = std::sync::mpsc::channel();
        let slot = Arc::new(Mutex::new(None));

        #[cfg(feature = "image-library")]
        let mut watcher = FileWatcher::new(config, tx, bus, tx_pdf, tx_img, slot.clone());
        #[cfg(not(feature = "image-library"))]
        let mut watcher = FileWatcher::new(config, tx, bus, tx_pdf, slot.clone());
        watcher.start();

        let event = rx.recv_timeout(std::time::Duration::from_millis(1000));
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
}
