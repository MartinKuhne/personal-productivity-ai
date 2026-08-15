//! Unit tests for vector search background indexing and helpers (AGENT-005, AGENT-031).

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

#[test]
fn build_chunk_index_groups_by_vpath_and_hash() {
    let mut collection = Collection::new(&collection_config());
    let rec1 = Record::new(
        &Vector::from(vec![1.0, 0.0]),
        &chunk_metadata("Lib/doc1.md", "model", "hash1", 0, 5),
    );
    let rec2 = Record::new(
        &Vector::from(vec![0.0, 1.0]),
        &chunk_metadata("Lib/doc1.md", "model", "hash2", 5, 5),
    );
    let rec3 = Record::new(
        &Vector::from(vec![0.5, 0.5]),
        &chunk_metadata("Lib/doc2.md", "model", "hash3", 0, 8),
    );

    let ids = collection.insert_many(&[rec1, rec2, rec3]).unwrap();
    let id1 = ids[0];
    let id2 = ids[1];
    let id3 = ids[2];

    let index = build_chunk_index(&collection);
    assert_eq!(index.len(), 2);

    let doc1_chunks = index.get("Lib/doc1.md").unwrap();
    assert_eq!(doc1_chunks.len(), 2);
    assert_eq!(doc1_chunks.get("hash1"), Some(&id1));
    assert_eq!(doc1_chunks.get("hash2"), Some(&id2));

    let doc2_chunks = index.get("Lib/doc2.md").unwrap();
    assert_eq!(doc2_chunks.len(), 1);
    assert_eq!(doc2_chunks.get("hash3"), Some(&id3));

    assert!(!index.contains_key("Lib/missing.md"));
}

#[test]
fn remove_path_cleans_collection_and_chunk_index() {
    let service = VectorSearchService::new();
    let rec = Record::new(
        &Vector::from(vec![1.0, 0.0]),
        &chunk_metadata("doc.md", "model", "h1", 0, 5),
    );
    let id = {
        let mut state = service.state.lock().unwrap();
        let ids = state.collection.insert_many(&[rec]).unwrap();
        let id = ids[0];
        state
            .chunk_index
            .entry("doc.md".to_string())
            .or_default()
            .insert("h1".to_string(), id);
        id
    };

    service.remove_path(Path::new("doc.md"));

    let state = service.state.lock().unwrap();
    assert!(!state.chunk_index.contains_key("doc.md"));
    assert!(state.collection.get(&id).is_err());
}
