use std::path::PathBuf;
use notify::RecommendedWatcher;
use serde_json::Value;

#[derive(Debug)]
pub enum BackgroundMessage {
    FileParsed { path: PathBuf, tags: Vec<String> },
    DirParsed { path: PathBuf },
    Finished(RecommendedWatcher),
    FinishedWithoutWatcher,
    FileModified { path: PathBuf, tags: Vec<String> },
    FileDeleted { path: PathBuf },

    AgentStatus(String),
    AgentThinking(String),
    AgentResponse(String),
    AgentFinished(Vec<Value>),
    AgentFailed(String),
    
    LogEntry(crate::background::BackgroundLogEntry),
}