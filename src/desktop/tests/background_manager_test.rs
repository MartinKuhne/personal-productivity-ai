use fastmd::background::models::{BackgroundLogEntry, LogCategory};
use fastmd::background::manager::{BackgroundProcessManager, MAX_LOG_ENTRIES};

#[test]
fn test_background_manager_push_limit() {
    let mut manager = BackgroundProcessManager::new();
    
    // Add MAX_LOG_ENTRIES + 10 entries
    for i in 0..(MAX_LOG_ENTRIES + 10) {
        manager.push_log(BackgroundLogEntry::new(LogCategory::Indexer, format!("Log {}", i)));
    }
    
    let logs = manager.get_logs();
    assert_eq!(logs.len(), MAX_LOG_ENTRIES);
    
    // The first entry should be "Log 10" because 0..9 were evicted
    assert_eq!(logs.front().unwrap().message, "Log 10");
    assert_eq!(logs.back().unwrap().message, format!("Log {}", MAX_LOG_ENTRIES + 9));
}
