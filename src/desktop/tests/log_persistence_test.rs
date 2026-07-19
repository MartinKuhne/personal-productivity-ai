use fastmd::background::models::{BackgroundLogEntry, LogCategory};
use fastmd::background::manager::BackgroundProcessManager;
use tempfile::tempdir;

#[test]
fn test_log_persistence() {
    let mut manager = BackgroundProcessManager::new();
    
    manager.push_log(BackgroundLogEntry::new(LogCategory::Indexer, "Test indexer log".to_string()));
    manager.push_log(BackgroundLogEntry::new(LogCategory::Watcher, "Test watcher log".to_string()));
    
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("logs").join("background-process.log");
    
    // Save
    manager.save_logs(&log_path).unwrap();
    
    // Read and verify
    let content = std::fs::read_to_string(&log_path).unwrap();
    assert!(content.contains("[Indexer] Test indexer log"));
    assert!(content.contains("[Watcher] Test watcher log"));
}
