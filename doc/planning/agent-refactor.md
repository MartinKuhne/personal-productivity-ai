# Agent Subsystem Analysis

Here is a comprehensive breakdown of the dependencies, configuration boundaries, and side effects originating from the `src/desktop/src/agent` subsystem:

### 1. External Module Dependencies
The agent module relies on several other bounded subsystems across the crate. (All paths refer to `crate::`):

* **`app::` (Application Domain)**
  * `app::session::{BrowserSession, PdfBackingTracker}`: Stateful components passed through `Extensions` for the browser automation tools and PDF metadata verification.
  * `app::events::AgentEvent`: UI events emitted by the agent to update the presentation layer.
  * `app::background::{BackgroundLogEntry, LogCategory}`: Used by the registry to emit logs to the background worker pool (e.g., during MCP discovery).
  * *Note: Filesystem operations are decoupled. Tools now rely on the `agent::tools::vfs::VirtualFileSystem` trait, which the app implements via `VfsResolver`.*
* **`bus::` (Messaging & Events)**
  * `bus::core::{Bus, BusReader}`: The core broadcast channels used to fan out events.
  * `bus::events::debug::AgentDebugEntry`: Used to trace the agent's thought process and tool execution.
  * `bus::events::config::ConfigArrived`: The registry subscribes to this to hot-reload MCP tools.
  * `bus::events::messages::TokenUsageInfo`: Passed back for token budgeting.
  * *Note: File modification events (`FileEvent`) are no longer directly emitted by tools. Tools notify via the `agent::tools::observer::OnFileChanged` trait, which the app implements via `AppFileObserver`.*
* **`config::` (Configuration)**
  * Depends on `AgentConfig` (which the orchestrator slices from `AppConfig`). `ToolContext` natively holds `AgentConfig` instead of the global `AppConfig`.
  * References integration configs: `ContentLibrary`, `LlmConfig`, `McpServerConfig`, `ToolGroupsConfig`, `JmapClient`.
* **`integrations::` (External Services)**
  * `integrations::mcp::{McpClients, DynamicToolSource}`: The registry relies on the MCP integration layer to manage the transport, session, and discovery of external MCP tools.
* **`markdown::` and `utils::`**
  * `markdown::Document`: Used by filesystem tools to parse and write front-matter/markdown safely.
  * `utils::tags::extract_tags_from_file`: Used to read tags when the agent creates new files.
  * `utils::uuid::SystemUuidGenerator`: Injectable UUID generator for session IDs.

---

### 2. Configuration (`AgentConfig`)
The agent explicitly does **not** consume the entire `AppConfig` in its main loop. `ToolContext` uses `AgentConfig`, which slices only what the agent needs:
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

**Filesystem (`VirtualFileSystem` boundary):**
* **Mutations:** Filesystem and CSV tools use `ToolContext::vfs()` to verify paths and perform read, create, append, modify, or delete operations. The tools themselves no longer use `std::fs` or `tokio::fs` directly.
* **Database IO:** The `csv_db` tools rewrite entire CSV files via the injected VFS facade.

**Tool Call Policy:**
* Tool executions are gated by an injected `agent::tools::policy::ToolCallPolicy`, which evaluates if specific tools (and their arguments) are permitted to run in the current context (e.g., "can write to file" authorization).

**Browser Automation:**
* The `browser` tools interact with `BrowserSession`, which spins up a headless Chromium instance via Playwright, navigates to external URLs, clicks DOM elements, fills out forms, and evaluates arbitrary JavaScript.

**Internal Event Broadcasting:**
* **Filesystem Notifications:** Whenever a tool modifies a file, `ToolContext` (via the injected `OnFileChanged` trait) notifies the system to ensure the UI tree, indexing worker, and tabs immediately reflect the change (typically publishing `FileEventKind::Discovered`, `Updated`, or `Removed`).
* **UI Status Updates:** The agent streams its progress (`AgentEventObserver`) by publishing `AgentStatus` (thinking, streaming text, tool execution) to the UI.
* **Telemetry:** Spams `AgentDebugEntry` telemetry for developer observability and emits `BackgroundLogEntry` events during MCP discovery timeouts or failures.
