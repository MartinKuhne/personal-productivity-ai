//! Optional Markdown vector indexing and search worker (AGENT-005, AGENT-031).
//!
//! Unit tests live in the sibling `vector_search_tests.rs` sidecar.

use crate::app::background::embeddings::EmbeddingClient;
use crate::bus::core::{Bus, BusReader};
use crate::bus::events::file::{FileEvent, FileEventKind};
use crate::bus::events::typed::BackgroundEventSender;
use crate::config::library_display_label;
use crate::config::{AppConfig, ContentLibrary, VirtualPath};
use crate::markdown::parse_front_matter;
use qdrant_client::Qdrant;
use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, CreateFieldIndexCollectionBuilder, DeletePointsBuilder,
    Distance, FieldType, Filter, PointStruct, PointsIdsList, ScrollPointsBuilder,
    SearchPointsBuilder, Value as QdrantValue, VectorParamsBuilder,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use text_splitter::{ChunkConfig, TextSplitter};

const CHUNK_SIZE: usize = 1200;
const CHUNK_OVERLAP: usize = 200;
const DEFAULT_QDRANT_URL: &str = "http://localhost:6334";
const DEFAULT_COLLECTION_NAME: &str = "fastmd_chunks";
const VECTOR_SEARCH_FAILED: &str = "Vector search failed. See background logs for details.";

/// Default maximum cosine distance cutoff when not explicitly overridden by the caller.
pub const DEFAULT_MAX_DISTANCE: f32 = 0.6;

/// Convert a Qdrant cosine similarity score in `[-1.0, 1.0]` to cosine distance in `[0.0, 2.0]`.
pub fn score_to_distance(score: f32) -> f32 {
    (1.0 - score).max(0.0)
}

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
    client: Option<Arc<Qdrant>>,
    collection_name: String,
    content_libraries: Vec<ContentLibrary>,
    chunk_index: HashMap<String, HashSet<String>>,
}

impl VectorSearchService {
    /// Create an empty service; model and client loading is deferred to the background worker.
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(VectorState {
                model: None,
                client: None,
                collection_name: DEFAULT_COLLECTION_NAME.to_string(),
                content_libraries: Vec::new(),
                chunk_index: HashMap::new(),
            })),
        }
    }

    pub(crate) fn start(
        &self,
        config: AppConfig,
        reader: BusReader<FileEvent>,
        tx: BackgroundEventSender,
    ) {
        let service = self.clone();
        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(error) => {
                    let _ = tx
                        .send(bg_log_failed(&format!("Failed to create runtime: {error}")).into());
                    return;
                }
            };

            rt.block_on(async move {
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

                let qdrant_url = config
                    .qdrant_url
                    .clone()
                    .or_else(|| std::env::var("QDRANT_URL").ok())
                    .unwrap_or_else(|| DEFAULT_QDRANT_URL.to_string());

                let collection_name = config
                    .qdrant_collection
                    .clone()
                    .or_else(|| std::env::var("QDRANT_COLLECTION").ok())
                    .unwrap_or_else(|| DEFAULT_COLLECTION_NAME.to_string());

                let client = match build_qdrant_client(&qdrant_url, config.qdrant_api_key.as_deref()) {
                    Ok(client) => Arc::new(client),
                    Err(error) => {
                        tracing::error!(
                            name = "vector_search.qdrant_connect_failed",
                            url = %qdrant_url,
                            error = %error,
                            "Failed to connect to Qdrant vector database"
                        );
                        let _ = tx.send(
                            bg_log_failed(&format!("Failed to connect to Qdrant at {qdrant_url}: {error}")).into(),
                        );
                        return;
                    }
                };

                let model = Arc::new(model);
                if let Err(error) = ensure_collection(&client, &collection_name, &model).await {
                    tracing::error!(
                        name = "vector_search.collection_init_failed",
                        collection = %collection_name,
                        error = %error,
                        "Failed to initialize collection in Qdrant"
                    );
                    let _ = tx.send(
                        bg_log_failed(&format!("Failed to initialize collection {collection_name}: {error}")).into(),
                    );
                    return;
                }

                let chunk_index = match load_chunk_index(&client, &collection_name).await {
                    Ok(index) => index,
                    Err(error) => {
                        tracing::warn!(
                            name = "vector_search.load_index_failed",
                            error = %error,
                            "Failed to load existing chunk index from Qdrant; starting with empty cache"
                        );
                        HashMap::new()
                    }
                };

                {
                    let mut state = service.state.lock().expect("vector state lock poisoned");
                    state.model = Some(model);
                    state.client = Some(client);
                    state.collection_name = collection_name;
                    state.content_libraries = config.content_libraries.clone();
                    state.chunk_index = chunk_index;
                }

                let mut processed = 0usize;
                for library in &config.content_libraries {
                    for entry in walkdir::WalkDir::new(&library.root_folder)
                        .into_iter()
                        .filter_map(Result::ok)
                    {
                        if entry.file_type().is_file() && is_markdown(entry.path()) {
                            service.index_path_async(entry.path()).await;
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
                                    service.index_path_async(&path).await;
                                    processed += 1;
                                    log_progress(processed, &tx);
                                }
                            }
                            FileEventKind::Removed => service.remove_path_async(&path).await,
                            FileEventKind::DirDiscovered | FileEventKind::DirRemoved => {}
                        }
                    }
                }
            });
        });
    }

    async fn index_path_async(&self, path: &Path) {
        let Ok(content) = std::fs::read_to_string(path) else {
            return;
        };
        let body = if let Some(fm) = parse_front_matter(&content) {
            fm.body.strip_prefix('\n').unwrap_or(&fm.body).to_string()
        } else {
            content
        };
        let chunks = markdown_chunks(path, &body);
        if chunks.is_empty() {
            self.remove_path_async(path).await;
            return;
        }

        let (vpath, model, client, collection_name, existing) = {
            let Ok(state) = self.state.lock() else {
                return;
            };
            let (Some(model), Some(client)) = (
                state.model.as_ref().cloned(),
                state.client.as_ref().cloned(),
            ) else {
                return;
            };
            let vpath = library_display_label(&state.content_libraries, path)
                .unwrap_or_else(|| path.to_string_lossy().to_string());
            let existing = state.chunk_index.get(&vpath).cloned().unwrap_or_default();
            (
                vpath,
                model,
                client,
                state.collection_name.clone(),
                existing,
            )
        };

        let unique_hashes: HashSet<&str> = chunks.iter().map(|c| c.hash.as_str()).collect();

        if !existing.is_empty()
            && existing.len() == unique_hashes.len()
            && unique_hashes.iter().all(|hash| existing.contains(*hash))
        {
            tracing::debug!(
                name = "vector_search.file_already_indexed",
                path = %vpath,
                chunk_count = chunks.len(),
                "Virtual file is already indexed"
            );
            return;
        }

        let missing_chunks: Vec<MarkdownChunk> = chunks
            .iter()
            .filter(|chunk| !existing.contains(chunk.hash.as_str()))
            .cloned()
            .collect();

        let embeddings = if missing_chunks.is_empty() {
            Vec::new()
        } else {
            let texts: Vec<String> = missing_chunks.iter().map(|c| c.text.clone()).collect();
            let Ok(embeddings) = model.embed_async(texts).await else {
                tracing::error!(
                    name = "vector_search.embed_failed",
                    path = %path.display(),
                    "Embedding failed"
                );
                return;
            };
            embeddings
        };

        // Evict obsolete chunks
        let obsolete_ids: Vec<qdrant_client::qdrant::PointId> = existing
            .iter()
            .filter(|hash| !unique_hashes.contains(hash.as_str()))
            .map(|hash| chunk_point_id(&vpath, hash).into())
            .collect();

        if !obsolete_ids.is_empty() {
            let _ = client
                .delete_points(
                    DeletePointsBuilder::new(&collection_name).points(PointsIdsList {
                        ids: obsolete_ids.clone(),
                    }),
                )
                .await;

            if let Ok(mut state) = self.state.lock()
                && let Some(file_chunks) = state.chunk_index.get_mut(&vpath)
            {
                for hash in existing
                    .iter()
                    .filter(|h| !unique_hashes.contains(h.as_str()))
                {
                    file_chunks.remove(hash);
                }
            }
        }

        if !missing_chunks.is_empty() {
            let vector_dimension = embeddings.first().map_or(0, Vec::len);
            let mut points = Vec::with_capacity(missing_chunks.len());
            for (chunk, embedding) in missing_chunks.iter().zip(&embeddings) {
                let payload = chunk_payload(
                    &vpath,
                    model.model_name(),
                    &chunk.hash,
                    chunk.offset,
                    chunk.limit,
                );
                let point = PointStruct::new(
                    chunk_point_id(&vpath, &chunk.hash),
                    embedding.clone(),
                    payload,
                );
                points.push(point);
            }

            match client
                .upsert_points(qdrant_client::qdrant::UpsertPointsBuilder::new(
                    &collection_name,
                    points,
                ))
                .await
            {
                Ok(_) => {
                    if let Ok(mut state) = self.state.lock() {
                        let file_chunks = state.chunk_index.entry(vpath.clone()).or_default();
                        for chunk in missing_chunks {
                            file_chunks.insert(chunk.hash);
                        }
                    }
                    tracing::debug!(
                        name = "vector_search.vectors_created",
                        path = %vpath,
                        chunk_count = chunks.len(),
                        vector_dimension,
                        "Created vectors for virtual file"
                    );
                }
                Err(error) => {
                    tracing::error!(
                        name = "vector_search.upsert_failed",
                        path = %vpath,
                        model = model.model_name(),
                        error = %error,
                        "Failed to upsert points to Qdrant"
                    );
                }
            }
        }
    }

    async fn remove_path_async(&self, path: &Path) {
        let (vpath, client, collection_name) = {
            let Ok(mut state) = self.state.lock() else {
                return;
            };
            let vpath = library_display_label(&state.content_libraries, path)
                .unwrap_or_else(|| path.to_string_lossy().to_string());
            let Some(client) = state.client.as_ref().cloned() else {
                state.chunk_index.remove(&vpath);
                return;
            };
            state.chunk_index.remove(&vpath);
            (vpath, client, state.collection_name.clone())
        };

        let filter = Filter::all([Condition::matches("path", vpath)]);
        let _ = client
            .delete_points(DeletePointsBuilder::new(&collection_name).points(filter))
            .await;
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
        let (model, client, collection_name, libraries) = {
            let state = self.state.lock().map_err(|_| VECTOR_SEARCH_FAILED)?;
            let model = state
                .model
                .as_ref()
                .ok_or_else(|| {
                    "Vector search is unavailable: no model with the 'embeddings' use case is configured or the worker is not ready.".to_string()
                })?
                .clone();
            let client = state
                .client
                .as_ref()
                .ok_or_else(|| {
                    "Vector search is unavailable: Qdrant client is not connected.".to_string()
                })?
                .clone();
            (
                model,
                client,
                state.collection_name.clone(),
                state.content_libraries.clone(),
            )
        };

        let search_future = async {
            let embedding = model
                .embed_async(vec![query.to_string()])
                .await
                .map_err(|error| {
                    tracing::error!(name = "vector_search.embed_failed", error = %error, "Embedding query failed");
                    VECTOR_SEARCH_FAILED
                })?
                .remove(0);

            let search_points = SearchPointsBuilder::new(&collection_name, embedding, limit as u64)
                .with_payload(true);
            client.search_points(search_points).await.map_err(|error| {
                tracing::error!(
                    name = "vector_search.search_failed",
                    error = %error,
                    "Qdrant vector search failed"
                );
                VECTOR_SEARCH_FAILED
            })
        };

        let results = match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(search_future)),
            Err(_) => {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|_| VECTOR_SEARCH_FAILED)?;
                rt.block_on(search_future)
            }
        }?;

        let threshold = max_distance.unwrap_or(DEFAULT_MAX_DISTANCE);
        Ok(results
            .result
            .into_iter()
            .filter_map(|point| {
                let distance = score_to_distance(point.score);
                if distance > threshold {
                    return None;
                }
                let (vpath, _, offset, limit) = chunk_from_payload(&point.payload)?;
                let content = resolve_and_read(&vpath, &libraries, offset, limit)?;
                Some(crate::agent::tools::vector_search::VectorSearchHit {
                    path: vpath,
                    distance,
                    offset,
                    limit,
                    content,
                })
            })
            .collect())
    }
}

fn build_qdrant_client(url: &str, api_key: Option<&str>) -> anyhow::Result<Qdrant> {
    let mut builder = Qdrant::from_url(url);
    if let Some(key) = api_key.filter(|k| !k.is_empty()) {
        builder = builder.api_key(key);
    }
    Ok(builder.build()?)
}

async fn ensure_collection(
    client: &Qdrant,
    collection_name: &str,
    model: &EmbeddingClient,
) -> anyhow::Result<()> {
    if !client.collection_exists(collection_name).await? {
        let sample = model
            .embed_async(vec!["fastmd".to_string()])
            .await
            .map_err(anyhow::Error::msg)?;
        let dimension = sample.first().map_or(1536, Vec::len);
        client
            .create_collection(
                CreateCollectionBuilder::new(collection_name)
                    .vectors_config(VectorParamsBuilder::new(dimension as u64, Distance::Cosine)),
            )
            .await?;

        let _ = client
            .create_field_index(CreateFieldIndexCollectionBuilder::new(
                collection_name,
                "path",
                FieldType::Keyword,
            ))
            .await;

        let _ = client
            .create_field_index(CreateFieldIndexCollectionBuilder::new(
                collection_name,
                "hash",
                FieldType::Keyword,
            ))
            .await;
    }
    Ok(())
}

async fn load_chunk_index(
    client: &Qdrant,
    collection_name: &str,
) -> anyhow::Result<HashMap<String, HashSet<String>>> {
    let mut index: HashMap<String, HashSet<String>> = HashMap::new();
    let mut offset = None;

    loop {
        let mut builder = ScrollPointsBuilder::new(collection_name)
            .limit(1000)
            .with_payload(true);
        if let Some(off) = offset {
            builder = builder.offset(off);
        }

        let response = client.scroll(builder).await?;
        for point in response.result {
            if let Some((vpath, hash, _, _)) = chunk_from_payload(&point.payload) {
                index.entry(vpath).or_default().insert(hash);
            }
        }

        if response.next_page_offset.is_none() {
            break;
        }
        offset = response.next_page_offset;
    }

    Ok(index)
}

/// Deterministic point ID generated from virtual path and chunk hash.
pub fn chunk_point_id(vpath: &str, hash: &str) -> String {
    let namespace = uuid::Uuid::NAMESPACE_OID;
    let name = format!("{vpath}:{hash}");
    uuid::Uuid::new_v5(&namespace, name.as_bytes()).to_string()
}

/// Encode a chunk as structured Qdrant payload map.
pub fn chunk_payload(
    vpath: &str,
    model: &str,
    hash: &str,
    offset: usize,
    limit: usize,
) -> HashMap<String, QdrantValue> {
    let mut map = HashMap::new();
    map.insert("path".to_string(), vpath.to_string().into());
    map.insert("kind".to_string(), "note".to_string().into());
    map.insert("model".to_string(), model.to_string().into());
    map.insert("hash".to_string(), hash.to_string().into());
    map.insert("offset".to_string(), (offset as i64).into());
    map.insert("limit".to_string(), (limit as i64).into());
    map
}

/// Decode structured chunk payload into `(path, hash, line_offset, line_limit)`.
pub fn chunk_from_payload(
    payload: &HashMap<String, QdrantValue>,
) -> Option<(String, String, usize, usize)> {
    let path = match payload.get("path")?.kind.as_ref()? {
        qdrant_client::qdrant::value::Kind::StringValue(s) => s.clone(),
        _ => return None,
    };
    let hash = match payload.get("hash")?.kind.as_ref()? {
        qdrant_client::qdrant::value::Kind::StringValue(s) => s.clone(),
        _ => return None,
    };
    let offset = match payload.get("offset")?.kind.as_ref()? {
        qdrant_client::qdrant::value::Kind::IntegerValue(i) => *i as usize,
        _ => return None,
    };
    let limit = match payload.get("limit")?.kind.as_ref()? {
        qdrant_client::qdrant::value::Kind::IntegerValue(i) => *i as usize,
        _ => return None,
    };
    Some((path, hash, offset, limit))
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
    tx: BackgroundEventSender,
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

fn hash(text: &str) -> String {
    let digest = ring::digest::digest(&ring::digest::SHA256, text.as_bytes());
    digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn log_progress(processed: usize, tx: &BackgroundEventSender) {
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
#[path = "vector_search_tests.rs"]
mod tests;
