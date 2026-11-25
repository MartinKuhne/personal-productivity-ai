//! Background consumer for `[FileChanged]` events that extracts YAML tags before the UI applies tag state ([REQ-421], [REQ-422]).
//!
//! Unit tests live in the sibling `tag_manager_tests.rs` sidecar.

use crate::bus::core::Bus;
use crate::bus::events::file::{FileChanged, FileChangedKind};
use crate::bus::events::typed::{BackgroundEventSender, FsEvent};
use crate::config::AppConfig;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, SyncSender, channel};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;

const TAG_WORKER_COUNT: usize = 4;

#[derive(Clone, Copy)]
enum TagChangeKind {
    Added,
    Changed,
    Resync,
}

struct TagWorkItem {
    path: PathBuf,
    kind: TagChangeKind,
    generation: u64,
}

#[derive(Default)]
struct TagIndex {
    tags: BTreeMap<PathBuf, Vec<String>>,
    generations: BTreeMap<PathBuf, u64>,
}

impl TagIndex {
    fn next_generation(&mut self, path: &Path) -> u64 {
        let generation = self.generations.entry(path.to_path_buf()).or_default();
        *generation = generation.wrapping_add(1);
        *generation
    }
}

type SharedTagIndex = Arc<Mutex<TagIndex>>;

#[derive(Default)]
struct WorkTracker {
    pending: Mutex<usize>,
    idle: Condvar,
}

impl WorkTracker {
    fn register(&self) {
        if let Ok(mut pending) = self.pending.lock() {
            *pending += 1;
        } else {
            tracing::error!(
                name = "tag_manager.work_tracker.poisoned",
                "Tag manager work tracker was poisoned while registering work."
            );
        }
    }

    fn complete(&self) {
        if let Ok(mut pending) = self.pending.lock() {
            *pending = pending.saturating_sub(1);
            if *pending == 0 {
                self.idle.notify_all();
            }
        } else {
            tracing::error!(
                name = "tag_manager.work_tracker.poisoned",
                "Tag manager work tracker was poisoned while completing work."
            );
        }
    }

    fn wait_until_idle(&self) -> bool {
        let Ok(mut pending) = self.pending.lock() else {
            tracing::error!(
                name = "tag_manager.work_tracker.poisoned",
                "Tag manager could not wait for parsing work because its tracker was poisoned."
            );
            return false;
        };
        while *pending > 0 {
            match self.idle.wait(pending) {
                Ok(guard) => pending = guard,
                Err(error) => {
                    tracing::error!(
                        name = "tag_manager.work_tracker.poisoned",
                        error = %error,
                        "Tag manager could not wait for parsing work because its tracker was poisoned."
                    );
                    return false;
                }
            }
        }
        true
    }
}

/// Handle used by initial indexing to wait until already-published file events
/// have been parsed before it emits the indexing-finished notification.
pub(crate) struct TagManagerHandle {
    flush_tx: tokio::sync::mpsc::UnboundedSender<SyncSender<bool>>,
}

impl TagManagerHandle {
    /// Drains currently queued file events and waits for their tag results.
    pub(crate) fn flush(&self) -> bool {
        let (reply_tx, reply_rx) = std::sync::mpsc::sync_channel(1);
        if self.flush_tx.send(reply_tx).is_err() {
            tracing::error!(
                name = "tag_manager.flush.request_failed",
                "Tag manager is unavailable; initial tag parsing could not be joined."
            );
            return false;
        }
        match reply_rx.recv() {
            Ok(true) => true,
            Ok(false) => {
                tracing::error!(
                    name = "tag_manager.flush.incomplete",
                    "Tag manager could not confirm completion of initial tag parsing."
                );
                false
            }
            Err(_) => {
                tracing::error!(
                    name = "tag_manager.flush.response_failed",
                    "Tag manager stopped before initial tag parsing completed."
                );
                false
            }
        }
    }
}

/// Background half of the tag manager: subscribes to file events and reads YAML tags off the UI thread.
pub(crate) struct TagManager {
    file_bus: Bus<FileChanged>,
    tx: BackgroundEventSender,
    config: AppConfig,
}

impl TagManager {
    /// Creates a tag manager that observes the shared file-change bus ([REQ-421]).
    pub(crate) fn new(
        file_bus: Bus<FileChanged>,
        tx: BackgroundEventSender,
        config: AppConfig,
    ) -> Self {
        Self {
            file_bus,
            tx,
            config,
        }
    }

    /// Starts the event consumer and its four tag parsing workers ([REQ-302], [REQ-422]).
    pub(crate) fn spawn(self) -> TagManagerHandle {
        // Subscribe before starting the thread so scan-time events cannot race
        // the asynchronous consumer's first poll.
        let mut reader = self.file_bus.subscribe_async();
        let tx = self.tx;
        let config = self.config;
        let work_tracker = Arc::new(WorkTracker::default());
        let worker_tracker = Arc::clone(&work_tracker);
        let tag_index = Arc::new(Mutex::new(TagIndex::default()));
        let worker_tag_index = Arc::clone(&tag_index);
        let (flush_tx, mut flush_rx) = tokio::sync::mpsc::unbounded_channel::<SyncSender<bool>>();

        std::thread::spawn(move || {
            let (work_tx, work_rx) = channel::<TagWorkItem>();
            let workers =
                Self::spawn_workers(work_rx, tx.clone(), worker_tracker, worker_tag_index);
            let work_tx_for_loop = work_tx.clone();
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    tracing::error!(
                        name = "tag_manager.runtime.create_failed",
                        error = %error,
                        "Failed to start the tag manager runtime. File tags will not be refreshed."
                    );
                    return;
                }
            };

            runtime.block_on(async move {
                let mut flush_open = true;
                let mut resync_pending = false;
                loop {
                    tokio::select! {
                        event = reader.recv() => {
                            match event {
                                Ok(event) => Self::process_file_event(
                                    event,
                                    &mut resync_pending,
                                    &config,
                                    &work_tx_for_loop,
                                    &work_tracker,
                                    &tag_index,
                                    &tx,
                                ),
                                Err(tokio::sync::broadcast::error::RecvError::Lagged(count)) => {
                                    tracing::warn!(
                                        name = "tag_manager.file_bus.lagged",
                                        lagged_events = count,
                                        "Tag manager fell behind the file event bus; it will rebuild tags from configured libraries on the next event."
                                    );
                                    reader = reader.resubscribe();
                                    resync_pending = true;
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                            }
                        }
                        command = flush_rx.recv(), if flush_open => {
                            match command {
                                Some(reply) => {
                                    let lagged = Self::drain_available(
                                        &mut reader,
                                        &work_tx_for_loop,
                                        &work_tracker,
                                        &tag_index,
                                    );
                                    let flushed = if lagged || resync_pending {
                                        reader = reader.resubscribe();
                                        Self::resynchronize(
                                            &config,
                                            &work_tx_for_loop,
                                            &work_tracker,
                                            &tag_index,
                                            &tx,
                                        )
                                    } else {
                                        work_tracker.wait_until_idle()
                                    };
                                    resync_pending = !flushed && (lagged || resync_pending);
                                    let _ = reply.send(flushed);
                                }
                                None => flush_open = false,
                            }
                        }
                    }
                }
            });

            drop(work_tx);
            for worker in workers {
                if let Err(error) = worker.join() {
                    tracing::error!(
                        name = "tag_manager.worker.panicked",
                        error = ?error,
                        "A tag parsing worker panicked."
                    );
                }
            }
        });

        TagManagerHandle { flush_tx }
    }

    fn drain_available(
        reader: &mut tokio::sync::broadcast::Receiver<FileChanged>,
        work_tx: &Sender<TagWorkItem>,
        work_tracker: &WorkTracker,
        tag_index: &SharedTagIndex,
    ) -> bool {
        let mut lagged = false;
        loop {
            match reader.try_recv() {
                Ok(event) => Self::dispatch(event, work_tx, work_tracker, tag_index),
                Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
                Err(tokio::sync::broadcast::error::TryRecvError::Closed) => break,
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(count)) => {
                    lagged = true;
                    tracing::warn!(
                        name = "tag_manager.file_bus.lagged",
                        lagged_events = count,
                        "Tag manager fell behind the file event bus while flushing; it will rebuild tags from configured libraries."
                    );
                }
            }
        }
        lagged
    }

    fn process_file_event(
        event: FileChanged,
        resync_pending: &mut bool,
        config: &AppConfig,
        work_tx: &Sender<TagWorkItem>,
        work_tracker: &WorkTracker,
        tag_index: &SharedTagIndex,
        tx: &BackgroundEventSender,
    ) {
        if *resync_pending {
            if Self::resynchronize(config, work_tx, work_tracker, tag_index, tx) {
                *resync_pending = false;
            }
        } else {
            Self::dispatch(event, work_tx, work_tracker, tag_index);
        }
    }

    fn dispatch(
        event: FileChanged,
        work_tx: &Sender<TagWorkItem>,
        work_tracker: &WorkTracker,
        tag_index: &SharedTagIndex,
    ) {
        let kind = match event.kind {
            FileChangedKind::Added => TagChangeKind::Added,
            FileChangedKind::Changed => TagChangeKind::Changed,
            FileChangedKind::Deleted => {
                if let Ok(mut index) = tag_index.lock() {
                    for entry in event.files {
                        index.next_generation(&entry.path);
                        index.tags.remove(&entry.path);
                    }
                } else {
                    tracing::error!(
                        name = "tag_manager.index.poisoned",
                        "Tag index was poisoned while removing deleted files."
                    );
                }
                return;
            }
        };

        for entry in event.files {
            let generation = match tag_index.lock() {
                Ok(mut index) => index.next_generation(&entry.path),
                Err(error) => {
                    tracing::error!(
                        name = "tag_manager.index.poisoned",
                        error = %error,
                        "Tag index was poisoned while dispatching file tags."
                    );
                    continue;
                }
            };
            work_tracker.register();
            if let Err(error) = work_tx.send(TagWorkItem {
                path: entry.path,
                kind,
                generation,
            }) {
                work_tracker.complete();
                tracing::warn!(
                    name = "tag_manager.work_queue.closed",
                    error = %error,
                    "Tag manager could not queue a file for tag extraction."
                );
                break;
            }
        }
    }

    fn resynchronize(
        config: &AppConfig,
        work_tx: &Sender<TagWorkItem>,
        work_tracker: &WorkTracker,
        tag_index: &SharedTagIndex,
        tx: &BackgroundEventSender,
    ) -> bool {
        if !work_tracker.wait_until_idle() {
            return false;
        }
        let paths = match current_tag_paths(config) {
            Ok(paths) => paths,
            Err(error) => {
                tracing::error!(
                    name = "tag_manager.resync.scan_failed",
                    error = %error,
                    "Tag manager could not enumerate the configured workspace for resynchronization."
                );
                return false;
            }
        };
        let Ok(mut index) = tag_index.lock() else {
            tracing::error!(
                name = "tag_manager.index.poisoned",
                "Tag index was poisoned while preparing a resynchronization."
            );
            return false;
        };
        index.tags.clear();
        let generations = paths
            .iter()
            .map(|path| (path.clone(), index.next_generation(path)))
            .collect::<BTreeMap<_, _>>();
        drop(index);

        for path in paths {
            let generation = generations[&path];
            work_tracker.register();
            if let Err(error) = work_tx.send(TagWorkItem {
                path,
                kind: TagChangeKind::Resync,
                generation,
            }) {
                work_tracker.complete();
                tracing::error!(
                    name = "tag_manager.resync.queue_failed",
                    error = %error,
                    "Tag manager could not queue a file during resynchronization."
                );
                return false;
            }
        }
        if !work_tracker.wait_until_idle() {
            return false;
        }
        let Ok(index) = tag_index.lock() else {
            tracing::error!(
                name = "tag_manager.index.poisoned",
                "Tag index was poisoned after resynchronization."
            );
            return false;
        };
        let files = index
            .tags
            .iter()
            .map(|(path, tags)| (path.clone(), tags.clone()))
            .collect();
        if let Err(error) = tx.send(FsEvent::TagsResynchronized { files }.into()) {
            tracing::error!(
                name = "tag_manager.resync.delivery_failed",
                error = %error,
                "Tag manager could not deliver its resynchronized tag index."
            );
            return false;
        }
        true
    }

    fn spawn_workers(
        work_rx: Receiver<TagWorkItem>,
        tx: BackgroundEventSender,
        work_tracker: Arc<WorkTracker>,
        tag_index: SharedTagIndex,
    ) -> Vec<JoinHandle<()>> {
        let work_rx = Arc::new(Mutex::new(work_rx));
        (0..TAG_WORKER_COUNT)
            .map(|_| {
                let work_rx = Arc::clone(&work_rx);
                let tx = tx.clone();
                let work_tracker = Arc::clone(&work_tracker);
                let tag_index = Arc::clone(&tag_index);
                std::thread::spawn(move || {
                    loop {
                        let work = {
                            let receiver = match work_rx.lock() {
                                Ok(receiver) => receiver,
                                Err(error) => {
                                    tracing::error!(
                                        name = "tag_manager.work_queue.poisoned",
                                        error = %error,
                                        "Tag manager work queue was poisoned."
                                    );
                                    break;
                                }
                            };
                            match receiver.recv() {
                                Ok(work) => work,
                                Err(_) => break,
                            }
                        };

                        let _completion = WorkCompletion(Arc::clone(&work_tracker));
                        let tags = crate::agent::utils::tags::extract_tags_from_file(&work.path);
                        let is_current = if let Ok(mut index) = tag_index.lock() {
                            let is_current = index
                                .generations
                                .get(&work.path)
                                .copied()
                                .unwrap_or_default()
                                == work.generation;
                            if is_current {
                                index.tags.insert(work.path.clone(), tags.clone());
                            }
                            is_current
                        } else {
                            tracing::error!(
                                name = "tag_manager.index.poisoned",
                                "Tag index was poisoned while applying parsed file tags."
                            );
                            false
                        };
                        if !is_current {
                            continue;
                        }
                        let event = match work.kind {
                            TagChangeKind::Added => Some(FsEvent::FileParsed {
                                path: work.path,
                                tags,
                            }),
                            TagChangeKind::Changed => Some(FsEvent::FileModified {
                                path: work.path,
                                tags,
                            }),
                            TagChangeKind::Resync => None,
                        };
                        let Some(event) = event else {
                            continue;
                        };
                        if let Err(error) = tx.send(event.into()) {
                            tracing::warn!(
                                name = "tag_manager.result.delivery_failed",
                                error = %error,
                                "Tag extraction result could not be delivered to the UI."
                            );
                        }
                    }
                })
            })
            .collect()
    }
}

fn current_tag_paths(config: &AppConfig) -> Result<Vec<PathBuf>, walkdir::Error> {
    let mut paths = std::collections::BTreeSet::new();
    for library in &config.content_libraries {
        if library.kind.eq_ignore_ascii_case("image") {
            continue;
        }
        let root = PathBuf::from(&library.root_folder);
        if !root.exists() {
            continue;
        }
        for entry in walkdir::WalkDir::new(&root)
            .into_iter()
            .filter_entry(|entry| entry.file_name() != ".git")
        {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            let Some(extension) = entry.path().extension().and_then(|ext| ext.to_str()) else {
                continue;
            };
            if ["md", "markdown", "txt", "pdf"]
                .iter()
                .any(|supported| extension.eq_ignore_ascii_case(supported))
            {
                paths.insert(entry.path().to_path_buf());
            }
        }
    }
    Ok(paths.into_iter().collect())
}

struct WorkCompletion(Arc<WorkTracker>);

impl Drop for WorkCompletion {
    fn drop(&mut self) {
        self.0.complete();
    }
}

#[cfg(test)]
#[path = "tag_manager_tests.rs"]
mod tests;
