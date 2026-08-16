//! Unit tests for vector search background indexing and helpers (AGENT-005, AGENT-031).

use super::*;

#[test]
fn markdown_filter_excludes_txt() {
    assert!(is_markdown(Path::new("a.MARKDOWN")));
    assert!(is_markdown(Path::new("a.md")));
    assert!(!is_markdown(Path::new("a.txt")));
}

#[test]
fn payload_round_trips_structured_chunks() {
    let payload = chunk_payload("Library/a.md", "embed-model", "abc", 10, 50);
    let (path, hash, offset, limit) = chunk_from_payload(&payload).unwrap();
    assert_eq!(path, "Library/a.md");
    assert_eq!(hash, "abc");
    assert_eq!(offset, 10);
    assert_eq!(limit, 50);

    let empty_payload = HashMap::new();
    assert!(chunk_from_payload(&empty_payload).is_none());
}

#[test]
fn chunk_point_id_is_deterministic() {
    let id1 = chunk_point_id("Notes/doc.md", "hash123");
    let id2 = chunk_point_id("Notes/doc.md", "hash123");
    let id3 = chunk_point_id("Notes/doc.md", "hash456");
    let id4 = chunk_point_id("Other/doc.md", "hash123");

    assert_eq!(id1, id2);
    assert_ne!(id1, id3);
    assert_ne!(id1, id4);
    assert!(uuid::Uuid::parse_str(&id1).is_ok());
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
fn test_score_to_distance_conversion() {
    assert!((score_to_distance(1.0) - 0.0).abs() < 1e-6);
    assert!((score_to_distance(0.8) - 0.2).abs() < 1e-6);
    assert!((score_to_distance(0.4) - 0.6).abs() < 1e-6);
    assert!((score_to_distance(0.0) - 1.0).abs() < 1e-6);
    assert!((score_to_distance(-0.5) - 1.5).abs() < 1e-6);
    assert_eq!(score_to_distance(1.05), 0.0); // Clamped at 0.0
}

#[test]
fn test_default_max_distance_filtering_logic() {
    assert_eq!(DEFAULT_MAX_DISTANCE, 0.6);

    let threshold = DEFAULT_MAX_DISTANCE;

    // Simulated Qdrant cosine similarity scores:
    let high_relevance_score = 0.75; // distance = 0.25 <= 0.6 (Pass)
    let exact_cutoff_score = 0.40; // distance = 0.60 <= 0.6 (Pass)
    let low_relevance_score = 0.35; // distance = 0.65 > 0.6 (Filtered out)
    let very_low_score = 0.10; // distance = 0.90 > 0.6 (Filtered out)

    assert!(score_to_distance(high_relevance_score) <= threshold);
    assert!(score_to_distance(exact_cutoff_score) <= threshold);
    assert!(score_to_distance(low_relevance_score) > threshold);
    assert!(score_to_distance(very_low_score) > threshold);
}

#[test]
fn test_custom_max_distance_overrides_default() {
    let scores = [0.85, 0.60, 0.35, 0.15];

    // With default threshold (0.6): distances <= 0.6 (scores >= 0.40)
    let default_passes: Vec<f32> = scores
        .iter()
        .copied()
        .filter(|&s| score_to_distance(s) <= DEFAULT_MAX_DISTANCE)
        .collect();
    assert_eq!(default_passes, vec![0.85, 0.60]);

    // With strict custom threshold (0.3): distances <= 0.3 (scores >= 0.70)
    let strict_threshold = 0.3;
    let strict_passes: Vec<f32> = scores
        .iter()
        .copied()
        .filter(|&s| score_to_distance(s) <= strict_threshold)
        .collect();
    assert_eq!(strict_passes, vec![0.85]);

    // With relaxed custom threshold (0.8): distances <= 0.8 (scores >= 0.20)
    let relaxed_threshold = 0.8;
    let relaxed_passes: Vec<f32> = scores
        .iter()
        .copied()
        .filter(|&s| score_to_distance(s) <= relaxed_threshold)
        .collect();
    assert_eq!(relaxed_passes, vec![0.85, 0.60, 0.35]);
}

#[test]
fn test_exact_hash_match_logic_detects_already_indexed() {
    let p1 = "Paragraph one with detailed content to exceed chunk threshold. ".repeat(20);
    let p2 = "Paragraph two with completely different details to form a second chunk. ".repeat(20);
    let text = format!("{p1}\n\n{p2}");
    let chunks = markdown_chunks(Path::new("note.md"), &text);
    assert!(!chunks.is_empty());

    let mut existing: HashSet<String> = HashSet::new();
    for chunk in &chunks {
        existing.insert(chunk.hash.clone());
    }

    let unique_hashes: HashSet<&str> = chunks.iter().map(|c| c.hash.as_str()).collect();

    // Verify exact match criteria
    let is_already_indexed = !existing.is_empty()
        && existing.len() == unique_hashes.len()
        && unique_hashes.iter().all(|hash| existing.contains(*hash));

    assert!(is_already_indexed);

    // Verify that filtering for missing chunks yields empty list
    let missing_chunks: Vec<&MarkdownChunk> = chunks
        .iter()
        .filter(|chunk| !existing.contains(chunk.hash.as_str()))
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

    let mut existing: HashSet<String> = HashSet::new();
    for hash in &unique_hashes {
        existing.insert(hash.to_string());
    }

    // When indexed, the unique hash matches the single record in existing
    let is_already_indexed = !existing.is_empty()
        && existing.len() == unique_hashes.len()
        && unique_hashes.iter().all(|hash| existing.contains(*hash));

    assert!(is_already_indexed);
}

#[test]
fn test_partial_chunk_modifications_identifies_missing_and_obsolete() {
    let mut existing: HashSet<String> = HashSet::new();
    existing.insert("hash_a".to_string());
    existing.insert("hash_b".to_string());
    existing.insert("hash_c".to_string()); // will become obsolete

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
        && unique_hashes.iter().all(|hash| existing.contains(*hash));
    assert!(!is_already_indexed);

    // 2. Missing chunks should be only "hash_d"
    let missing_chunks: Vec<&MarkdownChunk> = chunks
        .iter()
        .filter(|chunk| !existing.contains(chunk.hash.as_str()))
        .collect();
    assert_eq!(missing_chunks.len(), 1);
    assert_eq!(missing_chunks[0].hash, "hash_d");

    // 3. Obsolete chunks should be only "hash_c"
    let obsolete_hashes: Vec<&str> = existing
        .iter()
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
    {
        let mut state = service.state.lock().unwrap();
        state
            .chunk_index
            .entry("LibA/doc.md".to_string())
            .or_default()
            .insert("common_hash".to_string());
        state
            .chunk_index
            .entry("LibB/doc.md".to_string())
            .or_default()
            .insert("common_hash".to_string());
    }

    {
        let mut state = service.state.lock().unwrap();
        state.chunk_index.remove("LibA/doc.md");
    }

    let state = service.state.lock().unwrap();
    assert!(!state.chunk_index.contains_key("LibA/doc.md"));
    assert!(state.chunk_index.contains_key("LibB/doc.md"));
    assert!(state.chunk_index["LibB/doc.md"].contains("common_hash"));
}
