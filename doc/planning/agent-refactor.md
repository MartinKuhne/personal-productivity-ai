# Agent Subsystem Analysis

Here is a comprehensive breakdown of the dependencies, configuration boundaries, and side effects originating from the `src/desktop/src/agent` subsystem:

### 1. External Module Dependencies
The agent module relies on several other bounded subsystems across the crate. (All paths refer to `crate::`):

* **`app::` (Application Domain & VFS)**
  * `app::vfs`: Resolving virtual paths to absolute paths, and handling content library permissions.
  * `app::session::{BrowserSession, PdfBackingTracker}`: Stateful components passed through `Extensions` for the browser automation tools and PDF metadata verification.
  * `app::events::AgentEvent`: UI events emitted by the agent to update the presentation layer.
  * `app::background::{BackgroundLogEntry, LogCategory}`: Used by the tool registry to emit logs to the background worker pool (e.g., during MCP discovery).
* **`bus::` (Messaging & Events)**
  * `bus::core::{Bus, BusReader}`: The core broadcast channels used to fan out events.
  * `bus::events::file::FileEvent`: Emitted by tool executors (via `ToolContext`) when the filesystem changes.
  * `bus::events::debug::AgentDebugEntry`: Used to trace the agent's thought process and tool execution.
  * `bus::events::config::ConfigArrived`: The registry subscribes to this to hot-reload MCP tools.
  * `bus::events::messages::TokenUsageInfo`: Passed back for token budgeting.
* **`config::` (Configuration)**
  * Depends heavily on `AppConfig` to project the domain-specific `AgentConfig`. 
  * References integration configs: `ContentLibrary`, `LlmConfig`, `McpServerConfig`, `ToolGroupsConfig`, `JmapClient`.
* **`integrations::` (External Services)**
  * `integrations::mcp::{McpClients, DynamicToolSource}`: The registry relies on the MCP integration layer to manage the transport, session, and discovery of external MCP tools.
* **`markdown::` and `utils::`**
  * `markdown::Document`: Used by filesystem tools to parse and write front-matter/markdown safely.
  * `utils::tags::extract_tags_from_file`: Used to read tags when the agent creates new files.
  * `utils::uuid::SystemUuidGenerator`: Injectable UUID generator for session IDs.

---

### 2. Configuration (`AgentConfig`)
The agent explicitly does **not** consume the entire `AppConfig` in its main loop (though `ToolContext` holds an `Arc<AppConfig>` for tool executions). The orchestrator projects `AppConfig` into an `AgentConfig`, which slices only what the agent needs:
* **Models**: `models` (Map of available LLMs) and `max_tokens`.
* **Tool Toggles**: `tool_groups` determines which families of built-in tools (web, fs, email, csv_db, etc.) to expose to the LLM.
* **MCP**: `mcp_servers` defines external servers to spin up.
* **Resolved Paths**: `browser` (headless Playwright configuration) and `csv_db_path`.
* **Integration Credentials**: `jmap_clients`, `caldav_clients`, `trello_client`, `searxng_url`.
* **Feature Flags**: E.g., `toolCallDebugMode` (for verbose tool-call logging).

*Note: User identity and system prompts are intentionally excluded from `AgentConfig` because prompt construction happens outside the agent module.*

---

### 3. Side Effects
The agent is designed to be highly sandboxed, but the `ToolExecutor` and its tools intentionally breach the sandbox to enact the agent's decisions. 

**I/O & Network:**
* **LLM Inferences:** `LLMClient` makes outbound HTTP requests to LLM APIs (e.g., OpenAI, Anthropic, Ollama).
* **Integration API Calls:** Built-in tools like `web`, `jmap`, `caldav`, and `trello` make outbound HTTP requests to interact with web servers and APIs.
* **MCP Processes:** The registry spawns persistent external OS processes (via `stdio`) or establishes Server-Sent Events (SSE) connections for MCP tools.

**Filesystem (`vfs` boundary):**
* **Mutations:** Filesystem and CSV tools use `ToolContext::resolve_writable` to verify paths, then use standard `std::fs` / `tokio::fs` calls to read, create, append, modify, or delete files on disk. 
* **Database IO:** The `csv_db` tools rewrite entire CSV files on the disk.

**Browser Automation:**
* The `browser` tools interact with `BrowserSession`, which spins up a headless Chromium instance via Playwright, navigates to external URLs, clicks DOM elements, fills out forms, and evaluates arbitrary JavaScript.

**Internal Event Broadcasting:**
* **Filesystem Notifications:** Whenever a tool modifies a file, `ToolContext` (via `FileEventProducer`) publishes `FileEventKind::Discovered`, `Updated`, or `Removed` to ensure the UI tree, indexing worker, and tabs immediately reflect the change.
* **UI Status Updates:** The agent streams its progress (`AgentEventObserver`) by publishing `AgentStatus` (thinking, streaming text, tool execution) to the UI.
* **Telemetry:** Spams `AgentDebugEntry` telemetry for developer observability and emits `BackgroundLogEntry` events during MCP discovery timeouts or failures.
