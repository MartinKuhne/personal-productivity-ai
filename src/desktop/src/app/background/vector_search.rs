//! Optional Markdown vector indexing and search worker.

use crate::bus::core::{Bus, BusReader};
use crate::bus::events::file::{FileEvent, FileEventKind};
use crate::bus::events::typed::BackgroundEvent;
use crate::config::AppConfig;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use sahomedb::prelude::{Collection, Config, Database, Distance, Metadata, Record, Vector};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const CHUNK_SIZE: usize = 1200;
const CHUNK_OVERLAP: usize = 200;
const COLLECTION_NAME: &str = "markdown";

/// A chunk of Markdown content with its stable content hash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownChunk {
    /// Source path.
    pub path: PathBuf,
    /// Chunk body.
    pub text: String,
    /// Stable hash of the chunk body.
    pub hash: String,
}

/// Shared service handle injected into the agent tool context.
#[derive(Clone)]
pub struct VectorSearchService {
    state: Arc<Mutex<VectorState>>,
}

struct VectorState {
    model: Option<TextEmbedding>,
    collection: Collection,
    database: Option<Database>,
}

impl VectorSearchService {
    /// Create an empty service; model loading is deferred to the background worker.
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(VectorState {
                model: None,
                collection: Collection::new(&collection_config()),
                database: None,
            })),
        }
    }

    pub(crate) fn start(
        &self,
        config: AppConfig,
        reader: BusReader<FileEvent>,
        tx: std::sync::mpsc::Sender<BackgroundEvent>,
    ) {
        let service = self.clone();
        std::thread::spawn(move || {
            let database_path = config
                .content_libraries
                .first()
                .map(|library| PathBuf::from(&library.root_folder).join(".fastmd-vector-index"));
            let model = TextEmbedding::try_new(InitOptions {
                model_name: EmbeddingModel::BGESmallENV15,
                show_download_progress: false,
                ..Default::default()
            });
            let Ok(model) = model else {
                let _ =
                    tx.send(BackgroundLog::failed("Embedding model initialization failed").into());
                return;
            };
            let mut state = service.state.lock().expect("vector state lock poisoned");
            state.model = Some(model);
            if let Some(path) = database_path {
                if let Err(error) = std::fs::create_dir_all(&path) {
                    let _ = tx.send(
                        BackgroundLog::failed(&format!("Vector index directory: {error}")).into(),
                    );
                } else if let Ok(database) = Database::open(&path.to_string_lossy()) {
                    state.database = Some(database);
                }
            }
            drop(state);
            let mut processed = 0usize;
            for library in &config.content_libraries {
                for entry in walkdir::WalkDir::new(&library.root_folder)
                    .into_iter()
                    .filter_map(Result::ok)
                {
                    if entry.file_type().is_file() && is_markdown(entry.path()) {
                        service.index_path(entry.path());
                        processed += 1;
                        log_progress(processed, &tx);
                    }
                }
            }
            while let Ok(event) = reader.recv() {
                for path in event.paths {
                    match event.kind {
                        FileEventKind::Discovered | FileEventKind::Updated => {
                            if is_markdown(&path) {
                                service.index_path(&path);
                                processed += 1;
                                log_progress(processed, &tx);
                            }
                        }
                        FileEventKind::Removed => service.remove_path(&path),
                        FileEventKind::DirDiscovered | FileEventKind::DirRemoved => {}
                    }
                }
            }
        });
    }

    fn index_path(&self, path: &Path) {
        let Ok(content) = std::fs::read_to_string(path) else {
            return;
        };
        let chunks = markdown_chunks(path, &content);
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let Some(model) = state.model.as_ref() else {
            return;
        };
        let texts: Vec<&str> = chunks.iter().map(|chunk| chunk.text.as_str()).collect();
        let Ok(embeddings) = model.embed(texts, None) else {
            return;
        };
        for (chunk, embedding) in chunks.into_iter().zip(embeddings) {
            let record = Record::new(
                &Vector::from(embedding),
                &Metadata::from(format!("{}\n{}", path.display(), chunk.text)),
            );
            let _ = state.collection.insert(&record);
        }
        let collection = state.collection.clone();
        if let Some(database) = state.database.as_mut() {
            let _ = database.save_collection(COLLECTION_NAME, &collection);
        }
    }

    fn remove_path(&self, _path: &Path) {}
}

impl Default for VectorSearchService {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::agent::tools::vector_search::VectorSearchService for VectorSearchService {
    fn search(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::agent::tools::vector_search::VectorSearchHit>, String> {
        let service = self.clone();
        let query = query.to_string();
        std::thread::spawn(move || service.search_inner(&query, limit))
            .join()
            .map_err(|_| "Vector search worker panicked.".to_string())?
    }
}

impl VectorSearchService {
    fn search_inner(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::agent::tools::vector_search::VectorSearchHit>, String> {
        let state = self
            .state
            .lock()
            .map_err(|_| "Vector state lock poisoned".to_string())?;
        let model = state
            .model
            .as_ref()
            .ok_or_else(|| "Vector search is still initializing.".to_string())?;
        let embedding = model
            .embed(vec![query], None)
            .map_err(|e| e.to_string())?
            .remove(0);
        let results = state
            .collection
            .search(&Vector::from(embedding), limit)
            .map_err(|e| e.message().to_string())?;
        Ok(results
            .into_iter()
            .filter_map(|result| {
                let Metadata::Text(value) = result.data else {
                    return None;
                };
                let mut parts = value.splitn(2, '\n');
                Some(crate::agent::tools::vector_search::VectorSearchHit {
                    path: parts.next().unwrap_or_default().to_string(),
                    text: parts.next().unwrap_or_default().to_string(),
                    distance: result.distance,
                })
            })
            .collect())
    }
}

/// Start the optional vector worker and return its shared tool service.
pub fn start(
    config: AppConfig,
    bus: Bus<FileEvent>,
    tx: std::sync::mpsc::Sender<BackgroundEvent>,
) -> Arc<VectorSearchService> {
    let service = Arc::new(VectorSearchService::new());
    service.start(config, bus.subscribe(), tx);
    service
}

/// Return whether a path is a supported Markdown document.
pub fn is_markdown(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("md" | "markdown")
    )
}

/// Split Markdown into overlapping bounded chunks and hash each chunk.
pub fn markdown_chunks(path: &Path, content: &str) -> Vec<MarkdownChunk> {
    let chars: Vec<char> = content.chars().collect();
    if chars.is_empty() {
        return Vec::new();
    }
    let step = CHUNK_SIZE.saturating_sub(CHUNK_OVERLAP).max(1);
    (0..chars.len())
        .step_by(step)
        .map(|start| {
            let text: String = chars[start..(start + CHUNK_SIZE).min(chars.len())]
                .iter()
                .collect();
            MarkdownChunk {
                path: path.to_path_buf(),
                hash: hash(&text),
                text,
            }
        })
        .collect()
}

fn hash(text: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, text.as_bytes());
    digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn collection_config() -> Config {
    Config {
        distance: Distance::Cosine,
        ..Default::default()
    }
}

fn log_progress(processed: usize, tx: &std::sync::mpsc::Sender<BackgroundEvent>) {
    if processed.is_multiple_of(100) {
        let _ = tx.send(BackgroundLog::progress(processed).into());
    }
}

struct BackgroundLog;
impl BackgroundLog {
    fn progress(processed: usize) -> crate::bus::events::BackgroundLogEntry {
        crate::bus::events::BackgroundLogEntry::new(
            crate::bus::events::LogCategory::Indexer,
            format!("Vector search indexed {processed} Markdown files"),
        )
    }
    fn failed(message: &str) -> crate::bus::events::BackgroundLogEntry {
        crate::bus::events::BackgroundLogEntry::new(
            crate::bus::events::LogCategory::Indexer,
            message.to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_filter_excludes_txt() {
        assert!(is_markdown(Path::new("a.MARKDOWN")));
        assert!(is_markdown(Path::new("a.md")));
        assert!(!is_markdown(Path::new("a.txt")));
    }

    #[test]
    fn chunks_have_stable_hashes_and_overlap() {
        let chunks = markdown_chunks(Path::new("a.md"), &"a".repeat(1400));
        assert!(chunks.len() > 1);
        assert_eq!(
            chunks[0].hash,
            markdown_chunks(Path::new("a.md"), &"a".repeat(1400))[0].hash
        );
        assert_eq!(chunks[0].text[1000..], chunks[1].text[..200]);
    }
}
