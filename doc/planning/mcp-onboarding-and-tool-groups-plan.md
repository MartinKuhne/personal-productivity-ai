# Plan: Onboard `mcp-client-rust` and Configurable Tool Groups

**Status**: Proposal  
**Date**: 2026-07-27  
**Target Component**: `src/desktop` (`fastmd`)  

---

## 1. Executive Summary

This document presents a comprehensive implementation plan to:
1. **Onboard `mcp-client-rust`** into `fastmd` (`src/desktop`) to support external Model Context Protocol (MCP) servers operating locally (over `stdio`) or remotely (over `sse` / HTTP).
2. **Add User Configuration for MCP Servers**: Allow users to specify local binaries/scripts and remote endpoints in `AppConfig` (`config.toml` / `config.json`).
3. **Add User Configuration for Internal Tool Groups**: Provide explicit options in `AppConfig` to enable or disable internal tool groups (`filesystem`, `web`, `email`, `contacts`, `calendar`, `csv_db`, `weather`), defaulting all groups to `enabled = true`.

---

## 2. Technical Context & Requirements

### 2.1 Existing Architecture
- **Configuration**: Defined in `src/desktop/src/config.rs` (`AppConfig`), persisted as TOML/JSON.
- **Tool System**: Defined in `src/desktop/src/tools/`:
  - `Tool` trait in `mod.rs` (`name`, `description`, `parameters_schema`, `is_enabled`, `safety`, `execute`).
  - `ToolRegistry` in `registry.rs` registers and dispatches tools.
- **Tool Execution Context**: `ToolContext` provides access to `AppConfig`, Tokio runtime, and system resources.

### 2.2 Requirements
- **REQ-MCP-001**: Support local MCP servers via `stdio` transport (command, arguments, environment variables).
- **REQ-MCP-002**: Support remote MCP servers via `sse` transport (URL, custom HTTP headers/auth).
- **REQ-MCP-003**: Provide dynamic discovery of tools exposed by configured MCP servers via `tools/list` and dispatch calls via `tools/call`.
- **REQ-TG-001**: Introduce `tool_groups` configuration section in `AppConfig`.
- **REQ-TG-002**: Support toggling `filesystem`, `web`, `email`, `contacts`, `calendar`, `csv_db`, and `weather` tool groups individually.
- **REQ-TG-003**: Default all internal tool groups to `true` (enabled).

---

## 3. Data Structures & Configuration Schemas

### 3.1 Internal Tool Groups (`ToolGroupsConfig`)

Added to `src/desktop/src/config.rs`:

```rust
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct ToolGroupsConfig {
    pub filesystem: bool,
    pub web: bool,
    pub email: bool,
    pub contacts: bool,
    pub calendar: bool,
    pub csv_db: bool,
    pub weather: bool,
}

impl Default for ToolGroupsConfig {
    fn default() -> Self {
        Self {
            filesystem: true,
            web: true,
            email: true,
            contacts: true,
            calendar: true,
            csv_db: true,
            weather: true,
        }
    }
}
```

### 3.2 MCP Server Configuration (`McpServerConfig`)

Added to `src/desktop/src/config.rs`:

```rust
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum McpServerConfig {
    /// Local MCP server spawned via stdio child process
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
        #[serde(default = "default_true")]
        enabled: bool,
    },
    /// Remote MCP server over Server-Sent Events (SSE) / HTTP
    Sse {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        #[serde(default = "default_true")]
        enabled: bool,
    },
}

fn default_true() -> bool {
    true
}
```

### 3.3 `AppConfig` Updates

```rust
#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(default)]
pub struct AppConfig {
    // ... existing fields ...

    /// Internal tool group enablement flags. Default: all true.
    #[serde(default)]
    pub tool_groups: ToolGroupsConfig,

    /// External Model Context Protocol (MCP) server definitions.
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,
}
```

### 3.4 TOML User Configuration Example

```toml
# config.toml

[tool_groups]
filesystem = true
web = true
email = true
contacts = true
calendar = true
csv_db = true
weather = true

[mcp_servers.local_filesystem]
transport = "stdio"
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "C:/Users/mkuhn/documents"]
enabled = true

[mcp_servers.remote_weather]
transport = "sse"
url = "https://mcp.weather-service.example.com/sse"
enabled = true

[mcp_servers.remote_weather.headers]
Authorization = "Bearer secret_token_123"
```

---

## 4. Architectural Design & Implementation

### 4.1 Internal Tool Group Enforcement
Each tool module in `src/desktop/src/tools/` is mapped to an internal group:
- `filesystem`: `ReadFileTool`, `ReadFileLinesTool`, `CreateFileTool`, `InsertLinesTool`, `DeleteLinesTool`, `ReplaceTextTool`, `ListFilesTool`, `ListFilesByTagTool`, `ReadTagsTool`, `GrepTool`, `ReadYamlHeaderTool`, `WriteYamlHeaderTool`
- `web`: `WebFetchTool`, `WebSearchTool`, `WebDelegateTool`
- `email`: `SearchEmailTool`, `GetEmailByIdTool`, `SendEmailTool`
- `contacts`: `SearchContactTool`, `AddContactTool`, `GetContactTool`
- `calendar`: CalDAV calendar tools
- `csv_db`: `CsvCreateTool`, `CsvListTool`, `CsvAddRowsTool`, `CsvDeleteRowsTool`, `CsvQueryTool`
- `weather`: `WeatherTool`

In `Tool::is_enabled(&self, config: &AppConfig, prompt: &str) -> bool`, the tool inspects its corresponding flag in `config.tool_groups`. If `false`, the tool is excluded from `get_tools_schema` and `ToolRegistry::execute`.

### 4.2 Onboarding `mcp-client-rust` & MCP Bridge
1. **Dependency Addition** (`src/desktop/Cargo.toml`):
   ```toml
   [dependencies]
   mcp-client-rust = "0.1"
   ```
2. **MCP Module** (`src/desktop/src/tools/mcp.rs`):
   - `McpClientManager`: Manages active connections to configured MCP servers (stdio handles or SSE client instances).
   - `McpToolAdapter`: Implements `crate::tools::Tool` for each tool discovered via `tools/list`.
     - `name`: Namespaced as `mcp_<server_id>_<remote_tool_name>`.
     - `description`: Directly derived from the MCP tool description.
     - `parameters_schema`: Directly derived from the MCP tool input schema.
     - `is_enabled`: Returns `true` if `config.mcp_servers.get(server_id)` is enabled.
     - `execute`: Dispatches `tools/call` RPC via `mcp-client-rust` handle within Tokio runtime.

3. **Tool Registry Integration**:
   - `ToolRegistry` queries `McpClientManager` to register dynamic `McpToolAdapter` instances alongside internal tools.

---

## 5. Detailed Implementation Phases

### Phase 1: Configuration & Internal Tool Groups
- **Task 1.1**: Define `ToolGroupsConfig` in `src/desktop/src/config.rs` with `Default` returning `true` for all groups.
- **Task 1.2**: Add `tool_groups` field to `AppConfig`.
- **Task 1.3**: Update `is_enabled` in internal tool implementations to check `config.tool_groups.<group_name>`.
- **Task 1.4**: Add unit tests in `src/desktop/tests/config_test.rs` verifying default enablement and toggle behavior.

### Phase 2: `mcp-client-rust` Onboarding & Configuration
- **Task 2.1**: Add `mcp-client-rust` dependency to `src/desktop/Cargo.toml`.
- **Task 2.2**: Define `McpServerConfig` (`Stdio` vs `Sse`) in `src/desktop/src/config.rs`.
- **Task 2.3**: Add `mcp_servers: HashMap<String, McpServerConfig>` field to `AppConfig`.
- **Task 2.4**: Create `src/desktop/src/tools/mcp.rs` implementing `McpClientManager` and `McpToolAdapter`.
- **Task 2.5**: Wire MCP tool registration into `ToolRegistry`.

### Phase 3: Testing & Quality Gate Verification
- **Task 3.1**: Write integration tests for local (`stdio`) and remote (`sse`) MCP server connections.
- **Task 3.2**: Write integration tests for tool group toggling (verifying schema filtering and execution blocks).
- **Task 3.3**: Run `cargo check` and `cargo test` to ensure zero compilation or lint warnings.

---

## 6. Consequences & Considerations

- **Security**: Local `stdio` MCP servers execute external subprocesses; the plan enforces strict command/arg isolation and respects existing tool safety classifications (`Safety::Mutating` by default).
- **Performance**: MCP tool discovery (`tools/list`) is performed asynchronously at setup time to prevent blocking UI frame rendering.
- **Backward Compatibility**: All existing configurations deserialize seamlessly with default `true` values for tool groups and empty MCP server maps.
