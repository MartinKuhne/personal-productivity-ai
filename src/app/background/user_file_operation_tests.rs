//! Unit tests for user-requested background file operations.

use super::*;
use crate::utils::recycle_bin::{MockRecycleBinBackend, with_thread_backend};
use std::sync::Arc;
use tempfile::tempdir;

fn sender() -> BackgroundEventSender {
    let (tx, _) = tokio::sync::broadcast::channel(16);
    BackgroundEventSender::new(tx)
}

#[test]
fn load_prompt_reads_and_parses_body_without_front_matter() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("prompt.md");
    std::fs::write(&path, "---\ntitle: Prompt\n---\nUse this prompt\n").unwrap();

    let result = execute(
        UserFileOperation::LoadPrompt {
            path,
            target_dir: None,
            target_file: None,
        },
        &sender(),
    );

    assert!(matches!(
        result,
        UserFileOperationResult::PromptLoaded {
            content: Ok(content),
            target_dir: None,
            target_file: None,
        } if content == "\nUse this prompt\n"
    ));
}

#[test]
fn load_prompt_reports_missing_file() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("missing.md");

    let result = execute(
        UserFileOperation::LoadPrompt {
            path,
            target_dir: None,
            target_file: None,
        },
        &sender(),
    );

    assert!(matches!(
        result,
        UserFileOperationResult::PromptLoaded {
            content: Err(_),
            ..
        }
    ));
}

#[test]
fn inline_editor_load_and_save_run_in_worker_operation() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("note.md");
    std::fs::write(&path, "before").unwrap();

    let opened = execute(
        UserFileOperation::OpenEditor {
            path: path.clone(),
            inline: true,
        },
        &sender(),
    );
    assert!(matches!(
        opened,
        UserFileOperationResult::EditorOpened {
            content: Ok(Some(content)), ..
        } if content == "before"
    ));

    let saved = execute(
        UserFileOperation::SaveEditor {
            path: path.clone(),
            content: "after".to_string(),
        },
        &sender(),
    );
    assert!(matches!(
        saved,
        UserFileOperationResult::EditorSaved { result: Ok(()), .. }
    ));
    assert_eq!(std::fs::read_to_string(path).unwrap(), "after");
}

#[test]
fn move_and_document_creation_report_updated_paths() {
    let directory = tempdir().unwrap();
    let source = directory.path().join("source.md");
    let destination = directory.path().join("destination");
    std::fs::write(&source, "contents").unwrap();
    std::fs::create_dir(&destination).unwrap();

    let moved = execute(
        UserFileOperation::Move {
            source: source.clone(),
            destination: destination.clone(),
        },
        &sender(),
    );
    let moved_path = destination.join("source.md");
    assert!(matches!(
        moved,
        UserFileOperationResult::Moved {
            destination: ref path,
            is_directory: false,
            result: Ok(()), ..
        } if path == &moved_path
    ));

    let created = execute(
        UserFileOperation::CreateDocument {
            parent: destination,
            name: "new-note".to_string(),
        },
        &sender(),
    );
    assert!(matches!(
        created,
        UserFileOperationResult::DocumentCreated {
            path: Ok(path),
        } if path.file_name().is_some_and(|name| name == "new-note.md")
    ));
}

#[test]
fn directory_create_rejects_unsafe_names_and_creates_valid_names() {
    let directory = tempdir().unwrap();
    let unsafe_result = execute(
        UserFileOperation::CreateDirectory {
            parent: directory.path().to_path_buf(),
            name: "../escape".to_string(),
        },
        &sender(),
    );
    assert!(matches!(
        unsafe_result,
        UserFileOperationResult::DirectoryCreated { result: Err(_), .. }
    ));

    let created_path = directory.path().join("created");
    let valid_result = execute(
        UserFileOperation::CreateDirectory {
            parent: directory.path().to_path_buf(),
            name: "created".to_string(),
        },
        &sender(),
    );
    assert!(matches!(
        valid_result,
        UserFileOperationResult::DirectoryCreated {
            path,
            result: Ok(()),
        } if path == created_path
    ));
}

#[test]
fn delete_uses_the_injected_recycle_backend() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("trash-me.md");
    std::fs::write(&path, "temporary test file").unwrap();
    let backend = Arc::new(MockRecycleBinBackend::new());

    let result = with_thread_backend(backend.clone(), || {
        execute(UserFileOperation::Delete { path: path.clone() }, &sender())
    });

    assert!(matches!(
        result,
        UserFileOperationResult::Deleted { result: Ok(()), .. }
    ));
    assert_eq!(backend.recorded_paths(), vec![path]);
}

#[test]
fn worker_delivers_operation_completion_as_a_process_event() {
    let (tx, _) = tokio::sync::broadcast::channel(16);
    let sender = BackgroundEventSender::new(tx);
    let mut events = sender.subscribe();
    let worker = UserFileOperationWorker::spawn(sender);
    let directory = tempdir().unwrap();
    let path = directory.path().join("prompt.md");
    std::fs::write(&path, "body").unwrap();

    worker
        .submit(UserFileOperation::LoadPrompt {
            path,
            target_dir: None,
            target_file: None,
        })
        .unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let event = runtime.block_on(async {
        tokio::time::timeout(std::time::Duration::from_secs(2), events.recv())
            .await
            .unwrap()
            .unwrap()
    });
    assert!(matches!(
        event,
        crate::bus::events::typed::BackgroundEvent::Process(
            ProcessEvent::UserFileOperationCompleted(
                UserFileOperationResult::PromptLoaded {
                    content: Ok(content), ..
                }
            )
        ) if content == "body"
    ));
}
