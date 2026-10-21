//! Tests for `app/background_task.rs`.

use super::*;
use crate::bus::events::file::{FileChangedKind, FileEvent};
use crate::bus::events::typed::FsEvent;
#[cfg(feature = "image-library")]
use crate::bus::events::typed::ProcessEvent;
use crate::config::{AppConfig, ContentLibrary};
use tempfile::tempdir;

/// True when `ev` is a `[FileChanged]` event with the given kind.
fn is_file_changed_of_kind(ev: &FileEvent, kind: FileChangedKind) -> bool {
    matches!(ev, FileEvent::FileChanged(changed) if changed.kind == kind)
}

/// Helper: pump the typed channel until the file watcher signals
/// completion (either with or without a real `RecommendedWatcher`
/// handle), or the deadline elapses. Returns the matched event so
/// callers can also assert on the variant.
fn wait_for_finished(task: &Task, timeout_secs: u64) -> Option<BackgroundEvent> {
    let mut rx = task.tx.subscribe();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime for test");
    rt.block_on(async move {
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout_secs);
        while tokio::time::Instant::now() < deadline {
            let timeout = deadline - tokio::time::Instant::now();
            if let Ok(ev) = tokio::time::timeout(timeout, rx.recv()).await
                && let Ok(event) = ev
                && matches!(
                    event,
                    BackgroundEvent::Fs(FsEvent::Finished | FsEvent::FinishedWithoutWatcher)
                )
            {
                return Some(event);
            }
        }
        None
    })
}

#[test]
fn test_background_task_new_no_libraries() {
    let config = AppConfig::default();
    let task = Task::new_for_test(config);

    let got = wait_for_finished(&task, 5);
    assert!(got.is_some(), "Should complete initialization");
}

#[test]
fn test_cancel_signals_indexer_without_panicking() {
    // Regression for the no-op `Task::cancel` stub: after the fix the
    // method must store into the shared `Arc<AtomicBool>` and not
    // panic on repeated or out-of-order calls.
    let config = AppConfig::default();
    let task = Task::new_for_test(config);
    task.cancel();
    task.cancel(); // second call must also be safe
    // Drive the background to completion so the test exits cleanly.
    let _ = wait_for_finished(&task, 5);
}

#[test]
fn test_background_task_indexing() {
    let mut config = AppConfig::default();
    let dir = tempdir().unwrap();
    let md = dir.path().join("test.md");
    std::fs::write(&md, "test").unwrap();
    let pdf = dir.path().join("test.pdf");
    std::fs::write(&pdf, "pdf").unwrap();

    config.content_libraries.push(ContentLibrary {
        name: "test".to_string(),
        kind: "text".to_string(),
        root_folder: dir.path().to_string_lossy().to_string(),
        readonly: true,
        priority: 0,
    });

    let task = Task::new_for_test(config);

    let got = wait_for_finished(&task, 5);
    assert!(got.is_some(), "Should complete indexing");
}

#[test]
fn test_background_task_retains_finished_watcher_in_task_slot() {
    let mut config = AppConfig::default();
    let dir = tempdir().unwrap();
    config.content_libraries.push(ContentLibrary {
        name: "test".to_string(),
        kind: "text".to_string(),
        root_folder: dir.path().to_string_lossy().to_string(),
        readonly: true,
        priority: 0,
    });

    let task = Task::new_for_test(config);
    assert!(
        wait_for_finished(&task, 5).is_some(),
        "indexing should finish"
    );
    assert!(
        task.take_finished_watcher().is_some(),
        "the watcher must be retained in the Task slot that the UI owns"
    );
}

#[test]
fn test_initial_scan_publishes_discovered_events() {
    let mut config = AppConfig::default();
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("a.md"), "test").unwrap();
    std::fs::write(dir.path().join("b.md"), "test").unwrap();
    std::fs::write(dir.path().join("c.txt"), "test").unwrap();

    config.content_libraries.push(ContentLibrary {
        name: "test".to_string(),
        kind: "text".to_string(),
        root_folder: dir.path().to_string_lossy().to_string(),
        readonly: true,
        priority: 0,
    });

    let task = Task::new_for_test(config);
    let reader = task.file_event_bus.subscribe();

    let got = wait_for_finished(&task, 5);
    assert!(got.is_some(), "Should complete initialization");

    let mut events = Vec::new();
    while let Ok(ev) = reader.recv_timeout(std::time::Duration::from_millis(100)) {
        events.push(ev);
    }

    let discovered: Vec<_> = events
        .iter()
        .filter(|e| is_file_changed_of_kind(e, FileChangedKind::Added))
        .collect();
    let total: usize = discovered.iter().map(|e| e.file_paths().len()).sum();
    assert_eq!(total, 3);
    let mut names: Vec<String> = discovered
        .iter()
        .flat_map(|e| e.file_paths())
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    names.sort();
    assert_eq!(names, vec!["a.md", "b.md", "c.txt"]);
}

#[test]
fn test_bus_subscribers_see_discovered_events() {
    let mut config = AppConfig::default();
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("a.md"), "test").unwrap();

    config.content_libraries.push(ContentLibrary {
        name: "test".to_string(),
        kind: "text".to_string(),
        root_folder: dir.path().to_string_lossy().to_string(),
        readonly: true,
        priority: 0,
    });

    let task = Task::new_for_test(config);
    let tag_reader = task.file_event_bus.subscribe();
    let tree_reader = task.file_event_bus.subscribe();

    let _ = wait_for_finished(&task, 5);

    let mut tag_events = Vec::new();
    while let Ok(ev) = tag_reader.recv_timeout(std::time::Duration::from_millis(100)) {
        tag_events.push(ev);
    }
    let mut tree_events = Vec::new();
    while let Ok(ev) = tree_reader.recv_timeout(std::time::Duration::from_millis(100)) {
        tree_events.push(ev);
    }

    assert_eq!(tag_events.len(), 2);
    assert_eq!(tree_events.len(), 2);
    // The scan emits one `[FileChanged][Added]` file batch and — with the
    // library root enumerated — one `[DirectoryChanged][Added]` event,
    // kind-homogeneous ([REQ-320..334]).
    let file_event = tag_events
        .iter()
        .find(|e| is_file_changed_of_kind(e, FileChangedKind::Added))
        .expect("file event");
    let dir_event = tag_events
        .iter()
        .find(|e| matches!(e, FileEvent::DirectoryChanged(_)))
        .expect("directory event");
    let tree_file_event = tree_events
        .iter()
        .find(|e| is_file_changed_of_kind(e, FileChangedKind::Added))
        .expect("file event for tree reader");
    let tree_dir_event = tree_events
        .iter()
        .find(|e| matches!(e, FileEvent::DirectoryChanged(_)))
        .expect("directory event for tree reader");
    // Both subscribers observe the same file payloads.
    assert_eq!(file_event.file_paths(), tree_file_event.file_paths());
    assert_eq!(file_event.file_paths(), vec![dir.path().join("a.md")]);
    // The directory event carries the enumerated library root ([REQ-321]).
    let FileEvent::DirectoryChanged(dir_changed) = dir_event else {
        panic!("expected [DirectoryChanged]");
    };
    assert_eq!(dir_changed.path, dir.path());
    let FileEvent::DirectoryChanged(tree_dir_changed) = tree_dir_event else {
        panic!("expected [DirectoryChanged] for tree reader");
    };
    assert_eq!(tree_dir_changed.path, dir.path());
}

#[test]
fn test_initial_scan_publishes_pdf_discovered_to_bus() {
    let mut config = AppConfig::default();
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("report.pdf"), b"dummy pdf").unwrap();

    config.content_libraries.push(ContentLibrary {
        name: "test".to_string(),
        kind: "text".to_string(),
        root_folder: dir.path().to_string_lossy().to_string(),
        readonly: true,
        priority: 0,
    });

    let task = Task::new_for_test(config);
    let reader = task.file_event_bus.subscribe();

    let _ = wait_for_finished(&task, 5);

    let mut events = Vec::new();
    while let Ok(ev) = reader.recv_timeout(std::time::Duration::from_millis(100)) {
        events.push(ev);
    }

    let pdf_discovered = events
        .iter()
        .find(|e| {
            is_file_changed_of_kind(e, FileChangedKind::Added)
                && e.file_paths()
                    .first()
                    .map(|p| p.extension().and_then(|x| x.to_str()) == Some("pdf"))
                    .unwrap_or(false)
        })
        .expect("initial scan should publish [FileChanged][Added] for PDFs");
    assert_eq!(
        pdf_discovered.file_paths()[0],
        dir.path().join("report.pdf")
    );
}

#[test]
fn test_bus_published_pdf_triggers_conversion_via_subscriber() {
    use crate::background::LogCategory;
    use crate::bus::events::typed::ProcessEvent;

    let mut config = AppConfig::default();
    let dir = tempdir().unwrap();

    #[cfg(windows)]
    let cmd_template = Some(vec![
        "cmd".to_string(),
        "/C".to_string(),
        "echo done".to_string(),
    ]);
    #[cfg(not(windows))]
    let cmd_template = Some(vec!["true".to_string()]);
    config.pdf_converter_command = cmd_template;

    config.content_libraries.push(ContentLibrary {
        name: "test".to_string(),
        kind: "text".to_string(),
        root_folder: dir.path().to_string_lossy().to_string(),
        readonly: true,
        priority: 0,
    });

    let task = Task::new_for_test(config);

    let _ = wait_for_finished(&task, 5);

    std::thread::sleep(std::time::Duration::from_millis(200));

    let pdf_path = dir.path().join("dropped.pdf");
    std::fs::write(&pdf_path, b"dummy").unwrap();
    task.file_event_bus
        .publish(FileEvent::discovered_one(pdf_path.clone()));

    let mut rx = task.tx.subscribe();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime for test");
    let (saw_success, all_messages) = rt.block_on(async move {
        let mut saw_success = false;
        let mut all_messages: Vec<String> = Vec::new();
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(5);
        while tokio::time::Instant::now() < deadline {
            let timeout = deadline - tokio::time::Instant::now();
            let ev = match tokio::time::timeout(timeout, rx.recv()).await {
                Ok(Ok(ev)) => ev,
                _ => continue,
            };
            match ev {
                BackgroundEvent::Process(ProcessEvent::LogEntry(entry)) => {
                    all_messages.push(format!("{:?}: {}", entry.category, entry.message));
                    if entry.category == LogCategory::PdfConverter
                        && entry.message.contains("Successfully converted")
                    {
                        saw_success = true;
                        break;
                    }
                }
                other => {
                    all_messages.push(format!("other: {:?}", std::mem::discriminant(&other)));
                }
            }
        }
        (saw_success, all_messages)
    });
    if !saw_success {
        eprintln!("Test saw messages: {:?}", all_messages);
    }
    assert!(
        saw_success,
        "Bus-published PDF Discovered event should reach the PDF converter worker"
    );
}

#[test]
fn test_pdf_worker_publishes_discovered_for_output_md() {
    let mut config = AppConfig::default();
    let dir = tempdir().unwrap();

    #[cfg(windows)]
    let cmd_template = Some(vec![
        "cmd".to_string(),
        "/C".to_string(),
        "echo done".to_string(),
    ]);
    #[cfg(not(windows))]
    let cmd_template = Some(vec!["true".to_string()]);
    config.pdf_converter_command = cmd_template;

    config.content_libraries.push(ContentLibrary {
        name: "test".to_string(),
        kind: "text".to_string(),
        root_folder: dir.path().to_string_lossy().to_string(),
        readonly: true,
        priority: 0,
    });

    let task = Task::new_for_test(config);

    let _ = wait_for_finished(&task, 5);

    let bus_reader = task.file_event_bus.subscribe();
    std::thread::sleep(std::time::Duration::from_millis(200));

    let pdf_path = dir.path().join("dropped.pdf");
    std::fs::write(&pdf_path, b"dummy").unwrap();
    task.file_event_bus
        .publish(FileEvent::discovered_one(pdf_path.clone()));

    let expected_md = {
        let mut p = pdf_path.clone();
        p.set_extension("md");
        p
    };

    let start = std::time::Instant::now();
    let mut saw_discovered = false;
    while start.elapsed().as_secs() < 5 {
        match bus_reader.recv_timeout(std::time::Duration::from_millis(100)) {
            Ok(event) => {
                if is_file_changed_of_kind(&event, FileChangedKind::Added)
                    && event.file_paths().contains(&expected_md)
                {
                    saw_discovered = true;
                    break;
                }
            }
            Err(_) => continue,
        }
    }
    assert!(
        saw_discovered,
        "PDF worker should publish a Discovered event for the output .md after a successful conversion"
    );
}

#[cfg(feature = "image-library")]
#[test]
fn test_bus_published_image_triggers_vision_via_subscriber() {
    use crate::background::LogCategory;
    use crate::config::LlmConfig;

    let mut config = AppConfig::default();
    let dir = tempdir().unwrap();

    config.models.insert(
        "test-vision".to_string(),
        LlmConfig {
            model: "test-vision".to_string(),
            api_key: "dummy".to_string(),
            api_url: "http://127.0.0.1:1".to_string(),
            cost: None,
            use_case: vec!["vision".to_string()],
        },
    );

    config.content_libraries.push(ContentLibrary {
        name: "test".to_string(),
        kind: "image".to_string(),
        root_folder: dir.path().to_string_lossy().to_string(),
        readonly: true,
        priority: 0,
    });

    let task = Task::new_for_test(config);

    let _ = wait_for_finished(&task, 5);

    std::thread::sleep(std::time::Duration::from_millis(200));

    let img_path = dir.path().join("dropped.png");
    std::fs::write(&img_path, b"dummy image data").unwrap();
    task.file_event_bus
        .publish(FileEvent::discovered_one(img_path.clone()));

    let rx = task.rx_std();
    let start = std::time::Instant::now();
    let mut all_messages: Vec<String> = Vec::new();
    let mut saw_analyzing = false;
    while start.elapsed().as_secs() < 10 {
        if let Ok(ev) = rx.recv_timeout(std::time::Duration::from_millis(100))
            && let BackgroundEvent::Process(ProcessEvent::LogEntry(entry)) = ev
        {
            all_messages.push(format!("{:?}: {}", entry.category, entry.message));
            if entry.category == LogCategory::ImageVision
                && entry.message.contains("Analyzing image")
            {
                saw_analyzing = true;
                break;
            }
        }
    }
    if !saw_analyzing {
        eprintln!("Test saw messages: {:?}", all_messages);
    }
    assert!(
        saw_analyzing,
        "Bus-published image Discovered event should reach the image-vision worker"
    );
}

/// Regression: `Task::new(bus)` must not start any background work
/// until the first [`ConfigArrived`] event is observed. The
/// observable signal is the file-event bus subscriber count:
/// workers (which subscribe) have not been spawned yet, so the
/// count is zero.
///
/// Note: the task falls back to [`AppConfig::default`] after
/// [`crate::config::CONFIG_ARRIVAL_TIMEOUT`] if no event
/// arrives, so the absence of a `Finished` message is *not*
/// a reliable assertion. The subscriber count is.
#[test]
fn test_task_does_not_spawn_workers_before_publish() {
    let bus = crate::bus::config::config_bus();
    let task = Task::new(bus.clone());

    assert_eq!(
        task.file_event_bus.subscriber_count(),
        0,
        "no worker should be subscribed to the file-event bus before config arrives"
    );
}

/// Regression: after `publish(ConfigArrived { .. })` the workers
/// spin up and the scan completes within a reasonable timeout.
/// (`BusRouter` is spawned *after* the scan sends `Finished`,
/// so we cannot rely on `subscriber_count() > 0` immediately
/// after observing the message — the assertion is intentionally
/// on the `Finished` delivery, which is the user-visible
/// signal that the workers have done their first pass.)
#[test]
fn test_task_spawns_workers_after_publish() {
    let bus = crate::bus::config::config_bus();
    let task = Task::new(bus.clone());

    // Publish before the worker thread has a chance to poll.
    bus.publish(ConfigArrived::new(AppConfig::default()));

    // Wait for the scan to finish so we know all workers are
    // alive and subscribed.
    let finished = wait_for_finished(&task, 5).is_some();
    assert!(finished, "task should finish after config arrives");
}
