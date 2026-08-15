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

#[test]
fn test_exact_hash_match_logic_detects_already_indexed() {
    let p1 = "Paragraph one with detailed content to exceed chunk threshold. ".repeat(20);
    let p2 = "Paragraph two with completely different details to form a second chunk. ".repeat(20);
    let text = format!("{p1}\n\n{p2}");
    let chunks = markdown_chunks(Path::new("note.md"), &text);
    assert!(!chunks.is_empty());

    let mut existing = HashMap::new();
    for (i, chunk) in chunks.iter().enumerate() {
        existing.insert(chunk.hash.clone(), VectorID(i as u32));
    }

    let unique_hashes: HashSet<&str> = chunks.iter().map(|c| c.hash.as_str()).collect();

    // Verify exact match criteria
    let is_already_indexed = !existing.is_empty()
        && existing.len() == unique_hashes.len()
        && unique_hashes
            .iter()
            .all(|hash| existing.contains_key(*hash));

    assert!(is_already_indexed);

    // Verify that filtering for missing chunks yields empty list
    let missing_chunks: Vec<&MarkdownChunk> = chunks
        .iter()
        .filter(|chunk| !existing.contains_key(&chunk.hash))
        .collect();
    assert!(missing_chunks.is_empty());
}

#[test]
fn test_duplicate_chunks_in_same_file_handled_correctly() {
    let paragraph = "A".repeat(800);
    let text = format!("{paragraph}\n\n{paragraph}");
    let chunks = markdown_chunks(Path::new("dup.md"), &text);
    assert!(!chunks.is_empty());

    let unique_hashes: HashSet<&str> = chunks.iter().map(|c| c.hash.as_str()).collect();

    let mut existing = HashMap::new();
    for (i, hash) in unique_hashes.iter().enumerate() {
        existing.insert(hash.to_string(), VectorID(i as u32));
    }

    // When indexed, the unique hash matches the single record in existing
    let is_already_indexed = !existing.is_empty()
        && existing.len() == unique_hashes.len()
        && unique_hashes
            .iter()
            .all(|hash| existing.contains_key(*hash));

    assert!(is_already_indexed);
}

#[test]
fn test_partial_chunk_modifications_identifies_missing_and_obsolete() {
    let mut existing = HashMap::new();
    existing.insert("hash_a".to_string(), VectorID(10));
    existing.insert("hash_b".to_string(), VectorID(20));
    existing.insert("hash_c".to_string(), VectorID(30)); // will become obsolete

    // New version of the file has hash_a, hash_b, and new hash_d
    let chunks = [
        MarkdownChunk {
            path: PathBuf::from("doc.md"),
            text: "A".into(),
            hash: "hash_a".into(),
            offset: 0,
            limit: 1,
        },
        MarkdownChunk {
            path: PathBuf::from("doc.md"),
            text: "B".into(),
            hash: "hash_b".into(),
            offset: 1,
            limit: 1,
        },
        MarkdownChunk {
            path: PathBuf::from("doc.md"),
            text: "D".into(),
            hash: "hash_d".into(),
            offset: 2,
            limit: 1,
        },
    ];

    let unique_hashes: HashSet<&str> = chunks.iter().map(|c| c.hash.as_str()).collect();

    // 1. Should NOT match as already indexed
    let is_already_indexed = !existing.is_empty()
        && existing.len() == unique_hashes.len()
        && unique_hashes
            .iter()
            .all(|hash| existing.contains_key(*hash));
    assert!(!is_already_indexed);

    // 2. Missing chunks should be only "hash_d"
    let missing_chunks: Vec<&MarkdownChunk> = chunks
        .iter()
        .filter(|chunk| !existing.contains_key(&chunk.hash))
        .collect();
    assert_eq!(missing_chunks.len(), 1);
    assert_eq!(missing_chunks[0].hash, "hash_d");

    // 3. Obsolete chunks should be only "hash_c"
    let obsolete_hashes: Vec<&str> = existing
        .keys()
        .filter(|h| !unique_hashes.contains(h.as_str()))
        .map(|s| s.as_str())
        .collect();
    assert_eq!(obsolete_hashes.len(), 1);
    assert_eq!(obsolete_hashes[0], "hash_c");
}

#[test]
fn test_front_matter_change_preserves_body_chunk_hashes() {
    let file1 = "---\ntitle: Old Title\ntags: [a]\n---\nBody paragraph one.\n\nBody paragraph two.";
    let file2 = "---\ntitle: New Title\ntags: [b, c]\nauthor: Alice\n---\nBody paragraph one.\n\nBody paragraph two.";

    let fm1 = parse_front_matter(file1).unwrap();
    let fm2 = parse_front_matter(file2).unwrap();

    let body1 = fm1.body.strip_prefix('\n').unwrap_or(&fm1.body);
    let body2 = fm2.body.strip_prefix('\n').unwrap_or(&fm2.body);

    let chunks1 = markdown_chunks(Path::new("file.md"), body1);
    let chunks2 = markdown_chunks(Path::new("file.md"), body2);

    assert_eq!(chunks1.len(), chunks2.len());
    for (c1, c2) in chunks1.iter().zip(chunks2.iter()) {
        assert_eq!(c1.hash, c2.hash);
        assert_eq!(c1.text, c2.text);
    }
}

#[test]
fn test_multiple_vpaths_isolated_in_chunk_index() {
    let service = VectorSearchService::new();
    let rec_a = Record::new(
        &Vector::from(vec![1.0, 0.0]),
        &chunk_metadata("LibA/doc.md", "model", "common_hash", 0, 5),
    );
    let rec_b = Record::new(
        &Vector::from(vec![1.0, 0.0]),
        &chunk_metadata("LibB/doc.md", "model", "common_hash", 0, 5),
    );

    let (id_a, id_b) = {
        let mut state = service.state.lock().unwrap();
        let ids = state.collection.insert_many(&[rec_a, rec_b]).unwrap();
        state
            .chunk_index
            .entry("LibA/doc.md".to_string())
            .or_default()
            .insert("common_hash".to_string(), ids[0]);
        state
            .chunk_index
            .entry("LibB/doc.md".to_string())
            .or_default()
            .insert("common_hash".to_string(), ids[1]);
        (ids[0], ids[1])
    };

    // Remove LibA/doc.md
    service.remove_path(Path::new("LibA/doc.md"));

    let state = service.state.lock().unwrap();
    assert!(!state.chunk_index.contains_key("LibA/doc.md"));
    assert!(state.collection.get(&id_a).is_err());

    // LibB/doc.md MUST remain intact
    assert!(state.chunk_index.contains_key("LibB/doc.md"));
    assert_eq!(
        state.chunk_index["LibB/doc.md"].get("common_hash"),
        Some(&id_b)
    );
    assert!(state.collection.get(&id_b).is_ok());
}
