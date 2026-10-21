//! Background executor for user-triggered file reads, writes, and path mutations.
//!
//! Unit tests live in the sibling `user_file_operation_tests.rs` sidecar.

use crate::bus::events::typed::{BackgroundEventSender, ProcessEvent};
use crate::markdown::DocumentContent;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

const FILE_OPERATION_WORKER_COUNT: usize = 4;
const INVALID_DIRECTORY_NAME: &str = "Directory name contains invalid characters";
const INVALID_DOCUMENT_NAME: &str = "Document name contains invalid characters";

/// Self-contained user-requested file operation executed outside the UI thread.
#[derive(Debug, Clone, PartialEq)]
pub enum UserFileOperation {
    /// Load a prompt/skill file and preserve its target context.
    LoadPrompt {
        /// File containing the prompt body.
        path: PathBuf,
        /// Optional directory targeted by this prompt.
        target_dir: Option<PathBuf>,
        /// Optional file targeted by this prompt.
        target_file: Option<PathBuf>,
    },
    /// Open a file in the configured inline or system editor.
    OpenEditor {
        /// File to open.
        path: PathBuf,
        /// Whether FastMD's inline editor is enabled.
        inline: bool,
    },
    /// Persist an editor snapshot to disk.
    SaveEditor {
        /// File receiving the snapshot.
        path: PathBuf,
        /// Full document contents including front matter.
        content: String,
    },
    /// Move a file or directory beneath a destination directory.
    Move {
        /// Existing file or directory.
        source: PathBuf,
        /// Destination directory.
        destination: PathBuf,
    },
    /// Send a file or directory to the system recycle bin.
    Delete {
        /// Path to remove.
        path: PathBuf,
    },
    /// Create a directory below a parent directory.
    CreateDirectory {
        /// Parent directory.
        parent: PathBuf,
        /// New directory basename.
        name: String,
    },
    /// Create a Markdown document below a parent directory.
    CreateDocument {
        /// Parent directory.
        parent: PathBuf,
        /// New document basename.
        name: String,
    },
    /// Export Markdown to a selected PDF destination.
    SaveAsPdf {
        /// Source Markdown path.
        markdown_path: PathBuf,
        /// Destination PDF path.
        output_path: PathBuf,
    },
    /// Reveal a path in the operating-system file explorer.
    Reveal {
        /// Path to reveal.
        path: PathBuf,
    },
}

/// Completion of a user file operation, delivered back to the UI thread.
#[derive(Debug, Clone, PartialEq)]
pub enum UserFileOperationResult {
    /// Prompt file read and parsing completed.
    PromptLoaded {
        /// Parsed Markdown body.
        content: Result<String, String>,
        /// Optional directory targeted by the prompt.
        target_dir: Option<PathBuf>,
        /// Optional file targeted by the prompt.
        target_file: Option<PathBuf>,
    },
    /// Editor open request completed.
    EditorOpened {
        /// File that was opened.
        path: PathBuf,
        /// `Some` contains file contents for the inline editor; `None` means
        /// the system editor was launched.
        content: Result<Option<String>, String>,
    },
    /// Editor snapshot write completed.
    EditorSaved {
        /// File that received the snapshot.
        path: PathBuf,
        /// Snapshot written by the worker.
        saved_content: String,
        /// Write status.
        result: Result<(), String>,
    },
    /// Move operation completed.
    Moved {
        /// Original path.
        source: PathBuf,
        /// New path.
        destination: PathBuf,
        /// Whether the source was a directory.
        is_directory: bool,
        /// Filesystem operation status.
        result: Result<(), String>,
    },
    /// Delete operation completed.
    Deleted {
        /// Removed path.
        path: PathBuf,
        /// Whether the path was a directory.
        is_directory: bool,
        /// Filesystem operation status.
        result: Result<(), String>,
    },
    /// Directory creation completed.
    DirectoryCreated {
        /// Created directory path.
        path: PathBuf,
        /// Filesystem operation status.
        result: Result<(), String>,
    },
    /// Markdown document creation completed.
    DocumentCreated {
        /// Created document path on success.
        path: Result<PathBuf, String>,
    },
    /// Save-as-PDF export completed.
    PdfSaved {
        /// Source Markdown path.
        markdown_path: PathBuf,
        /// Export result path or error.
        result: Result<PathBuf, String>,
    },
    /// File-explorer reveal request completed.
    Revealed {
        /// Path that was revealed.
        path: PathBuf,
        /// Shell request status.
        result: Result<(), String>,
    },
}

/// Bounded-concurrency worker pool for user-triggered file operations.
pub struct UserFileOperationWorker {
    sender: Sender<UserFileOperation>,
    workers: Vec<JoinHandle<()>>,
}

impl UserFileOperationWorker {
    /// Starts four workers and returns the sender used by the orchestrator.
    pub fn spawn(tx: BackgroundEventSender) -> Self {
        let (sender, receiver) = mpsc::channel();
        let receiver = Arc::new(Mutex::new(receiver));
        let workers = (0..FILE_OPERATION_WORKER_COUNT)
            .map(|_| {
                let receiver = Arc::clone(&receiver);
                let tx = tx.clone();
                std::thread::spawn(move || Self::run_worker(receiver, tx))
            })
            .collect();
        Self { sender, workers }
    }

    /// Queues an operation without waiting for its filesystem work.
    pub fn submit(&self, operation: UserFileOperation) -> Result<(), String> {
        self.sender
            .send(operation)
            .map_err(|error| format!("file operation worker queue closed: {error}"))
    }

    fn run_worker(receiver: Arc<Mutex<Receiver<UserFileOperation>>>, tx: BackgroundEventSender) {
        loop {
            let operation = {
                let Ok(receiver) = receiver.lock() else {
                    tracing::error!(
                        name = "file_operation.queue.poisoned",
                        "File operation queue was poisoned."
                    );
                    break;
                };
                match receiver.recv() {
                    Ok(operation) => operation,
                    Err(_) => break,
                }
            };
            let result = execute(operation, &tx);
            if let Err(error) = tx.send(ProcessEvent::UserFileOperationCompleted(result).into()) {
                tracing::warn!(
                    name = "file_operation.result.delivery_failed",
                    error = %error,
                    "File operation result could not be delivered to the UI."
                );
            }
        }
    }
}

impl Drop for UserFileOperationWorker {
    fn drop(&mut self) {
        // The application owns the worker for its lifetime. Joining here would
        // block shutdown if an OS filesystem call is stalled, so workers are
        // allowed to exit after the channel closes.
        self.workers.clear();
    }
}

fn execute(operation: UserFileOperation, tx: &BackgroundEventSender) -> UserFileOperationResult {
    match operation {
        UserFileOperation::LoadPrompt {
            path,
            target_dir,
            target_file,
        } => UserFileOperationResult::PromptLoaded {
            content: std::fs::read_to_string(&path)
                .map(|raw| DocumentContent::parse(&raw).body)
                .map_err(|error| error.to_string()),
            target_dir,
            target_file,
        },
        UserFileOperation::OpenEditor { path, inline } => {
            let content = if inline {
                std::fs::read_to_string(&path)
                    .map(Some)
                    .map_err(|error| error.to_string())
            } else {
                crate::ui_iced::os_shell::open_in_system_editor(&path);
                Ok(None)
            };
            UserFileOperationResult::EditorOpened { path, content }
        }
        UserFileOperation::SaveEditor {
            path,
            content: saved_content,
        } => UserFileOperationResult::EditorSaved {
            result: std::fs::write(&path, &saved_content).map_err(|error| error.to_string()),
            path,
            saved_content,
        },
        UserFileOperation::Move {
            source,
            destination,
        } => {
            let is_directory = source.is_dir();
            let mut new_path = destination;
            if let Some(name) = source.file_name() {
                new_path.push(name);
            }
            UserFileOperationResult::Moved {
                result: std::fs::rename(&source, &new_path).map_err(|error| error.to_string()),
                source,
                destination: new_path,
                is_directory,
            }
        }
        UserFileOperation::Delete { path } => {
            let is_directory = path.is_dir();
            UserFileOperationResult::Deleted {
                result: crate::utils::recycle_bin::delete(&path).map_err(|error| error.to_string()),
                path,
                is_directory,
            }
        }
        UserFileOperation::CreateDirectory { parent, name } => {
            let path = parent.join(&name);
            let result = if crate::utils::path::is_safe_basename(&name) {
                std::fs::create_dir_all(&path).map_err(|error| error.to_string())
            } else {
                Err(INVALID_DIRECTORY_NAME.to_string())
            };
            UserFileOperationResult::DirectoryCreated { path, result }
        }
        UserFileOperation::CreateDocument { parent, name } => {
            let path = if crate::utils::path::is_safe_basename(&name) {
                crate::utils::path::write_new_document(&parent, &name)
                    .map_err(|error| error.to_string())
            } else {
                Err(INVALID_DOCUMENT_NAME.to_string())
            };
            UserFileOperationResult::DocumentCreated { path }
        }
        UserFileOperation::SaveAsPdf {
            markdown_path,
            output_path,
        } => {
            let mut job = crate::export::pdf::SaveAsPdfJob::from_path(markdown_path.clone());
            job.output_path = Some(output_path);
            UserFileOperationResult::PdfSaved {
                markdown_path,
                result: crate::export::pdf::execute_save_as_pdf_blocking(job, Some(tx.clone()))
                    .map_err(|error| error.to_string()),
            }
        }
        UserFileOperation::Reveal { path } => {
            crate::ui_iced::os_shell::show_in_file_explorer(&path);
            UserFileOperationResult::Revealed {
                path,
                result: Ok(()),
            }
        }
    }
}

#[cfg(test)]
#[path = "user_file_operation_tests.rs"]
mod tests;
