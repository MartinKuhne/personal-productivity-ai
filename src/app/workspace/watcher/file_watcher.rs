//! Filesystem watcher — publishes content-library changes to file and directory event buses.

use crate::background::models::{BackgroundLogEntry, LogCategory};
use crate::bus::core::Bus;
use crate::bus::events::directory::{DirectoryChanged, DirectoryChangedKind};
use crate::bus::events::file::FileChanged;
use crate::bus::events::typed::FsEvent;
use crate::config::AppConfig;
use notify::Watcher;
use std::path::PathBuf;
#[cfg(feature = "image-library")]
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};

pub struct FileWatcher {
    config: AppConfig,
    tx: crate::bus::events::typed::BackgroundEventSender,
    /// File-event bus: `[FileChanged]` events are published here.
    file_bus: Bus<FileChanged>,
    /// Directory-event bus: `[DirectoryChanged]` events are published here.
    dir_bus: Bus<DirectoryChanged>,
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
        file_bus: Bus<FileChanged>,
        dir_bus: Bus<DirectoryChanged>,
        #[cfg(feature = "image-library")] tx_img: Sender<PathBuf>,
        finished_watcher: Arc<Mutex<Option<notify::RecommendedWatcher>>>,
    ) -> Self {
        Self {
            config,
            tx,
            file_bus,
            dir_bus,
            #[cfg(feature = "image-library")]
            tx_img,
            finished_watcher,
        }
    }

    pub fn start(&mut self) {
        let tx_notify = self.tx.clone();
        #[cfg(feature = "image-library")]
        let config_watcher = self.config.clone();
        #[cfg(feature = "image-library")]
        let tx_img_watcher = self.tx_img.clone();
        let file_bus_watcher = self.file_bus.clone();
        let dir_bus_watcher = self.dir_bus.clone();
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
                                    dir_bus_watcher.publish(DirectoryChanged {
                                        path: path.clone(),
                                        kind: DirectoryChangedKind::Added,
                                    });
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
                                    dir_bus_watcher.publish(DirectoryChanged {
                                        path: path.clone(),
                                        kind: DirectoryChangedKind::Deleted,
                                    });
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
                            dir_bus_watcher.publish(DirectoryChanged {
                                path: old.clone(),
                                kind: DirectoryChangedKind::Deleted,
                            });
                            dir_bus_watcher.publish(DirectoryChanged {
                                path: new.clone(),
                                kind: DirectoryChangedKind::Added,
                            });
                        } else if let Some(ext) = new.extension() {
                            let ext_str = ext.to_string_lossy().to_lowercase();
                            if matches!(ext_str.as_str(), "md" | "markdown" | "txt" | "pdf") {
                                file_bus_watcher.publish(FileChanged::discovered_one(new.clone()));
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
                                        // [REQ-332] A not-previously-seen file is
                                        // `[FileChanged][Added]`; modifying an
                                        // existing one is `[FileChanged][Changed]`.
                                        if is_added {
                                            file_bus_watcher
                                                .publish(FileChanged::discovered_one(path.clone()));
                                        } else {
                                            file_bus_watcher
                                                .publish(FileChanged::updated_one(path.clone()));
                                        }
                                    } else if !path.exists() {
                                        // Rename delivered as a separate event:
                                        // the old name is already gone —
                                        // `[FileChanged][Deleted]`.
                                        file_bus_watcher
                                            .publish(FileChanged::removed_one(path.clone()));
                                    }
                                }
                                notify::EventKind::Remove(_) => {
                                    file_bus_watcher
                                        .publish(FileChanged::removed_one(path.clone()));
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
                                        file_bus_watcher
                                            .publish(FileChanged::discovered_one(path.clone()));
                                    } else {
                                        file_bus_watcher
                                            .publish(FileChanged::updated_one(path.clone()));
                                    }
                                }
                                _ => {}
                            }
                        } else {
                            // Image-vision dispatch. Only compiled
                            // when the `image-library` Cargo feature
                            // is enabled; without it, image files
                            // still reach the file bus (above)
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
                                    file_bus_watcher
                                        .publish(FileChanged::removed_one(path.clone()));
                                }
                            }
                            #[cfg(not(feature = "image-library"))]
                            {
                                if !path.exists() {
                                    file_bus_watcher
                                        .publish(FileChanged::removed_one(path.clone()));
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
#[path = "file_watcher_tests.rs"]
mod tests;
