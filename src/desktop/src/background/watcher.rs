//! Filesystem watcher — observes content-library directories and routes changes to PDF converter and vision processor queues.

use crate::background::PdfConversionJob;
use crate::background::models::{BackgroundLogEntry, LogCategory};
use crate::config::AppConfig;
use crate::file_events::{Bus, FileEvent};
use crate::messages::BackgroundMessage;
use notify::Watcher;
use std::path::PathBuf;
use std::sync::mpsc::Sender;

pub struct FileWatcher {
    config: AppConfig,
    tx: Sender<BackgroundMessage>,
    bus: Bus<FileEvent>,
    tx_pdf: Sender<PathBuf>,
    tx_img: Sender<PathBuf>,
}

impl FileWatcher {
    pub fn new(
        config: AppConfig,
        tx: Sender<BackgroundMessage>,
        bus: Bus<FileEvent>,
        tx_pdf: Sender<PathBuf>,
        tx_img: Sender<PathBuf>,
    ) -> Self {
        Self {
            config,
            tx,
            bus,
            tx_pdf,
            tx_img,
        }
    }

    fn process_event(
        path: &std::path::Path,
        event_kind: notify::EventKind,
        config: &AppConfig,
        tx_notify: &Sender<BackgroundMessage>,
        bus_watcher: &Bus<FileEvent>,
        tx_pdf_watcher: &Sender<PathBuf>,
        tx_img_watcher: &Sender<PathBuf>,
    ) {
        if path.components().any(|c| c.as_os_str() == ".git") {
            return;
        }

        let event_type = match event_kind {
            notify::EventKind::Create(_) => "created",
            notify::EventKind::Modify(_) => "modified",
            notify::EventKind::Remove(_) => "deleted",
            _ => "changed",
        };

        let mut is_image_lib = false;
        for lib in &config.content_libraries {
            let lib_path = PathBuf::from(&lib.root_folder);
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
            ) && is_image_lib
            {
                is_img = true;
            }
        }

        if is_md || is_pdf || is_img {
            let _ = tx_notify.send(BackgroundMessage::LogEntry(BackgroundLogEntry::new(
                LogCategory::Watcher,
                format!(
                    "File {} {:?}",
                    event_type,
                    path.file_name().unwrap_or_default()
                ),
            )));
        }

        if is_md {
            match event_kind {
                notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {
                    if path.is_file() {
                        let tags = crate::utils::tags::extract_tags_from_file(path);
                        let _ = tx_notify.send(BackgroundMessage::FileModified {
                            path: path.to_path_buf(),
                            tags,
                        });
                        bus_watcher.publish(FileEvent::updated_one(path.to_path_buf()));
                    }
                }
                notify::EventKind::Remove(_) => {
                    let _ = tx_notify.send(BackgroundMessage::FileDeleted {
                        path: path.to_path_buf(),
                    });
                    bus_watcher.publish(FileEvent::removed_one(path.to_path_buf()));
                }
                _ => {}
            }
        } else if is_pdf {
            match event_kind {
                notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {
                    bus_watcher.publish(FileEvent::updated_one(path.to_path_buf()));
                    let job = PdfConversionJob::new(path.to_path_buf());
                    if job.should_convert() {
                        let _ = tx_pdf_watcher.send(path.to_path_buf());
                    }
                }
                notify::EventKind::Remove(_) => {
                    let _ = tx_notify.send(BackgroundMessage::FileDeleted {
                        path: path.to_path_buf(),
                    });
                    bus_watcher.publish(FileEvent::removed_one(path.to_path_buf()));
                }
                _ => {}
            }
        } else if is_img {
            match event_kind {
                notify::EventKind::Create(_) | notify::EventKind::Modify(_) => {
                    let job = crate::background::models::ImageJob::new(path.to_path_buf());
                    if job.should_process() {
                        let _ = tx_img_watcher.send(path.to_path_buf());
                    }
                }
                notify::EventKind::Remove(_) => {
                    let _ = tx_notify.send(BackgroundMessage::FileDeleted {
                        path: path.to_path_buf(),
                    });
                    bus_watcher.publish(FileEvent::removed_one(path.to_path_buf()));
                }
                _ => {}
            }
        } else if !path.exists() {
            let _ = tx_notify.send(BackgroundMessage::FileDeleted {
                path: path.to_path_buf(),
            });
            bus_watcher.publish(FileEvent::removed_one(path.to_path_buf()));
        }
    }

    pub fn start(&mut self) {
        let tx_notify = self.tx.clone();
        let config_watcher = self.config.clone();
        let tx_pdf_watcher = self.tx_pdf.clone();
        let tx_img_watcher = self.tx_img.clone();
        let bus_watcher = self.bus.clone();

        let watcher_result =
            notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    for path in event.paths {
                        Self::process_event(
                            &path,
                            event.kind,
                            &config_watcher,
                            &tx_notify,
                            &bus_watcher,
                            &tx_pdf_watcher,
                            &tx_img_watcher,
                        );
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
            let _ = self.tx.send(BackgroundMessage::Finished(watcher));
        } else if let Err(e) = watcher_result {
            tracing::error!(
                name = "background_task.watcher_init_failed",
                error = %e,
                "Failed to initialize file system watcher. Changes will not be detected. Likely cause: OS limits on open files or permissions."
            );
            let _ = self.tx.send(BackgroundMessage::FinishedWithoutWatcher);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AppConfig, ContentLibrary};
    use notify::event::RemoveKind;
    use tempfile::tempdir;

    #[test]
    fn test_watcher_new_creates_struct() {
        let config = AppConfig::default();
        let (tx, _rx) = std::sync::mpsc::channel();
        let bus = Bus::new();
        let (tx_pdf, _rx_pdf) = std::sync::mpsc::channel();
        let (tx_img, _rx_img) = std::sync::mpsc::channel();

        let _watcher = FileWatcher::new(config, tx, bus, tx_pdf, tx_img);
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
        let bus = Bus::new();
        let (tx_pdf, _rx_pdf) = std::sync::mpsc::channel();
        let (tx_img, _rx_img) = std::sync::mpsc::channel();

        let mut watcher = FileWatcher::new(config, tx, bus, tx_pdf, tx_img);
        watcher.start();

        let msg = rx.recv_timeout(std::time::Duration::from_millis(1000));
        assert!(msg.is_ok());
        match msg.unwrap() {
            BackgroundMessage::Finished(_) | BackgroundMessage::FinishedWithoutWatcher => {}
            other => panic!(
                "Expected Finished or FinishedWithoutWatcher, got {:?}",
                other
            ),
        }
    }

    #[test]
    fn test_pdf_remove_event_publishes_deletion() {
        let config = AppConfig::default();
        let bus = Bus::new();
        let reader = bus.subscribe();
        let (tx, rx) = std::sync::mpsc::channel();
        let (tx_pdf, _rx_pdf) = std::sync::mpsc::channel();
        let (tx_img, _rx_img) = std::sync::mpsc::channel();

        let path = PathBuf::from("C:\\test\\doc.pdf");
        let event_kind = notify::EventKind::Remove(RemoveKind::Any);

        FileWatcher::process_event(&path, event_kind, &config, &tx, &bus, &tx_pdf, &tx_img);

        let _log_entry = rx.recv_timeout(std::time::Duration::from_millis(500));
        let msg = rx.recv_timeout(std::time::Duration::from_millis(500));
        assert!(msg.is_ok());
        match msg.unwrap() {
            BackgroundMessage::FileDeleted { path: p } => assert_eq!(p, path),
            other => panic!("Expected FileDeleted, got {:?}", other),
        }

        let bus_event = reader.recv_timeout(std::time::Duration::from_millis(500));
        assert!(bus_event.is_ok());
        let bus_event = bus_event.unwrap();
        assert_eq!(bus_event.kind, crate::file_events::FileEventKind::Removed);
        assert_eq!(bus_event.paths, vec![path]);
    }

    #[test]
    fn test_image_remove_event_publishes_deletion() {
        let dir = tempdir().unwrap();
        let config = AppConfig {
            content_libraries: vec![ContentLibrary {
                name: "images".to_string(),
                kind: "image".to_string(),
                root_folder: dir.path().to_string_lossy().to_string(),
                readonly: false,
                priority: 0,
            }],
            ..AppConfig::default()
        };
        let bus = Bus::new();
        let reader = bus.subscribe();
        let (tx, rx) = std::sync::mpsc::channel();
        let (tx_pdf, _rx_pdf) = std::sync::mpsc::channel();
        let (tx_img, _rx_img) = std::sync::mpsc::channel();

        let path = dir.path().join("photo.png");
        let event_kind = notify::EventKind::Remove(RemoveKind::Any);

        FileWatcher::process_event(&path, event_kind, &config, &tx, &bus, &tx_pdf, &tx_img);

        let _log_entry = rx.recv_timeout(std::time::Duration::from_millis(500));
        let msg = rx.recv_timeout(std::time::Duration::from_millis(500));
        assert!(msg.is_ok());
        match msg.unwrap() {
            BackgroundMessage::FileDeleted { path: p } => assert_eq!(p, path),
            other => panic!("Expected FileDeleted, got {:?}", other),
        }

        let bus_event = reader.recv_timeout(std::time::Duration::from_millis(500));
        assert!(bus_event.is_ok());
        let bus_event = bus_event.unwrap();
        assert_eq!(bus_event.kind, crate::file_events::FileEventKind::Removed);
        assert_eq!(bus_event.paths, vec![path]);
    }

    #[test]
    fn test_markdown_remove_event_publishes_deletion() {
        let config = AppConfig::default();
        let bus = Bus::new();
        let reader = bus.subscribe();
        let (tx, rx) = std::sync::mpsc::channel();
        let (tx_pdf, _rx_pdf) = std::sync::mpsc::channel();
        let (tx_img, _rx_img) = std::sync::mpsc::channel();

        let path = PathBuf::from("C:\\test\\doc.md");
        let event_kind = notify::EventKind::Remove(RemoveKind::Any);

        FileWatcher::process_event(&path, event_kind, &config, &tx, &bus, &tx_pdf, &tx_img);

        let _log_entry = rx.recv_timeout(std::time::Duration::from_millis(500));
        let msg = rx.recv_timeout(std::time::Duration::from_millis(500));
        assert!(msg.is_ok());
        match msg.unwrap() {
            BackgroundMessage::FileDeleted { path: p } => assert_eq!(p, path),
            other => panic!("Expected FileDeleted, got {:?}", other),
        }

        let bus_event = reader.recv_timeout(std::time::Duration::from_millis(500));
        assert!(bus_event.is_ok());
        let bus_event = bus_event.unwrap();
        assert_eq!(bus_event.kind, crate::file_events::FileEventKind::Removed);
        assert_eq!(bus_event.paths, vec![path]);
    }

    #[test]
    fn test_pdf_create_event_publishes_update() {
        let config = AppConfig::default();
        let bus = Bus::new();
        let reader = bus.subscribe();
        let (tx, _rx) = std::sync::mpsc::channel();
        let (tx_pdf, rx_pdf) = std::sync::mpsc::channel();
        let (tx_img, _rx_img) = std::sync::mpsc::channel();

        let dir = tempdir().unwrap();
        let path = dir.path().join("doc.pdf");
        std::fs::write(&path, "test content").unwrap();
        let event_kind = notify::EventKind::Create(notify::event::CreateKind::File);

        FileWatcher::process_event(&path, event_kind, &config, &tx, &bus, &tx_pdf, &tx_img);

        let bus_event = reader.recv_timeout(std::time::Duration::from_millis(500));
        assert!(bus_event.is_ok());
        let bus_event = bus_event.unwrap();
        assert_eq!(bus_event.kind, crate::file_events::FileEventKind::Updated);
        assert_eq!(bus_event.paths, vec![path.clone()]);

        let pdf_msg = rx_pdf.recv_timeout(std::time::Duration::from_millis(500));
        assert!(pdf_msg.is_ok());
        assert_eq!(pdf_msg.unwrap(), path);
    }
}
