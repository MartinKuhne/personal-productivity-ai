//! Optional Markdown vector indexing and search worker (AGENT-005, AGENT-031).
//! Chunk vectors are produced by the user-configured `embeddings` model.

use crate::app::background::embeddings::EmbeddingClient;
use crate::bus::core::{Bus, BusReader};
use crate::bus::events::file::{FileEvent, FileEventKind};
use crate::bus::events::typed::BackgroundEvent;
use crate::config::library_display_label;
use crate::config::{get_config_path, AppConfig, ContentLibrary, VirtualPath};
use crate::markdown::parse_front_matter;
use sahomedb::prelude::{
    Collection, Config, Database, Distance, Metadata, Record, Vector, VectorID,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use text_splitter::{ChunkConfig, TextSplitter};

const CHUNK_SIZE: usize = 1200;
const CHUNK_OVERLAP: usize = 200;
// SahomeDB serializes the whole collection on each save; batching avoids
// excessive sled blob growth during the initial scan and update bursts.
const SAVE_INTERVAL: usize = 100;
const COLLECTION_NAME: &str = "markdown";
const VECTOR_SEARCH_FAILED: &str = "Vector search failed. See background logs for details.";

/// A chunk of Markdown content with its stable content hash, line offset and limit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownChunk {
    /// Source path.
    pub path: PathBuf,
    /// Chunk body.
    pub text: String,
    /// Stable hash of the chunk body.
    pub hash: String,
    /// 0-indexed line offset of this chunk within the note body (after YAML front matter).
    pub offset: usize,
    /// Number of lines in this chunk.
    pub limit: usize,
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
    content_libraries: Vec<ContentLibrary>,
}

impl VectorSearchService {
    /// Create an empty service; model loading is deferred to the background worker.
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(VectorState {
                model: None,
                collection: Collection::new(&collection_config()),
                database: None,
                content_libraries: Vec::new(),
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
            let database_path = Some(
                get_config_path()
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".fastmd-vector-index"),
            );
            let Some(model) = EmbeddingClient::from_config(&config) else {
                tracing::warn!(
                    name = "vector_search.no_embeddings_model",
                    "Vector search feature is enabled but no model is configured with the 'embeddings' use case; vector indexing is disabled."
                );
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
            state.content_libraries = config.content_libraries.clone();
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
                        if should_save(processed) {
                            service.save_collection();
                        }
                    }
                }
            }
            service.save_collection();
            while let Ok(event) = reader.recv() {
                for path in event.paths {
                    match event.kind {
                        FileEventKind::Discovered | FileEventKind::Updated => {
                            if is_markdown(&path) {
                                service.index_path(&path);
                                processed += 1;
                                log_progress(processed, &tx);
                                if should_save(processed) {
                                    service.save_collection();
                                }
                            }
                        }
                        FileEventKind::Removed => service.remove_path(&path),
                        FileEventKind::DirDiscovered | FileEventKind::DirRemoved => {}
                    }
                }
            }
            service.save_collection();
        });
    }

    fn index_path(&self, path: &Path) {
        let Ok(content) = std::fs::read_to_string(path) else {
            return;
        };
        let body = if let Some(fm) = parse_front_matter(&content) {
            fm.body.strip_prefix('\n').unwrap_or(&fm.body).to_string()
        } else {
            content
        };
        let chunks = markdown_chunks(path, &body);
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
        let vpath = library_display_label(&state.content_libraries, path)
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        let current: HashMap<String, Vec<f32>> = chunks
            .iter()
            .cloned()
            .zip(embeddings)
            .map(|(chunk, embedding)| (chunk.hash.clone(), embedding))
            .collect();
        let existing = records_for_path(&state.collection, &vpath);
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
                &chunk_metadata(
                    &vpath,
                    model.model_name(),
                    &chunk.hash,
                    chunk.offset,
                    chunk.limit,
                ),
            );
            let _ = state.collection.insert(&record);
        }
    }

    fn save_collection(&self) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let collection = state.collection.clone();
        if let Some(database) = state.database.as_mut() {
            let _ = database.save_collection(COLLECTION_NAME, &collection);
        }
    }

    fn remove_path(&self, path: &Path) {
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        let vpath = library_display_label(&state.content_libraries, path)
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        let ids: Vec<VectorID> = records_for_path(&state.collection, &vpath)
            .into_values()
            .collect();
        for id in ids {
            let _ = state.collection.delete(&id);
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
        max_distance: Option<f32>,
    ) -> Result<Vec<crate::agent::tools::vector_search::VectorSearchHit>, String> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.search_inner(query, limit, max_distance)
        }))
        .map_err(|_| "Vector search worker panicked.".to_string())?
    }
}

impl VectorSearchService {
    fn search_inner(
        &self,
        query: &str,
        limit: usize,
        max_distance: Option<f32>,
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
        let (results, libraries) = {
            let state = self.state.lock().map_err(|_| VECTOR_SEARCH_FAILED)?;
            let results = state
                .collection
                .search(&Vector::from(embedding), limit)
                .map_err(|error| {
                    tracing::error!(
                        name = "vector_search.search_failed",
                        error = error.message(),
                        "Collection search failed"
                    );
                    VECTOR_SEARCH_FAILED
                })?;
            (results, state.content_libraries.clone())
        };
        let threshold = max_distance.unwrap_or(0.6);
        Ok(results
            .into_iter()
            .filter(|result| result.distance <= threshold)
            .filter_map(|result| {
                let (vpath, _, offset, limit) = chunk_from_metadata(result.data)?;
                let content = resolve_and_read(&vpath, &libraries, offset, limit)?;
                Some(crate::agent::tools::vector_search::VectorSearchHit {
                    path: vpath,
                    distance: result.distance,
                    offset,
                    limit,
                    content,
                })
            })
            .collect())
    }
}

/// Resolve a virtual path to a physical path, read the source file,
/// strip YAML front matter, and extract lines at the given 0-indexed
/// line offset/limit within the body. Returns `None` when the offset
/// is past the end of the body.
fn resolve_and_read(
    vpath: &str,
    libraries: &[ContentLibrary],
    line_offset: usize,
    line_limit: usize,
) -> Option<String> {
    let vp = VirtualPath::parse(vpath).ok()?;
    let physical = vp.resolve(libraries).ok()?;
    let content = std::fs::read_to_string(&physical).ok()?;
    let body = if let Some(fm) = parse_front_matter(&content) {
        fm.body.strip_prefix('\n').unwrap_or(&fm.body).to_string()
    } else {
        content
    };
    let lines: Vec<&str> = body.lines().collect();
    if line_offset >= lines.len() {
        return None;
    }
    let end = (line_offset + line_limit).min(lines.len());
    Some(lines[line_offset..end].join("\n"))
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

/// Split Markdown body text into semantically-bounded chunks using text-splitter.
/// `content` should be the note body (YAML front matter already stripped).
pub fn markdown_chunks(path: &Path, content: &str) -> Vec<MarkdownChunk> {
    let config = ChunkConfig::new(CHUNK_SIZE)
        .with_overlap(CHUNK_OVERLAP)
        .unwrap();
    let splitter = TextSplitter::new(config);
    splitter
        .chunk_indices(content)
        .map(|(byte_offset, text)| {
            let line_offset = content[..byte_offset]
                .chars()
                .filter(|&c| c == '\n')
                .count();
            let line_limit = text.lines().count();
            MarkdownChunk {
                path: path.to_path_buf(),
                hash: hash(text),
                offset: line_offset,
                limit: line_limit,
                text: text.to_string(),
            }
        })
        .collect()
}

/// Encode a chunk as structured collection metadata (virtual path, kind, model, hash, line offset, line limit).
fn chunk_metadata(vpath: &str, model: &str, hash: &str, offset: usize, limit: usize) -> Metadata {
    let mut map = HashMap::new();
    map.insert("path".to_string(), vpath.to_string());
    map.insert("kind".to_string(), "note".to_string());
    map.insert("model".to_string(), model.to_string());
    map.insert("hash".to_string(), hash.to_string());
    map.insert("offset".to_string(), offset.to_string());
    map.insert("limit".to_string(), limit.to_string());
    Metadata::from(map)
}

/// Decode structured chunk metadata into `(path, hash, line_offset, line_limit)`.
fn chunk_from_metadata(metadata: Metadata) -> Option<(String, String, usize, usize)> {
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
    let offset = match map.get("offset") {
        Some(Metadata::Text(s)) => s.parse().ok()?,
        _ => return None,
    };
    let limit = match map.get("limit") {
        Some(Metadata::Text(s)) => s.parse().ok()?,
        _ => return None,
    };
    Some((path, hash, offset, limit))
}

/// Return the IDs of records belonging to the given virtual path, keyed by chunk hash.
fn records_for_path(collection: &Collection, vpath: &str) -> HashMap<String, VectorID> {
    let Ok(records) = collection.list() else {
        return HashMap::new();
    };
    records
        .into_iter()
        .filter_map(|(id, record)| {
            let (record_path, hash, _, _) = chunk_from_metadata(record.data)?;
            (record_path == vpath).then_some((hash, id))
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

fn should_save(processed: usize) -> bool {
    processed.is_multiple_of(SAVE_INTERVAL)
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
    fn metadata_round_trips_structured_chunks() {
        let metadata = chunk_metadata("Library/a.md", "embed-model", "abc", 10, 50);
        let (path, hash, offset, limit) = chunk_from_metadata(metadata).unwrap();
        assert_eq!(path, "Library/a.md");
        assert_eq!(hash, "abc");
        assert_eq!(offset, 10);
        assert_eq!(limit, 50);
        assert!(chunk_from_metadata(Metadata::Text("nope".into())).is_none());
    }

    #[test]
    fn chunks_use_semantic_boundaries() {
        let text = "First paragraph.\n\nSecond sentence here.\n\nThird paragraph content with more words.\n\nFourth paragraph ending now.";
        let chunks = markdown_chunks(Path::new("a.md"), text);
        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert!(!chunk.text.is_empty());
            assert!(chunk.limit > 0);
            let line_count = chunk.text.lines().count();
            assert_eq!(chunk.limit, line_count);
        }
    }

    #[test]
    fn chunks_are_deterministic() {
        let text = "Some content\n\nSecond paragraph\n\nThird paragraph\n\nFourth paragraph\n\nFifth paragraph.";
        let a = markdown_chunks(Path::new("a.md"), text);
        let b = markdown_chunks(Path::new("a.md"), text);
        assert_eq!(a.len(), b.len());
        for (ca, cb) in a.iter().zip(b.iter()) {
            assert_eq!(ca.hash, cb.hash);
            assert_eq!(ca.offset, cb.offset);
            assert_eq!(ca.limit, cb.limit);
        }
    }

    #[test]
    fn saves_every_save_interval() {
        assert!(!should_save(SAVE_INTERVAL - 1));
        assert!(should_save(SAVE_INTERVAL));
        assert!(!should_save(SAVE_INTERVAL + 1));
    }
}
