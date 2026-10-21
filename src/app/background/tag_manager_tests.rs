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

    let mut config = AppConfig::default();
    config
        .content_libraries
        .push(crate::config::ContentLibrary {
            name: "test".into(),
            kind: "text".into(),
            root_folder: directory.path().to_string_lossy().into_owned(),
            readonly: true,
            priority: 0,
        });

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
    let mut config = AppConfig::default();
    config
        .content_libraries
        .push(crate::config::ContentLibrary {
            name: "test".into(),
            kind: "text".into(),
            root_folder: directory.path().to_string_lossy().into_owned(),
            readonly: true,
            priority: 0,
        });

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
