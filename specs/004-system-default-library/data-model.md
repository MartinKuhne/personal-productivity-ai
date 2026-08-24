# Data Model: Default System Library & Conversation Logging

## Configuration Types
```rust
// In AppConfig:
pub system_library_name: Option<String>,

// Helpers:
pub fn system_library_display_name(&self) -> &str;
pub fn get_system_library_path() -> PathBuf;
pub fn ensure_system_library_dir() -> std::io::Result<PathBuf>;
pub fn ensure_conversations_dir() -> std::io::Result<PathBuf>;
pub fn get_or_create_system_library(&self) -> ContentLibrary;
```

## Conversation Logger
```rust
pub struct ConversationLogSession {
    pub session_id: uuid::Uuid,
    pub log_path: PathBuf,
    pub turn_count: usize,
}

pub struct WriteToolExecutionRecord {
    pub name: String,
    pub args: serde_json::Value,
    pub result: serde_json::Value,
}
```
