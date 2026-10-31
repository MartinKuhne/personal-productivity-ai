//! Tests for [`LogCategory`] and [`BackgroundLogEntry`] (`bus::events::messages`).

use super::*;

#[test]
fn test_log_category_display() {
    assert_eq!(LogCategory::Indexer.to_string(), "Indexer");
    assert_eq!(LogCategory::Watcher.to_string(), "Watcher");
    assert_eq!(LogCategory::PdfConverter.to_string(), "PDF Converter");
    assert_eq!(LogCategory::ImageVision.to_string(), "Image Vision");
    assert_eq!(LogCategory::LlmTools.to_string(), "LLM Tools");
    assert_eq!(LogCategory::Print.to_string(), "Print");
    assert_eq!(LogCategory::Batch.to_string(), "Batch");
}

#[test]
fn test_background_log_entry_new() {
    let entry = BackgroundLogEntry::new(LogCategory::Indexer, "test message".to_string());
    assert_eq!(entry.category, LogCategory::Indexer);
    assert_eq!(entry.message, "test message");
}

#[test]
fn test_log_category_serde_round_trip() {
    for category in [
        LogCategory::Indexer,
        LogCategory::Watcher,
        LogCategory::PdfConverter,
        LogCategory::ImageVision,
        LogCategory::LlmTools,
        LogCategory::Print,
        LogCategory::Batch,
    ] {
        let json = serde_json::to_string(&category).unwrap();
        let back: LogCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(back, category);
    }
}

#[test]
fn test_log_category_unknown_variant_fails_to_deserialize() {
    // An unknown variant name must fail deserialization rather than
    // silently defaulting, so a schema drift is loud.
    let result: Result<LogCategory, _> = serde_json::from_str("\"NotARealCategory\"");
    assert!(result.is_err());
}

#[test]
fn test_background_log_entry_serde_round_trip() {
    let entry = BackgroundLogEntry::new(LogCategory::Watcher, "hello structured log".to_string());
    let json = serde_json::to_string(&entry).unwrap();
    let back: BackgroundLogEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(back.category, entry.category);
    assert_eq!(back.message, entry.message);
    assert_eq!(back.timestamp, entry.timestamp);
}
