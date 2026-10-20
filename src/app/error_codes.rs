//! Standardized error codes for structured logging (NFR-004).
//!
//! Every `ERROR` and `FATAL` level log MUST include an error code from this module
//! to facilitate automated grouping, filtering, and alerting.
//!
//! Format: `SUBSYSTEM-NNNN` where subsystem is 3-4 chars and NNNN is a 4-digit number.

/// Agent subsystem error codes (AGENT-1xxx)
pub mod agent {
    /// Agent driver channel closed unexpectedly; prompt dropped
    pub const CHANNEL_CLOSED: &str = "AGENT-1001";
    /// Agent launch failed (thread spawn or initial send failed)
    pub const LAUNCH_FAILED: &str = "AGENT-1002";
    /// Agent session cancelled by user
    pub const CANCELLED: &str = "AGENT-1003";
    /// LLM client error (API, network, auth)
    pub const LLM_CLIENT_ERROR: &str = "AGENT-1004";
    /// Tool execution failed in agent context
    pub const TOOL_EXECUTION_FAILED: &str = "AGENT-1005";
}

/// Tool execution subsystem error codes (TOOL-2xxx)
pub mod tool {
    /// Tool not found in registry
    pub const NOT_FOUND: &str = "TOOL-2001";
    /// Tool execution failed (returned error status)
    pub const EXECUTION_FAILED: &str = "TOOL-2002";
    /// Tool call policy denied the call
    pub const POLICY_DENIED: &str = "TOOL-2003";
    /// Tool arguments invalid or missing
    pub const INVALID_ARGUMENTS: &str = "TOOL-2004";
    /// Tool timeout
    pub const TIMEOUT: &str = "TOOL-2005";
}

/// File system / workspace error codes (FS-3xxx)
pub mod fs {
    /// File watcher failed to start or encountered error
    pub const WATCHER_FAILED: &str = "FS-3001";
    /// File read failed
    pub const READ_FAILED: &str = "FS-3002";
    /// File write failed
    pub const WRITE_FAILED: &str = "FS-3003";
    /// File delete failed
    pub const DELETE_FAILED: &str = "FS-3004";
    /// Directory scan failed
    pub const SCAN_FAILED: &str = "FS-3005";
    /// PDF conversion failed
    pub const PDF_CONVERSION_FAILED: &str = "FS-3006";
    /// Image processing failed
    pub const IMAGE_PROCESSING_FAILED: &str = "FS-3007";
}

/// Configuration subsystem error codes (CFG-4xxx)
pub mod config {
    /// Configuration load failed
    pub const LOAD_FAILED: &str = "CFG-4001";
    /// Configuration save failed
    pub const SAVE_FAILED: &str = "CFG-4002";
    /// Configuration validation failed
    pub const VALIDATION_FAILED: &str = "CFG-4003";
    /// Secrets load failed
    pub const SECRETS_LOAD_FAILED: &str = "CFG-4004";
}

/// Indexer / search subsystem error codes (IDX-5xxx)
pub mod indexer {
    /// Index build failed
    pub const BUILD_FAILED: &str = "IDX-5001";
    /// Vector search failed
    pub const VECTOR_SEARCH_FAILED: &str = "IDX-5002";
    /// Embedding generation failed
    pub const EMBEDDING_FAILED: &str = "IDX-5003";
}

/// Background worker error codes (BG-6xxx)
pub mod background {
    /// Background task spawn failed
    pub const TASK_SPAWN_FAILED: &str = "BG-6001";
    /// Worker thread panic
    pub const WORKER_PANIC: &str = "BG-6002";
    /// Background channel closed unexpectedly
    pub const CHANNEL_CLOSED: &str = "BG-6003";
}

/// UI / rendering error codes (UI-7xxx)
pub mod ui {
    /// Font registration failed
    pub const FONT_LOAD_FAILED: &str = "UI-7001";
    /// Theme loading failed
    pub const THEME_LOAD_FAILED: &str = "UI-7002";
    /// View model rebuild failed
    pub const VIEW_MODEL_REBUILD_FAILED: &str = "UI-7003";
}

/// MCP integration error codes (MCP-8xxx)
pub mod mcp {
    /// MCP server connection failed
    pub const CONNECTION_FAILED: &str = "MCP-8001";
    /// MCP authentication failed
    pub const AUTH_FAILED: &str = "MCP-8002";
    /// MCP tool call failed
    pub const TOOL_CALL_FAILED: &str = "MCP-8003";
}

/// Discord integration error codes (DISC-9xxx)
pub mod discord {
    /// Discord gateway connection failed
    pub const GATEWAY_CONNECTION_FAILED: &str = "DISC-9001";
    /// Discord command handling failed
    pub const COMMAND_FAILED: &str = "DISC-9002";
    /// Discord rate limit exceeded
    pub const RATE_LIMIT_EXCEEDED: &str = "DISC-9003";
}
