//! Filesystem watcher — observes content-library directories and routes changes to PDF converter and vision processor queues.

use crate::background::PdfConversionJob;
use crate::background::models::{BackgroundLogEntry, LogCategory};
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
                    // [REQ-402]/[REQ-322] Directory lifecycle events come
                    // straight from the notify kinds (no filesystem probing
                    // needed after a delete). Published as kind-homogeneous
                    // `[DirectoryChanged][Added]` / `[DirectoryChanged][Deleted]`
                    // events ([REQ-320], [REQ-332]).
                    match event.kind {
                        notify::EventKind::Create(kind) => {
                            let is_folder = matches!(kind, notify::event::CreateKind::Folder)
                                || (kind == notify::event::CreateKind::Any
                                    && event.paths.iter().any(|p| p.is_dir()));
                            if is_folder {
                                for path in &event.paths {
                                    if path.components().any(|c| c.as_os_str() == ".git") {
                                        continue;
                                    }
                                    let _ = tx_notify.send(
                                        BackgroundLogEntry::new(
                                            LogCategory::Watcher,
                                            format!(
                                                "Directory created {:?}",
                                                path.file_name().unwrap_or_default()
                                            ),
                                        )
                                        .into(),
                                    );
                                    bus_watcher
                                        .publish(FileEvent::dir_discovered_one(path.clone()));
                                }
                                // [REQ-407] The library roots are watched with
                                // `RecursiveMode::Recursive` (see `start`), so
                                // a newly created directory is already covered;
                                // no extra registration is needed.
                                return;
                            }
                        }
                        notify::EventKind::Remove(remove_kind) => {
                            // `[Folder]` is unambiguous. The Windows backend
                            // often reports `[Any]` instead: workspace-tracked
                            // files always carry extensions (md/txt/pdf/
                            // image), so a gone extension-less path is treated
                            // as a removed directory ([REQ-402]/[REQ-322]).
                            let removed_dirs: Vec<PathBuf> = match remove_kind {
                                notify::event::RemoveKind::Folder => event.paths.clone(),
                                notify::event::RemoveKind::Any => event
                                    .paths
                                    .iter()
                                    .filter(|p| p.extension().is_none())
                                    .cloned()
                                    .collect(),
                                _ => Vec::new(),
                            };
                            if !removed_dirs.is_empty() {
                                for path in &removed_dirs {
                                    if path.components().any(|c| c.as_os_str() == ".git") {
                                        continue;
                                    }
                                    let _ = tx_notify.send(
                                        BackgroundLogEntry::new(
                                            LogCategory::Watcher,
                                            format!(
                                                "Directory deleted {:?}",
                                                path.file_name().unwrap_or_default()
                                            ),
                                        )
                                        .into(),
                                    );
                                    bus_watcher.publish(FileEvent::dir_removed_one(path.clone()));
                                }
                                return;
                            }
                        }
                        _ => {}
                    }

                    // [REQ-404] A rename is treated as delete(old) followed by
                    // create(new). Backends that deliver one notification with
                    // both names are handled here; backends that split it into
                    // separate Remove/Create events are handled naturally by
                    // the per-path logic below ([REQ-332]: kind-homogeneous
                    // events).
                    if let notify::EventKind::Modify(notify::event::ModifyKind::Name(_)) =
                        event.kind
                        && event.paths.len() >= 2
                    {
                        let old = event.paths[0].clone();
                        let new = event.paths[1].clone();
                        if new.is_dir() {
                            bus_watcher.publish(FileEvent::dir_removed_one(old.clone()));
                            bus_watcher.publish(FileEvent::dir_discovered_one(new.clone()));
                        } else if let Some(ext) = new.extension() {
                            let ext_str = ext.to_string_lossy().to_lowercase();
                            if ext_str == "md" || ext_str == "markdown" || ext_str == "txt" {
                                let tags = crate::agent::utils::tags::extract_tags_from_file(&new);
                                let _ = tx_notify.send(
                                    FsEvent::FileModified {
                                        path: new.clone(),
                                        tags,
                                    }
                                    .into(),
                                );
                                bus_watcher.publish(FileEvent::discovered_one(new.clone()));
                            } else if ext_str == "pdf" {
                                bus_watcher.publish(FileEvent::discovered_one(new.clone()));
                                let job = PdfConversionJob::new(new.clone());
                                if job.should_convert() {
                                    let _ = tx_pdf_watcher.send(new.clone());
                                }
                            }
                        }
                        return;
                    }

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
                                    // [REQ-332] A not-previously-seen file is
                                    // `[FileChanged][Added]`; modifying an
                                    // existing one is `[FileChanged][Changed]`.
                                    // The create-half of a rename `[Name::To]`
                                    // (or `[Name::Both]` on backends that
                                    // deliver both names in one notification)
                                    // is also an addition ([REQ-404]).
                                    let is_added = matches!(
                                        event.kind,
                                        notify::EventKind::Create(_)
                                            | notify::EventKind::Modify(
                                                notify::event::ModifyKind::Name(
                                                    notify::event::RenameMode::To
                                                ),
                                            )
                                            | notify::EventKind::Modify(
                                                notify::event::ModifyKind::Name(
                                                    notify::event::RenameMode::Both
                                                ),
                                            )
                                    );
                                    if path.is_file() {
                                        let tags =
                                            crate::agent::utils::tags::extract_tags_from_file(
                                                &path,
                                            );
                                        let _ = tx_notify.send(
                                            FsEvent::FileModified {
                                                path: path.clone(),
                                                tags,
                                            }
                                            .into(),
                                        );
                                        // [REQ-332] A not-previously-seen file is
                                        // `[FileChanged][Added]`; modifying an
                                        // existing one is `[FileChanged][Changed]`.
                                        if is_added {
                                            bus_watcher
                                                .publish(FileEvent::discovered_one(path.clone()));
                                        } else {
                                            bus_watcher
                                                .publish(FileEvent::updated_one(path.clone()));
                                        }
                                    } else if !path.exists() {
                                        // Rename delivered as a separate event:
                                        // the old name is already gone —
                                        // `[FileChanged][Deleted]`.
                                        let _ = tx_notify.send(
                                            FsEvent::FileDeleted { path: path.clone() }.into(),
                                        );
                                        bus_watcher.publish(FileEvent::removed_one(path.clone()));
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
                                    let is_added = matches!(
                                        event.kind,
                                        notify::EventKind::Create(_)
                                            | notify::EventKind::Modify(
                                                notify::event::ModifyKind::Name(
                                                    notify::event::RenameMode::To
                                                ),
                                            )
                                            | notify::EventKind::Modify(
                                                notify::event::ModifyKind::Name(
                                                    notify::event::RenameMode::Both
                                                ),
                                            )
                                    );
                                    if is_added {
                                        bus_watcher
                                            .publish(FileEvent::discovered_one(path.clone()));
                                    } else {
                                        bus_watcher.publish(FileEvent::updated_one(path.clone()));
                                    }
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
                                            let job = crate::background::models::ImageJob::new(
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
    use crate::bus::events::FileEventKind;
    use crate::bus::events::typed::BackgroundEvent;
    use crate::bus::events::typed::BackgroundEventSender;
    use crate::config::{AppConfig, ContentLibrary};
    use tempfile::tempdir;

    #[test]
    fn test_watcher_new_creates_struct() {
        let config = AppConfig::default();
        let (tx, _rx) = tokio::sync::broadcast::channel(16);
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

    #[tokio::test]
    async fn test_watcher_start_sends_finished() {
        let mut config = AppConfig::default();
        let dir = tempdir().unwrap();
        config.content_libraries.push(ContentLibrary {
            name: "test".to_string(),
            kind: "text".to_string(),
            root_folder: dir.path().to_string_lossy().to_string(),
            readonly: true,
            priority: 0,
        });

        let (tx, mut rx) = tokio::sync::broadcast::channel(16);
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

        let event = rx.recv().await;
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

    fn spawn_watcher_on(dir: &std::path::Path) -> (FileWatcher, Bus<FileEvent>) {
        let mut config = AppConfig::default();
        config.content_libraries.push(ContentLibrary {
            name: "test".to_string(),
            kind: "text".to_string(),
            root_folder: dir.to_string_lossy().to_string(),
            readonly: true,
            priority: 0,
        });
        let (tx, _rx) = tokio::sync::broadcast::channel(16);
        let tx = BackgroundEventSender::new(tx);
        let bus = Bus::new();
        let (tx_pdf, _rx_pdf) = std::sync::mpsc::channel();
        #[cfg(feature = "image-library")]
        let (tx_img, _rx_img) = std::sync::mpsc::channel();
        let slot = Arc::new(Mutex::new(None));

        #[cfg(feature = "image-library")]
        let watcher = FileWatcher::new(config, tx, bus.clone(), tx_pdf, tx_img, slot);
        #[cfg(not(feature = "image-library"))]
        let watcher = FileWatcher::new(config, tx, bus.clone(), tx_pdf, slot);
        (watcher, bus)
    }

    fn wait_for_bus_event(
        reader: &crate::bus::core::BusReader<FileEvent>,
        pred: impl Fn(&FileEvent) -> bool,
        timeout: std::time::Duration,
    ) -> FileEvent {
        let start = std::time::Instant::now();
        while start.elapsed() < timeout {
            match reader.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(ev) if pred(&ev) => return ev,
                Ok(_) | Err(_) => continue,
            }
        }
        panic!("no matching FileEvent observed within timeout");
    }

    #[test]
    fn test_watcher_routes_md_create_modify_remove() {
        let dir = tempdir().unwrap();
        let (mut watcher, bus) = spawn_watcher_on(dir.path());
        let reader = bus.subscribe();
        watcher.start();

        let file = dir.path().join("notes.md");
        std::fs::write(&file, "hello").unwrap();

        // Create -> `[FileChanged][Added]` on the bus (REQ-332).
        wait_for_bus_event(
            &reader,
            |e| e.kind == FileEventKind::Discovered && e.paths.contains(&file),
            std::time::Duration::from_secs(5),
        );

        // Modify the file -> another FileModified.
        std::thread::sleep(std::time::Duration::from_millis(50));
        std::fs::write(&file, "hello world").unwrap();
        wait_for_bus_event(
            &reader,
            |e| e.kind == FileEventKind::Updated && e.paths.contains(&file),
            std::time::Duration::from_secs(5),
        );

        // Remove -> FileEvent::Removed.
        std::fs::remove_file(&file).unwrap();
        wait_for_bus_event(
            &reader,
            |e| e.kind == FileEventKind::Removed && e.paths.contains(&file),
            std::time::Duration::from_secs(5),
        );
    }

    #[test]
    fn test_watcher_publishes_directory_events() {
        // [REQ-402]/[REQ-322] Directory create/remove emit kind-homogeneous
        // DirectoryChanged events with the fully qualified path ([REQ-321]).
        let dir = tempdir().unwrap();
        let (mut watcher, bus) = spawn_watcher_on(dir.path());
        let reader = bus.subscribe();
        watcher.start();

        let subdir = dir.path().join("newdocs");
        std::fs::create_dir_all(&subdir).unwrap();
        wait_for_bus_event(
            &reader,
            |e| e.kind == FileEventKind::DirDiscovered && e.paths.contains(&subdir),
            std::time::Duration::from_secs(5),
        );

        std::fs::remove_dir(&subdir).unwrap();
        wait_for_bus_event(
            &reader,
            |e| e.kind == FileEventKind::DirRemoved && e.paths.contains(&subdir),
            std::time::Duration::from_secs(5),
        );
    }

    #[test]
    fn test_watcher_rename_is_delete_then_create() {
        // [REQ-404] A rename must surface as a Removed(old) followed by an
        // Added(new), regardless of how the backend delivers the notification.
        let dir = tempdir().unwrap();
        let (mut watcher, bus) = spawn_watcher_on(dir.path());
        let reader = bus.subscribe();
        watcher.start();

        let old = dir.path().join("before.md");
        let new = dir.path().join("after.md");
        std::fs::write(&old, "hello").unwrap();
        wait_for_bus_event(
            &reader,
            |e| e.kind == FileEventKind::Discovered && e.paths.contains(&old),
            std::time::Duration::from_secs(5),
        );

        std::fs::rename(&old, &new).unwrap();
        wait_for_bus_event(
            &reader,
            |e| e.kind == FileEventKind::Removed && e.paths.contains(&old),
            std::time::Duration::from_secs(5),
        );
        wait_for_bus_event(
            &reader,
            |e| e.kind == FileEventKind::Discovered && e.paths.contains(&new),
            std::time::Duration::from_secs(5),
        );
    }

    #[test]
    fn test_watcher_ignores_git_directory_and_non_workspace_files() {
        let dir = tempdir().unwrap();
        let (mut watcher, bus) = spawn_watcher_on(dir.path());
        let reader = bus.subscribe();
        watcher.start();

        let git_dir = dir.path().join(".git");
        std::fs::create_dir_all(&git_dir).unwrap();
        std::fs::write(git_dir.join("config"), "x").unwrap();

        let bin = dir.path().join("program.exe");
        std::fs::write(&bin, "not text").unwrap();

        // The watcher must not surface .git or non-workspace extensions on
        // the file-event bus.
        std::thread::sleep(std::time::Duration::from_millis(700));
        let mut got = 0;
        while reader.try_recv().is_ok() {
            got += 1;
        }
        assert_eq!(got, 0, "unexpected FileEvents published");
    }

    #[test]
    fn test_watcher_routes_pdf_to_converter_queue() {
        let dir = tempdir().unwrap();
        let mut config = AppConfig::default();
        config.content_libraries.push(ContentLibrary {
            name: "test".to_string(),
            kind: "text".to_string(),
            root_folder: dir.path().to_string_lossy().to_string(),
            readonly: true,
            priority: 0,
        });
        let (tx, _rx) = tokio::sync::broadcast::channel(16);
        let tx = BackgroundEventSender::new(tx);
        let bus = Bus::new();
        let (tx_pdf, rx_pdf) = std::sync::mpsc::channel();
        #[cfg(feature = "image-library")]
        let (tx_img, _rx_img) = std::sync::mpsc::channel();
        let slot = Arc::new(Mutex::new(None));

        #[cfg(feature = "image-library")]
        let mut watcher = FileWatcher::new(config, tx, bus, tx_pdf, tx_img, slot);
        #[cfg(not(feature = "image-library"))]
        let mut watcher = FileWatcher::new(config, tx, bus, tx_pdf, slot);
        watcher.start();

        // Minimal valid PDF so `should_convert` does not depend on metadata.
        let pdf = dir.path().join("doc.pdf");
        std::fs::write(&pdf, b"%PDF-1.4\n%%EOF\n").unwrap();

        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(5) {
            match rx_pdf.recv_timeout(std::time::Duration::from_millis(100)) {
                Ok(p) if p == pdf => return,
                Ok(_) | Err(_) => continue,
            }
        }
        panic!("PDF path never reached the converter queue");
    }
}
