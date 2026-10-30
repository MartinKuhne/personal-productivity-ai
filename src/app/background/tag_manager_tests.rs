//! Unit tests for the tag manager's FileChanged event handling.

use super::*;
use crate::bus::events::typed::{BackgroundEvent, FsEvent};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tempfile::tempdir;

fn receive_tag_events(
    receiver: &mut tokio::sync::broadcast::Receiver<BackgroundEvent>,
    expected: usize,
) -> Vec<FsEvent> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("test runtime");
    runtime.block_on(async {
        let mut events = Vec::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        while events.len() < expected {
            let timeout = deadline.saturating_duration_since(tokio::time::Instant::now());
            assert!(!timeout.is_zero(), "tag event deadline elapsed");
            match tokio::time::timeout(timeout, receiver.recv()).await {
                Ok(Ok(BackgroundEvent::Fs(event @ FsEvent::FileParsed { .. })))
                | Ok(Ok(BackgroundEvent::Fs(event @ FsEvent::FileModified { .. }))) => {
                    events.push(event);
                }
                Ok(Ok(_)) | Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => {}
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                    panic!("background event channel closed before tag results arrived")
                }
                Err(_) => panic!("tag event deadline elapsed"),
            }
        }
        events
    })
}

#[test]
fn file_bus_added_and_changed_events_are_parsed_off_thread() {
    let directory = tempdir().unwrap();
    let added_path = directory.path().join("added.md");
    let changed_path = directory.path().join("changed.markdown");
    std::fs::write(&added_path, "---\ntags: [added]\n---\nbody").unwrap();
    std::fs::write(&changed_path, "---\ntags: [changed]\n---\nbody").unwrap();

    let file_bus = Bus::new();
    let (event_tx, _) = tokio::sync::broadcast::channel(16);
    let sender = BackgroundEventSender::new(event_tx);
    let mut results = sender.subscribe();
    let _manager = TagManager::new(file_bus.clone(), sender, AppConfig::default()).spawn();

    let deadline = Instant::now() + Duration::from_secs(2);
    while file_bus.subscriber_count() == 0 && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(file_bus.subscriber_count(), 1, "tag manager must subscribe");

    file_bus.publish(FileChanged::discovered_one(added_path.clone()));
    file_bus.publish(FileChanged::updated_one(changed_path.clone()));

    let events = receive_tag_events(&mut results, 2);
    let mut parsed = HashSet::new();
    let mut modified = HashSet::new();
    for event in events {
        match event {
            FsEvent::FileParsed { path, tags } => {
                assert_eq!(tags, vec!["added"]);
                parsed.insert(path);
            }
            FsEvent::FileModified { path, tags } => {
                assert_eq!(tags, vec!["changed"]);
                modified.insert(path);
            }
            _ => unreachable!("only tag parse results are collected"),
        }
    }
    assert_eq!(parsed, HashSet::from([added_path]));
    assert_eq!(modified, HashSet::from([changed_path]));
}

#[test]
fn tag_manager_ignores_deletions_and_reads_pdf_file_events() {
    let directory = tempdir().unwrap();
    let pdf_path = directory.path().join("report.pdf");
    std::fs::write(&pdf_path, b"%PDF-1.7\nnot a YAML header").unwrap();

    let file_bus = Bus::new();
    let (event_tx, _) = tokio::sync::broadcast::channel(16);
    let sender = BackgroundEventSender::new(event_tx);
    let mut results = sender.subscribe();
    let _manager = TagManager::new(file_bus.clone(), sender, AppConfig::default()).spawn();

    let deadline = Instant::now() + Duration::from_secs(2);
    while file_bus.subscriber_count() == 0 && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(file_bus.subscriber_count(), 1, "tag manager must subscribe");

    file_bus.publish(FileChanged::removed_one(directory.path().join("gone.md")));
    file_bus.publish(FileChanged::discovered_one(pdf_path.clone()));

    let events = receive_tag_events(&mut results, 1);
    assert!(matches!(
        events.as_slice(),
        [FsEvent::FileParsed { path, tags }] if path == &pdf_path && tags.is_empty()
    ));
}

#[test]
fn worker_pool_starts_four_workers_and_shuts_down_when_queue_closes() {
    let (work_tx, work_rx) = channel();
    let (event_tx, _) = tokio::sync::broadcast::channel(16);
    let workers = TagManager::spawn_workers(
        work_rx,
        BackgroundEventSender::new(event_tx),
        Arc::new(WorkTracker::default()),
        Arc::new(Mutex::new(TagIndex::default())),
    );

    assert_eq!(workers.len(), TAG_WORKER_COUNT);
    drop(work_tx);
    for worker in workers {
        worker.join().expect("tag worker should shut down cleanly");
    }
}

#[test]
fn worker_pool_processes_every_queued_file_and_tolerates_missing_paths() {
    let directory = tempdir().unwrap();
    let mut paths = Vec::new();
    for index in 0..16 {
        let path = directory.path().join(format!("note_{index}.md"));
        std::fs::write(&path, format!("---\ntags: [tag_{index}]\n---\nbody")).unwrap();
        paths.push(path);
    }
    let missing_path = directory.path().join("missing.md");

    let (work_tx, work_rx) = channel();
    let (event_tx, _) = tokio::sync::broadcast::channel(64);
    let sender = BackgroundEventSender::new(event_tx);
    let mut results = sender.subscribe();
    let workers = TagManager::spawn_workers(
        work_rx,
        sender,
        Arc::new(WorkTracker::default()),
        Arc::new(Mutex::new(TagIndex::default())),
    );
    for path in &paths {
        work_tx
            .send(TagWorkItem {
                path: path.clone(),
                kind: TagChangeKind::Added,
                generation: 0,
            })
            .unwrap();
    }
    work_tx
        .send(TagWorkItem {
            path: missing_path.clone(),
            kind: TagChangeKind::Added,
            generation: 0,
        })
        .unwrap();
    drop(work_tx);

    let events = receive_tag_events(&mut results, paths.len() + 1);
    let mut parsed = HashSet::new();
    let mut saw_missing_path = false;
    for event in events {
        match event {
            FsEvent::FileParsed { path, tags } if path == missing_path => {
                assert!(tags.is_empty());
                saw_missing_path = true;
            }
            FsEvent::FileParsed { path, tags } => {
                assert!(paths.contains(&path));
                assert_eq!(tags.len(), 1);
                parsed.insert(path);
            }
            _ => unreachable!("all queued work represents Added files"),
        }
    }
    assert_eq!(parsed, paths.into_iter().collect());
    assert!(saw_missing_path);

    for worker in workers {
        worker.join().expect("tag worker should shut down cleanly");
    }
}

#[test]
fn worker_pool_drops_superseded_results_for_the_same_path() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("latest.md");
    std::fs::write(&path, "---\ntags: [latest]\n---\nbody").unwrap();

    let (work_tx, work_rx) = channel();
    let (event_tx, _) = tokio::sync::broadcast::channel(16);
    let sender = BackgroundEventSender::new(event_tx);
    let mut results = sender.subscribe();
    let tracker = Arc::new(WorkTracker::default());
    let tag_index = Arc::new(Mutex::new(TagIndex {
        generations: BTreeMap::from([(path.clone(), 2)]),
        ..TagIndex::default()
    }));
    let workers = TagManager::spawn_workers(work_rx, sender, tracker, Arc::clone(&tag_index));
    work_tx
        .send(TagWorkItem {
            path: path.clone(),
            kind: TagChangeKind::Added,
            generation: 1,
        })
        .unwrap();
    work_tx
        .send(TagWorkItem {
            path: path.clone(),
            kind: TagChangeKind::Changed,
            generation: 2,
        })
        .unwrap();
    drop(work_tx);

    let events = receive_tag_events(&mut results, 1);
    assert!(matches!(
        events.as_slice(),
        [FsEvent::FileModified { path: result_path, tags }]
            if result_path == &path && tags == &["latest"]
    ));
    assert!(
        results.try_recv().is_err(),
        "superseded add result is omitted"
    );
    assert_eq!(
        tag_index.lock().unwrap().tags.get(&path),
        Some(&vec!["latest".into()])
    );

    for worker in workers {
        worker.join().expect("tag worker should shut down cleanly");
    }
}

#[test]
fn flush_waits_until_published_file_tags_are_delivered() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("flush.md");
    std::fs::write(&path, "---\ntags: [ready]\n---\nbody").unwrap();

    let file_bus = Bus::new();
    let (event_tx, _) = tokio::sync::broadcast::channel(16);
    let sender = BackgroundEventSender::new(event_tx);
    let mut results = sender.subscribe();
    let manager = TagManager::new(file_bus.clone(), sender, AppConfig::default()).spawn();

    let deadline = Instant::now() + Duration::from_secs(2);
    while file_bus.subscriber_count() == 0 && Instant::now() < deadline {
        std::thread::yield_now();
    }
    assert_eq!(file_bus.subscriber_count(), 1, "tag manager must subscribe");

    file_bus.publish(FileChanged::discovered_one(path.clone()));
    assert!(
        manager.flush(),
        "flush must complete while tag manager is live"
    );

    match results
        .try_recv()
        .expect("flush waits for tag result delivery")
    {
        BackgroundEvent::Fs(FsEvent::FileParsed {
            path: parsed_path,
            tags,
        }) => {
            assert_eq!(parsed_path, path);
            assert_eq!(tags, vec!["ready"]);
        }
        event => panic!("unexpected event after tag flush: {event:?}"),
    }
}

#[test]
fn flush_reports_when_the_tag_manager_request_channel_is_closed() {
    let (flush_tx, flush_rx) = tokio::sync::mpsc::unbounded_channel();
    drop(flush_rx);
    let manager = TagManagerHandle { flush_tx };

    assert!(
        !manager.flush(),
        "failed completion must be observable to the indexing caller"
    );
}

#[test]
fn resynchronization_replaces_worker_tag_index_from_current_workspace_files() {
    let directory = tempdir().unwrap();
    let markdown = directory.path().join("current.md");
    let pdf = directory.path().join("report.pdf");
    let image = directory.path().join("cover.png");
    std::fs::write(&markdown, "---\ntags: [current]\n---\nbody").unwrap();
    std::fs::write(&pdf, b"%PDF-1.7").unwrap();
    std::fs::write(&image, b"image").unwrap();

    let config = config_with_root(directory.path());

    let (work_tx, work_rx) = channel();
    let (event_tx, _) = tokio::sync::broadcast::channel(16);
    let sender = BackgroundEventSender::new(event_tx);
    let mut results = sender.subscribe();
    let tracker = Arc::new(WorkTracker::default());
    let tag_index = Arc::new(Mutex::new(TagIndex {
        tags: BTreeMap::from([(PathBuf::from("stale.md"), vec!["stale".into()])]),
        ..TagIndex::default()
    }));
    let workers = TagManager::spawn_workers(
        work_rx,
        sender.clone(),
        Arc::clone(&tracker),
        Arc::clone(&tag_index),
    );

    assert!(TagManager::resynchronize(
        &config, &work_tx, &tracker, &tag_index, &sender,
    ));

    let event = results.try_recv().expect("resync snapshot should be sent");
    let files = match event {
        BackgroundEvent::Fs(FsEvent::TagsResynchronized { files }) => files,
        other => panic!("expected tag resync snapshot, got {other:?}"),
    };
    assert_eq!(
        files,
        vec![(markdown, vec!["current".to_string()]), (pdf, vec![])]
    );
    assert!(!files.iter().any(|(path, _)| path == &image));

    drop(work_tx);
    for worker in workers {
        worker.join().expect("tag worker should shut down cleanly");
    }
}

#[test]
fn lagged_file_reader_resynchronizes_from_the_next_event() {
    let directory = tempdir().unwrap();
    let current = directory.path().join("current.md");
    std::fs::write(&current, "---\ntags: [current]\n---\nbody").unwrap();
    let config = config_with_root(directory.path());

    let (file_tx, mut file_rx) = tokio::sync::broadcast::channel(1);
    file_tx
        .send(FileChanged::discovered_one(
            directory.path().join("dropped.md"),
        ))
        .unwrap();
    file_tx
        .send(FileChanged::updated_one(
            directory.path().join("trigger.md"),
        ))
        .unwrap();
    assert!(matches!(
        file_rx.try_recv(),
        Err(tokio::sync::broadcast::error::TryRecvError::Lagged(1))
    ));
    let trigger = file_rx
        .try_recv()
        .expect("the newest event is the recovery boundary");

    let (work_tx, work_rx) = channel();
    let (event_tx, _) = tokio::sync::broadcast::channel(16);
    let sender = BackgroundEventSender::new(event_tx);
    let mut results = sender.subscribe();
    let tracker = Arc::new(WorkTracker::default());
    let index = Arc::new(Mutex::new(TagIndex::default()));
    let workers = TagManager::spawn_workers(
        work_rx,
        sender.clone(),
        Arc::clone(&tracker),
        Arc::clone(&index),
    );
    let mut resync_pending = true;

    TagManager::process_file_event(
        trigger,
        &mut resync_pending,
        &config,
        &work_tx,
        &tracker,
        &index,
        &sender,
    );

    assert!(!resync_pending, "successful snapshot clears the lag marker");
    let event = results.try_recv().expect("resync snapshot should be sent");
    assert!(matches!(
        event,
        BackgroundEvent::Fs(FsEvent::TagsResynchronized { files })
            if files == vec![(current, vec!["current".to_string()])]
    ));

    drop(work_tx);
    for worker in workers {
        worker.join().expect("tag worker should shut down cleanly");
    }
}

// ---------------------------------------------------------------------------
// Helpers shared by the white-box tests below
// ---------------------------------------------------------------------------

/// A background-event sender with no subscribers, so every delivery fails.
///
/// Tests that need to observe events subscribe to the returned sender first;
/// tests that exercise delivery failures simply do not subscribe.
fn event_sender() -> BackgroundEventSender {
    let (event_tx, _) = tokio::sync::broadcast::channel::<BackgroundEvent>(16);
    BackgroundEventSender::new(event_tx)
}

/// Builds a content library rooted at `root`.
fn library(name: &str, kind: &str, root: &Path) -> crate::config::ContentLibrary {
    crate::config::ContentLibrary {
        name: name.into(),
        kind: kind.into(),
        root_folder: root.to_string_lossy().into_owned(),
        readonly: true,
        priority: 0,
    }
}

/// Builds a configuration whose only library is rooted at `root`.
fn config_with_root(root: &Path) -> AppConfig {
    let mut config = AppConfig::default();
    config.content_libraries.push(library("test", "text", root));
    config
}

/// Poisons a mutex by panicking while its guard is held.
///
/// The panic hook is silenced for the duration of the induced panic so the
/// intentional panic does not look like a test failure in the log output.
fn poison_mutex<T>(mutex: &Mutex<T>) {
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = mutex
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        panic!("intentional panic used to poison a mutex");
    }));
    std::panic::set_hook(previous_hook);

    assert!(outcome.is_err(), "the induced panic must unwind");
    assert!(mutex.is_poisoned(), "the mutex must be poisoned");
}

// ---------------------------------------------------------------------------
// TagIndex generations
// ---------------------------------------------------------------------------

#[test]
fn next_generation_counts_per_path_and_wraps_instead_of_overflowing() {
    let mut index = TagIndex::default();
    let first = Path::new("first.md");
    let second = Path::new("second.md");

    assert_eq!(index.next_generation(first), 1, "a new path starts at 1");
    assert_eq!(index.next_generation(first), 2);
    assert_eq!(
        index.next_generation(second),
        1,
        "generations are counted per path"
    );
    assert_eq!(index.generations.get(first), Some(&2));
    assert_eq!(index.generations.get(second), Some(&1));

    let mut wrapping = TagIndex {
        generations: BTreeMap::from([(PathBuf::from("saturated.md"), u64::MAX)]),
        ..TagIndex::default()
    };
    assert_eq!(
        wrapping.next_generation(Path::new("saturated.md")),
        0,
        "a saturated generation wraps to zero instead of panicking"
    );
}

// ---------------------------------------------------------------------------
// WorkTracker
// ---------------------------------------------------------------------------

#[test]
fn work_tracker_becomes_idle_once_every_registered_job_completes() {
    let tracker = Arc::new(WorkTracker::default());
    assert!(
        tracker.wait_until_idle(),
        "a tracker without registered work is idle"
    );

    tracker.register();
    tracker.register();
    assert_eq!(
        *tracker.pending.lock().unwrap(),
        2,
        "each registration counts one job"
    );

    // The completer waits briefly so the assertion below really blocks on the
    // condition variable instead of observing an already-idle counter.
    let completer = {
        let tracker = Arc::clone(&tracker);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            tracker.complete();
            tracker.complete();
        })
    };

    assert!(
        tracker.wait_until_idle(),
        "waiting must succeed once every job completes"
    );
    completer.join().expect("completer thread");
    assert_eq!(*tracker.pending.lock().unwrap(), 0);

    // Completion is saturating: an extra completion never underflows.
    tracker.complete();
    assert_eq!(*tracker.pending.lock().unwrap(), 0);
    assert!(tracker.wait_until_idle());
}

#[test]
fn work_tracker_reports_failure_once_poisoned() {
    let tracker = WorkTracker::default();
    poison_mutex(&tracker.pending);

    // Registering and completing must log instead of panicking on a poisoned lock.
    tracker.register();
    tracker.complete();
    assert!(
        !tracker.wait_until_idle(),
        "a poisoned tracker cannot confirm that work finished"
    );
}

// ---------------------------------------------------------------------------
// TagManagerHandle::flush reply handling
// ---------------------------------------------------------------------------

#[test]
fn flush_reports_incomplete_when_the_worker_answers_false() {
    let (flush_tx, mut flush_rx) = tokio::sync::mpsc::unbounded_channel::<SyncSender<bool>>();
    let responder = std::thread::spawn(move || {
        let reply = flush_rx.blocking_recv().expect("flush request");
        let _ = reply.send(false);
    });

    let handle = TagManagerHandle {
        flush_tx: flush_tx.clone(),
    };
    assert!(
        !handle.flush(),
        "an incomplete flush must be visible to the indexing caller"
    );

    drop(flush_tx);
    responder.join().expect("flush responder thread");
}

#[test]
fn flush_reports_failure_when_the_worker_drops_the_reply_channel() {
    let (flush_tx, mut flush_rx) = tokio::sync::mpsc::unbounded_channel::<SyncSender<bool>>();
    let responder = std::thread::spawn(move || {
        let reply = flush_rx.blocking_recv().expect("flush request");
        drop(reply);
    });

    let handle = TagManagerHandle {
        flush_tx: flush_tx.clone(),
    };
    assert!(
        !handle.flush(),
        "a tag manager that stops mid-flush must not report success"
    );

    drop(flush_tx);
    responder.join().expect("flush responder thread");
}

// ---------------------------------------------------------------------------
// TagManager::dispatch
// ---------------------------------------------------------------------------

#[test]
fn dispatch_queues_added_and_changed_files_with_fresh_generations() {
    let (work_tx, work_rx) = channel::<TagWorkItem>();
    let tracker = WorkTracker::default();
    let index = Arc::new(Mutex::new(TagIndex::default()));
    let added = PathBuf::from("added.md");
    let changed = PathBuf::from("changed.md");
    let also_changed = PathBuf::from("also_changed.md");

    TagManager::dispatch(
        FileChanged::discovered(vec![added.clone()]),
        &work_tx,
        &tracker,
        &index,
    );
    TagManager::dispatch(
        FileChanged::updated(vec![changed.clone(), also_changed.clone()]),
        &work_tx,
        &tracker,
        &index,
    );

    let first = work_rx.try_recv().expect("the added file is queued");
    assert_eq!(first.path, added);
    assert!(matches!(first.kind, TagChangeKind::Added));
    assert_eq!(first.generation, 1);

    let second = work_rx
        .try_recv()
        .expect("the first changed file is queued");
    assert_eq!(second.path, changed);
    assert!(matches!(second.kind, TagChangeKind::Changed));
    assert_eq!(second.generation, 1);

    let third = work_rx
        .try_recv()
        .expect("the second changed file is queued");
    assert_eq!(third.path, also_changed);
    assert!(matches!(third.kind, TagChangeKind::Changed));
    assert_eq!(third.generation, 1);

    assert!(work_rx.try_recv().is_err(), "exactly three jobs are queued");
    assert_eq!(
        *tracker.pending.lock().unwrap(),
        3,
        "every queued job is counted as pending work"
    );
}

#[test]
fn dispatch_removes_deleted_files_and_supersedes_their_in_flight_results() {
    let (work_tx, work_rx) = channel::<TagWorkItem>();
    let tracker = WorkTracker::default();
    let gone = PathBuf::from("gone.md");
    let never_indexed = PathBuf::from("never_indexed.md");
    let index = Arc::new(Mutex::new(TagIndex {
        tags: BTreeMap::from([(gone.clone(), vec!["stale".to_string()])]),
        generations: BTreeMap::from([(gone.clone(), 4)]),
    }));

    TagManager::dispatch(
        FileChanged::removed(vec![gone.clone(), never_indexed.clone()]),
        &work_tx,
        &tracker,
        &index,
    );

    assert!(
        work_rx.try_recv().is_err(),
        "a deletion never queues tag extraction"
    );
    assert_eq!(
        *tracker.pending.lock().unwrap(),
        0,
        "a deletion is not pending work"
    );

    let index = index.lock().unwrap();
    assert!(
        !index.tags.contains_key(&gone),
        "the removed file loses its cached tags"
    );
    assert_eq!(
        index.generations.get(&gone),
        Some(&5),
        "the generation bumps so an in-flight result is discarded"
    );
    assert_eq!(
        index.generations.get(&never_indexed),
        Some(&1),
        "a deletion of an unknown file still starts its generation"
    );
}

#[test]
fn dispatch_stops_queueing_once_the_work_queue_is_closed() {
    let (work_tx, work_rx) = channel::<TagWorkItem>();
    drop(work_rx);
    let tracker = WorkTracker::default();
    let index = Arc::new(Mutex::new(TagIndex::default()));
    let first = PathBuf::from("first.md");
    let second = PathBuf::from("second.md");

    TagManager::dispatch(
        FileChanged::discovered(vec![first.clone(), second.clone()]),
        &work_tx,
        &tracker,
        &index,
    );

    assert_eq!(
        *tracker.pending.lock().unwrap(),
        0,
        "the pending count is released when queueing fails"
    );
    let index = index.lock().unwrap();
    assert_eq!(index.generations.get(&first), Some(&1));
    assert_eq!(
        index.generations.get(&second),
        None,
        "dispatch stops instead of retrying a closed queue"
    );
}

#[test]
fn dispatch_skips_every_file_when_the_tag_index_is_poisoned() {
    let (work_tx, work_rx) = channel::<TagWorkItem>();
    let tracker = WorkTracker::default();
    let index = Arc::new(Mutex::new(TagIndex::default()));
    poison_mutex(&index);

    TagManager::dispatch(
        FileChanged::discovered(vec![PathBuf::from("a.md"), PathBuf::from("b.md")]),
        &work_tx,
        &tracker,
        &index,
    );
    TagManager::dispatch(
        FileChanged::removed(vec![PathBuf::from("c.md")]),
        &work_tx,
        &tracker,
        &index,
    );

    assert!(
        work_rx.try_recv().is_err(),
        "no work is queued without a usable index"
    );
    assert_eq!(
        *tracker.pending.lock().unwrap(),
        0,
        "no work is registered without a usable index"
    );
}

// ---------------------------------------------------------------------------
// TagManager::drain_available
// ---------------------------------------------------------------------------

#[test]
fn drain_available_reports_no_lag_for_an_empty_or_closed_bus() {
    let (work_tx, work_rx) = channel::<TagWorkItem>();
    let tracker = WorkTracker::default();
    let index = Arc::new(Mutex::new(TagIndex::default()));

    let (_open_tx, mut open_rx) = tokio::sync::broadcast::channel::<FileChanged>(4);
    assert!(
        !TagManager::drain_available(&mut open_rx, &work_tx, &tracker, &index),
        "an empty queue drains without lag"
    );
    assert!(work_rx.try_recv().is_err());

    let (closed_tx, mut closed_rx) = tokio::sync::broadcast::channel::<FileChanged>(4);
    drop(closed_tx);
    assert!(
        !TagManager::drain_available(&mut closed_rx, &work_tx, &tracker, &index),
        "a closed bus drains without lag"
    );
    assert!(work_rx.try_recv().is_err());
}

#[test]
fn drain_available_dispatches_queued_events_and_reports_the_newest_after_lag() {
    let (work_tx, work_rx) = channel::<TagWorkItem>();
    let tracker = WorkTracker::default();
    let index = Arc::new(Mutex::new(TagIndex::default()));

    let (file_tx, mut file_rx) = tokio::sync::broadcast::channel::<FileChanged>(1);
    let stale = PathBuf::from("stale.md");
    let newest = PathBuf::from("newest.md");
    file_tx
        .send(FileChanged::discovered_one(stale))
        .expect("send stale event");
    file_tx
        .send(FileChanged::updated_one(newest.clone()))
        .expect("send newest event");

    assert!(
        TagManager::drain_available(&mut file_rx, &work_tx, &tracker, &index),
        "a dropped event must be reported as lag"
    );

    let queued = work_rx
        .try_recv()
        .expect("the newest event is still dispatched");
    assert_eq!(queued.path, newest);
    assert!(matches!(queued.kind, TagChangeKind::Changed));
    assert!(
        work_rx.try_recv().is_err(),
        "the superseded event is skipped by the bus"
    );
    assert_eq!(
        *tracker.pending.lock().unwrap(),
        1,
        "only the dispatched file counts as pending"
    );
}

// ---------------------------------------------------------------------------
// TagManager::process_file_event
// ---------------------------------------------------------------------------

#[test]
fn process_file_event_dispatches_directly_while_no_resync_is_pending() {
    let (work_tx, work_rx) = channel::<TagWorkItem>();
    let tracker = WorkTracker::default();
    let index = Arc::new(Mutex::new(TagIndex::default()));
    let path = PathBuf::from("direct.md");
    let mut resync_pending = false;

    TagManager::process_file_event(
        FileChanged::discovered_one(path.clone()),
        &mut resync_pending,
        &AppConfig::default(),
        &work_tx,
        &tracker,
        &index,
        &event_sender(),
    );

    assert!(!resync_pending);
    assert_eq!(
        work_rx.try_recv().expect("the event is dispatched").path,
        path
    );
}

#[test]
fn process_file_event_keeps_the_resync_marker_when_resynchronization_fails() {
    let (work_tx, work_rx) = channel::<TagWorkItem>();
    let tracker = WorkTracker::default();
    poison_mutex(&tracker.pending);
    let index = Arc::new(Mutex::new(TagIndex::default()));
    let mut resync_pending = true;

    TagManager::process_file_event(
        FileChanged::discovered_one(PathBuf::from("deferred.md")),
        &mut resync_pending,
        &AppConfig::default(),
        &work_tx,
        &tracker,
        &index,
        &event_sender(),
    );

    assert!(
        resync_pending,
        "a failed snapshot keeps the marker so the next event retries"
    );
    assert!(
        work_rx.try_recv().is_err(),
        "the triggering event is not dispatched while a resync is pending"
    );
}

// ---------------------------------------------------------------------------
// TagManager::resynchronize failure modes
// ---------------------------------------------------------------------------

#[test]
fn resynchronize_publishes_an_empty_snapshot_for_an_unconfigured_workspace() {
    let (work_tx, _work_rx) = channel::<TagWorkItem>();
    let tracker = Arc::new(WorkTracker::default());
    let index = Arc::new(Mutex::new(TagIndex {
        tags: BTreeMap::from([(PathBuf::from("stale.md"), vec!["stale".to_string()])]),
        ..TagIndex::default()
    }));
    let sender = event_sender();
    let mut results = sender.subscribe();

    assert!(
        TagManager::resynchronize(&AppConfig::default(), &work_tx, &tracker, &index, &sender),
        "an empty workspace is a valid snapshot"
    );

    let event = results.try_recv().expect("snapshot is published");
    match event {
        BackgroundEvent::Fs(FsEvent::TagsResynchronized { files }) => {
            assert!(
                files.is_empty(),
                "expected an empty snapshot, got {files:?}"
            );
        }
        other => panic!("expected an empty tag snapshot, got {other:?}"),
    }
    assert!(
        index.lock().unwrap().tags.is_empty(),
        "a snapshot starts from an empty index"
    );
    assert_eq!(*tracker.pending.lock().unwrap(), 0);
}

#[test]
fn resynchronize_reports_failure_when_the_work_queue_is_closed() {
    let directory = tempdir().unwrap();
    std::fs::write(
        directory.path().join("note.md"),
        "---\ntags: [note]\n---\nbody",
    )
    .unwrap();
    let config = config_with_root(directory.path());

    let (work_tx, work_rx) = channel::<TagWorkItem>();
    drop(work_rx);
    let tracker = Arc::new(WorkTracker::default());
    let index = Arc::new(Mutex::new(TagIndex::default()));

    assert!(
        !TagManager::resynchronize(&config, &work_tx, &tracker, &index, &event_sender()),
        "a snapshot that cannot be queued must be reported as a failure"
    );
    assert_eq!(
        *tracker.pending.lock().unwrap(),
        0,
        "the failed registration is released again"
    );
}

#[test]
fn resynchronize_reports_failure_when_the_tag_index_is_poisoned() {
    let directory = tempdir().unwrap();
    std::fs::write(
        directory.path().join("note.md"),
        "---\ntags: [note]\n---\nbody",
    )
    .unwrap();
    let config = config_with_root(directory.path());

    let (work_tx, work_rx) = channel::<TagWorkItem>();
    let tracker = Arc::new(WorkTracker::default());
    let index = Arc::new(Mutex::new(TagIndex::default()));
    poison_mutex(&index);

    assert!(
        !TagManager::resynchronize(&config, &work_tx, &tracker, &index, &event_sender()),
        "a poisoned index cannot produce a snapshot"
    );
    assert!(work_rx.try_recv().is_err(), "no work is queued");
}

#[test]
fn resynchronize_reports_failure_when_the_result_bus_is_closed() {
    let directory = tempdir().unwrap();
    let markdown = directory.path().join("note.md");
    std::fs::write(&markdown, "---\ntags: [note]\n---\nbody").unwrap();
    let config = config_with_root(directory.path());

    let (work_tx, work_rx) = channel::<TagWorkItem>();
    // No subscriber on the background bus: every delivery fails.
    let sender = event_sender();
    let tracker = Arc::new(WorkTracker::default());
    let index = Arc::new(Mutex::new(TagIndex::default()));
    let workers = TagManager::spawn_workers(
        work_rx,
        sender.clone(),
        Arc::clone(&tracker),
        Arc::clone(&index),
    );

    assert!(
        !TagManager::resynchronize(&config, &work_tx, &tracker, &index, &sender),
        "a snapshot the UI cannot receive must be reported as a failure"
    );
    assert_eq!(
        *tracker.pending.lock().unwrap(),
        0,
        "queued work completes even when its delivery fails"
    );
    assert_eq!(
        index.lock().unwrap().tags.get(&markdown),
        Some(&vec!["note".to_string()]),
        "the index is still updated"
    );

    drop(work_tx);
    for worker in workers {
        worker.join().expect("tag worker should shut down cleanly");
    }
}

// ---------------------------------------------------------------------------
// Tag worker result handling
// ---------------------------------------------------------------------------

#[test]
fn worker_records_resync_results_without_publishing_an_event() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("resync.md");
    std::fs::write(&path, "---\ntags: [resync]\n---\nbody").unwrap();

    let (work_tx, work_rx) = channel();
    let sender = event_sender();
    let mut results = sender.subscribe();
    let index = Arc::new(Mutex::new(TagIndex {
        generations: BTreeMap::from([(path.clone(), 7)]),
        ..TagIndex::default()
    }));
    let workers = TagManager::spawn_workers(
        work_rx,
        sender,
        Arc::new(WorkTracker::default()),
        Arc::clone(&index),
    );
    work_tx
        .send(TagWorkItem {
            path: path.clone(),
            kind: TagChangeKind::Resync,
            generation: 7,
        })
        .expect("queue resync work");
    drop(work_tx);

    for worker in workers {
        worker.join().expect("tag worker should shut down cleanly");
    }

    assert_eq!(
        index.lock().unwrap().tags.get(&path),
        Some(&vec!["resync".to_string()]),
        "a resync result updates the index"
    );
    assert!(
        results.try_recv().is_err(),
        "a resync result is reported through the snapshot, not as a file event"
    );
}

#[test]
fn worker_keeps_running_when_the_tag_index_is_poisoned() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("orphan.md");
    std::fs::write(&path, "---\ntags: [orphan]\n---\nbody").unwrap();

    let (work_tx, work_rx) = channel();
    let sender = event_sender();
    let mut results = sender.subscribe();
    let tracker = Arc::new(WorkTracker::default());
    let index = Arc::new(Mutex::new(TagIndex::default()));
    poison_mutex(&index);
    let workers =
        TagManager::spawn_workers(work_rx, sender, Arc::clone(&tracker), Arc::clone(&index));
    // Take the pending slot a real dispatch would have reserved, so the
    // assertion below proves the discarded job really released it.
    tracker.register();
    work_tx
        .send(TagWorkItem {
            path,
            kind: TagChangeKind::Added,
            generation: 1,
        })
        .expect("queue work");
    drop(work_tx);

    for worker in workers {
        worker
            .join()
            .expect("a poisoned index must not kill a tag worker");
    }
    assert!(
        results.try_recv().is_err(),
        "no result is published without a usable index"
    );
    assert_eq!(
        *tracker.pending.lock().unwrap(),
        0,
        "the discarded job still releases its pending slot"
    );
}

// ---------------------------------------------------------------------------
// current_tag_paths
// ---------------------------------------------------------------------------

#[test]
fn current_tag_paths_collects_supported_files_in_sorted_order() {
    let directory = tempdir().unwrap();
    let root = directory.path();
    std::fs::write(root.join("a.md"), "body").unwrap();
    std::fs::write(root.join("B.MARKDOWN"), "body").unwrap();
    std::fs::write(root.join("c.txt"), "body").unwrap();
    std::fs::write(root.join("d.Pdf"), b"%PDF-1.7").unwrap();
    std::fs::write(root.join(".hidden.md"), "body").unwrap();
    std::fs::write(root.join("e.rs"), "fn main() {}").unwrap();
    std::fs::write(root.join("noext"), "body").unwrap();
    std::fs::write(root.join("notes.md.bak"), "body").unwrap();
    std::fs::create_dir(root.join("dir.md")).unwrap();
    let nested = root.join("sub").join("deep").join("nested.md");
    std::fs::create_dir_all(nested.parent().unwrap()).unwrap();
    std::fs::write(&nested, "body").unwrap();
    std::fs::create_dir_all(root.join(".git")).unwrap();
    std::fs::write(root.join(".git").join("config.md"), "body").unwrap();

    let paths = current_tag_paths(&config_with_root(root)).expect("scan succeeds");
    assert_eq!(
        paths,
        vec![
            root.join(".hidden.md"),
            root.join("B.MARKDOWN"),
            root.join("a.md"),
            root.join("c.txt"),
            root.join("d.Pdf"),
            nested,
        ],
        "supported extensions are collected once, sorted, without .git or directories"
    );
}

#[test]
fn current_tag_paths_skips_only_image_libraries() {
    let directory = tempdir().unwrap();
    let markdown = directory.path().join("note.md");
    std::fs::write(&markdown, "---\ntags: [note]\n---\nbody").unwrap();
    let root = directory.path();

    for (kind, included) in [
        ("image", false),
        ("IMAGE", false),
        ("Image", false),
        ("text", true),
        ("markdown", true),
        ("pdf", true),
        ("", true),
    ] {
        let mut config = AppConfig::default();
        config
            .content_libraries
            .push(library("library", kind, root));

        let paths = current_tag_paths(&config).expect("scan succeeds");
        assert_eq!(
            paths.contains(&markdown),
            included,
            "library kind {kind:?} inclusion mismatch, scanned {paths:?}"
        );
    }
}

#[test]
fn current_tag_paths_skips_missing_roots_and_lists_each_file_once() {
    let directory = tempdir().unwrap();
    let note = directory.path().join("note.md");
    std::fs::write(&note, "body").unwrap();

    assert!(
        current_tag_paths(&AppConfig::default())
            .expect("scan succeeds")
            .is_empty(),
        "an unconfigured workspace has no taggable files"
    );

    let mut config = AppConfig::default();
    config
        .content_libraries
        .push(library("missing", "text", &directory.path().join("absent")));
    assert!(
        current_tag_paths(&config)
            .expect("a missing root is skipped, not an error")
            .is_empty(),
        "a library whose root folder does not exist contributes nothing"
    );

    let mut config = AppConfig::default();
    config
        .content_libraries
        .push(library("first", "text", directory.path()));
    config
        .content_libraries
        .push(library("second", "text", directory.path()));
    assert_eq!(
        current_tag_paths(&config).expect("scan succeeds"),
        vec![note],
        "overlapping libraries contribute each file once"
    );
}
