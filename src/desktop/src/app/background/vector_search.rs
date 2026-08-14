//! Optional Markdown vector indexing and search worker (AGENT-005, AGENT-031).
//! Chunk vectors are produced by the user-configured `embeddings` model.

use crate::app::background::embeddings::EmbeddingClient;
use crate::bus::core::{Bus, BusReader};
use crate::bus::events::file::{FileEvent, FileEventKind};
use crate::bus::events::typed::BackgroundEvent;
use crate::config::AppConfig;
use sahomedb::prelude::{
    Collection, Config, Database, Distance, Metadata, Record, Vector, VectorID,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const CHUNK_SIZE: usize = 1200;
const CHUNK_OVERLAP: usize = 200;
const COLLECTION_NAME: &str = "markdown";
const VECTOR_SEARCH_FAILED: &str = "Vector search failed. See background logs for details.";

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
    model: Option<Arc<EmbeddingClient>>,
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
            let Some(model) = EmbeddingClient::from_config(&config) else {
                let _ = tx.send(
                    bg_log_failed(
                        "Vector search disabled: no model configured with the 'embeddings' use case.",
                    )
                    .into(),
                );
                return;
            };
            let mut state = service.state.lock().expect("vector state lock poisoned");
            state.model = Some(Arc::new(model));
            if let Some(path) = database_path {
                if let Err(error) = std::fs::create_dir_all(&path) {
                    let _ =
                        tx.send(bg_log_failed(&format!("Vector index directory: {error}")).into());
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
        let model = {
            let Ok(state) = self.state.lock() else {
                return;
            };
            let Some(model) = state.model.as_ref() else {
                return;
            };
            model.clone()
        };
        let texts: Vec<String> = chunks.iter().map(|chunk| chunk.text.clone()).collect();
        let Ok(embeddings) = model.embed(texts) else {
            tracing::error!(name = "vector_search.embed_failed", path = %path.display(), "Embedding failed");
            return;
        };
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let current: HashMap<String, Vec<f32>> = chunks
            .iter()
            .cloned()
            .zip(embeddings)
            .map(|(chunk, embedding)| (chunk.hash.clone(), embedding))
            .collect();
        let existing = records_for_path(&state.collection, path);
        for (hash, id) in &existing {
            if !current.contains_key(hash) {
                let _ = state.collection.delete(id);
            }
        }
        for chunk in chunks {
            if existing.contains_key(&chunk.hash) {
                continue;
            }
            let Some(embedding) = current.get(&chunk.hash) else {
                continue;
            };
            let record = Record::new(
                &Vector::from(embedding),
                &chunk_metadata(path, &chunk.hash, &chunk.text),
            );
            let _ = state.collection.insert(&record);
        }
        let collection = state.collection.clone();
        if let Some(database) = state.database.as_mut() {
            let _ = database.save_collection(COLLECTION_NAME, &collection);
        }
    }

    fn remove_path(&self, path: &Path) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let ids: Vec<VectorID> = records_for_path(&state.collection, path)
            .into_values()
            .collect();
        for id in ids {
            let _ = state.collection.delete(&id);
        }
        let collection = state.collection.clone();
        if let Some(database) = state.database.as_mut() {
            let _ = database.save_collection(COLLECTION_NAME, &collection);
        }
    }
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
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.search_inner(query, limit)
        }))
        .map_err(|_| "Vector search worker panicked.".to_string())?
    }
}

impl VectorSearchService {
    fn search_inner(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<crate::agent::tools::vector_search::VectorSearchHit>, String> {
        let model = {
            let state = self.state.lock().map_err(|_| VECTOR_SEARCH_FAILED)?;
            state
                .model
                .as_ref()
                .ok_or_else(|| {
                    "Vector search is unavailable: no model with the 'embeddings' use case is configured or the worker is not ready.".to_string()
                })?
                .clone()
        };
        let embedding = model
            .embed(vec![query.to_string()])
            .map_err(|error| {
                tracing::error!(name = "vector_search.embed_failed", error = %error, "Embedding query failed");
                VECTOR_SEARCH_FAILED
            })?
            .remove(0);
        let results = {
            let state = self.state.lock().map_err(|_| VECTOR_SEARCH_FAILED)?;
            state
                .collection
                .search(&Vector::from(embedding), limit)
                .map_err(|error| {
                    tracing::error!(
                        name = "vector_search.search_failed",
                        error = error.message(),
                        "Collection search failed"
                    );
                    VECTOR_SEARCH_FAILED
                })?
        };
        Ok(results
            .into_iter()
            .filter_map(|result| {
                let (path, _, text) = chunk_from_metadata(result.data)?;
                Some(crate::agent::tools::vector_search::VectorSearchHit {
                    path,
                    text,
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

/// Encode a chunk as structured collection metadata (path, hash, text).
fn chunk_metadata(path: &Path, hash: &str, text: &str) -> Metadata {
    let mut map = HashMap::new();
    map.insert("path".to_string(), path.to_string_lossy().into_owned());
    map.insert("hash".to_string(), hash.to_string());
    map.insert("text".to_string(), text.to_string());
    Metadata::from(map)
}

/// Decode structured chunk metadata into `(path, hash, text)`.
fn chunk_from_metadata(metadata: Metadata) -> Option<(String, String, String)> {
    let Metadata::Object(map) = metadata else {
        return None;
    };
    let path = match map.get("path") {
        Some(Metadata::Text(path)) => path.clone(),
        _ => return None,
    };
    let hash = match map.get("hash") {
        Some(Metadata::Text(hash)) => hash.clone(),
        _ => return None,
    };
    let text = match map.get("text") {
        Some(Metadata::Text(text)) => text.clone(),
        _ => return None,
    };
    Some((path, hash, text))
}

/// Return the IDs of records belonging to the given path, keyed by chunk hash.
fn records_for_path(collection: &Collection, path: &Path) -> HashMap<String, VectorID> {
    let Ok(records) = collection.list() else {
        return HashMap::new();
    };
    let target = path.to_string_lossy().into_owned();
    records
        .into_iter()
        .filter_map(|(id, record)| {
            let (record_path, hash, _) = chunk_from_metadata(record.data)?;
            (record_path == target).then_some((hash, id))
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
        let _ = tx.send(bg_log_progress(processed).into());
    }
}

fn bg_log_progress(processed: usize) -> crate::bus::events::BackgroundLogEntry {
    crate::bus::events::BackgroundLogEntry::new(
        crate::bus::events::LogCategory::Indexer,
        format!("Vector search indexed {processed} Markdown files"),
    )
}

fn bg_log_failed(message: &str) -> crate::bus::events::BackgroundLogEntry {
    crate::bus::events::BackgroundLogEntry::new(
        crate::bus::events::LogCategory::Indexer,
        message.to_string(),
    )
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

    #[test]
    fn metadata_round_trips_structured_chunks() {
        let metadata = chunk_metadata(Path::new("a.md"), "abc", "body");
        let (path, hash, text) = chunk_from_metadata(metadata).unwrap();
        assert_eq!(path, "a.md");
        assert_eq!(hash, "abc");
        assert_eq!(text, "body");
        assert!(chunk_from_metadata(Metadata::Text("nope".into())).is_none());
    }
}
