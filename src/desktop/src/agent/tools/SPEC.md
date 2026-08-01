# LLM Tools Specification

> **GUARDRAIL**: This specification file is managed by the spec-split workflow. Do not edit
> this file directly unless explicitly instructed. Any changes to requirements must be
> reflected in the corresponding implementation code. If drift is detected between
> this spec and the actual code behavior, notify the user immediately.
>
> Part of [`SPEC.md`](../../SPEC.md) (FastMD crate)

## Requirements

The requirements below have been formatted using the **Easy Approach to Requirements Syntax (EARS)**, utilizing Ubiquitous, Event-Driven (When), State-Driven (While), Unwanted Behavior (If), and Optional Feature (Where) templates.

### LLM Tools Table

| Tool | Description |
|---|---|
| `grep` | Search for a specific pattern within Markdown files across all libraries. Returns at most 200 matching lines; when the result is truncated, the response directs the caller to refine the query with narrower terms or delegate to a sub-agent to analyse a specific file. |
| `read_tags` | Read all unique tags from markdown front-matter across all libraries. |
| `list_files_by_tag` | List files that contain a specific tag in their front-matter. |
| `list_files` | List markdown files in a directory (non-recursive). With "/" or "." returns library names. |
| `read_file` | Read the entire text contents of a file. |
| `read_file_lines` | Read specific line numbers or ranges from a file (1-indexed). |
| `create_file` | Create a new markdown file with the specified content. |
| `insert_lines` | Insert new lines of text into an existing file at a specific 1-indexed position. |
| `delete_lines` | Delete specific lines from a file (1-indexed, inclusive). |
| `replace_text` | Replace exact occurrences of old_string with new_string in a file. |
| `web_fetch` | Fetch content from a URL and convert HTML to Markdown. Supports pagination via `limit`/`offset`, optional response `headers`, and a 5-minute cache. |
| `web_search` | Search the web using SearXNG. Requires searxng_url config. |
| `web_delegate` | Delegate complex web research to a sub-agent with web_fetch/web_search tools. |
| `read_yaml_header` | Parse a YAML header from a markdown file and return its content. |
| `write_yaml_header` | Write or update data in a YAML header to a markdown file. |
| `search_calendar` | Search calendar events by keyword. Requires CalDAV config. |
| `get_calendar` | Get calendar items by date range. Requires CalDAV config. |
| `get_calendar_item` | Get a specific calendar item by its full href. Requires CalDAV config. |
| `add_calendar_item` | Add a new calendar item. Requires CalDAV config. |
| `update_calendar_item` | Update a calendar item. Requires CalDAV config. |
| `delete_calendar_item` | Delete a calendar item. Requires CalDAV config. |
| `search_email` | Search email by keyword, folder, date range, sender, recipient, unread, or flagged status. Results are paginated (default page size 10); every response includes total for follow-up page requests. Requires JMAP config. |
| `get_email_by_id` | Get email by id. Requires JMAP config. |
| `get_email` | Get email by date range, sender, recipient, unread, or flagged status. Requires JMAP config. |
| `send_email` | Send an email. Requires JMAP config. |
| `search_contact` | Search contacts by keyword. Requires JMAP config. |
| `get_contact` | Get contact by id. Requires JMAP config. |
| `add_contact` | Add a new contact. Requires JMAP config. |
| `add_rows` | Add rows to a CSV file database. |
| `delete_rows` | Delete rows from a CSV file database based on a predicate. |
| `create_csv` | Create a new CSV file database with specified headers. |
| `list_csv` | List all CSV file databases. |
| `query` | Query a CSV file database using an evalexpr predicate, supporting sum and average aggregates. |

### CSV Database Tools

* [TOOL-001] Tool Availability: The CSV database tools (`add_rows`, `delete_rows`, `create_csv`, `list_csv`, `query`) shall only be offered to the LLM if the user's query contains any of the tool names, "table", "csv", or "database".
* [TOOL-002] Query Evaluation: The `query` tool shall use the `evalexpr` crate to parse and execute query predicates as dynamic expressions against CSV rows.
* [TOOL-003] Aggregate Functions: The query system shall allow `sum` and `average` as aggregate functions over a specified column.
* [TOOL-004] The system shall store all csv databases in a user specified location. Default to %APPDATA%\fastmd\db\ if not configured.

### Web Fetch Pagination & Caching

* [TOOL-005] Web Fetch Headers: The `web_fetch` tool shall accept an optional `headers` boolean parameter (default: `false`). When `true`, the response shall include the HTTP response headers as a JSON object alongside the content.
* [TOOL-006] Web Fetch Pagination: The `web_fetch` tool shall accept optional `limit` (integer) and `offset` (integer, default 0) parameters. The `limit` parameter shall restrict the number of lines returned. The `offset` parameter shall skip the specified number of lines from the start of the content.
* [TOOL-007] Web Fetch Total Lines: The `web_fetch` response shall include a `total_lines` integer indicating the total number of lines in the full fetched content, enabling the caller to paginate through the content.
* [TOOL-008] Web Fetch Cache: The system shall cache fetched Markdown content for 5 minutes. Subsequent calls to `web_fetch` with the same URL and `force_refetch` set to `false` (default) shall return the cached content without making a network request.
* [TOOL-009] Web Fetch Force Refetch: The `web_fetch` tool shall accept an optional `force_refetch` boolean parameter (default: `false`). When `true`, the system shall bypass the cache and fetch fresh content from the URL, replacing the cached entry.
* [TOOL-010] Web Fetch Context Efficiency: The tool description shall encourage the LLM to save context by fetching a page once and issuing partial reads via `limit` and `offset` to paginate through the content, rather than re-fetching the full page multiple times.

### Grep Tool

* [TOOL-011] Grep Result Cap: The `grep` tool shall return at most 200 matching lines in a single response, capped across all configured content libraries.
* [TOOL-012] Grep Truncation Guidance: When the `grep` result is truncated by the 200-line cap, the response shall indicate the truncation and instruct the caller to refine the query with narrower terms or delegate to a sub-agent to analyse a specific file.
* [TOOL-013] Grep Scope: The `grep` tool shall only search Markdown files with a `.md` extension within configured content libraries, and shall not return matches outside a library's root folder.

### Tool Manager

The system shall maintain a single `ToolManager` type that owns the tool catalog, the per-group state, the error tracking, the parallel-safety classification, and the MCP client manager. There shall be no separate `ToolRegistry` type (TOOL-024). The manager lives in `src/agent/tools/manager/` and exposes free functions (`execute_tool`, `get_tools_schema`, `safety_of`, `init_mcp_on_startup`, `groups_snapshot`, `set_group_enabled`, `tool_char_count_for`, `clear_error`, `mcp_manager`) consumed by the agent loop, the tool executor, and the Tools dialog.

* [TOOL-014] Tool Group Enumeration: The `ToolManager` shall enumerate every tool group — the seven built-in groups (filesystem, web, email, contacts, calendar, csv_db, weather) and every configured MCP server — and expose, for each, its display name, kind label ("Internal" or "MCP"), enabled state, the list of tool names it contains, whether all of its tools are parallel-safe, and its most recent error (if any).
* [TOOL-015] Per-Tool Char Counting: For each tool, the system shall compute the character count as the byte length of the JSON serialization of `{"type":"function","function":{"name":<name>,"description":<description>,"parameters":<schema>}}` — the exact fragment the tool contributes to the LLM `tools` array.
* [TOOL-016] Group Char Sum: The character count for a group shall be the sum of the per-tool char counts of every tool in the group that is currently enabled by both the group's own enable flag and any prompt-content rule (`is_enabled(config, prompt)` returns `true`).
* [TOOL-017] Enable Toggle — Internal: Toggling a built-in tool group's checkbox in the UI shall flip the corresponding field on `AppConfig::tool_groups` and shall take effect on the next `get_tools_schema` call. The change shall be persisted to `config.yaml` immediately.
* [TOOL-018] Enable Toggle — MCP: Toggling a configured MCP server's checkbox in the UI shall flip `AppConfig::mcp_servers[server_name].enabled` (CONFIG-012) and shall take effect on the next `get_tools_schema` call. The server's `transport`, `command`, `url`, `headers`, and `oauth` fields shall remain intact. The change shall be persisted to `config.yaml` immediately.
* [TOOL-019] Parallel Safety Lookup: The `ToolManager` shall expose `safety_of(name) -> Safety` and `parallel_safe_tools() -> Vec<String>`. A tool is parallel-safe iff its `Tool::safety()` returns `Safety::ReadOnly`.
* [TOOL-020] Group Parallel Safety: A group's `parallel_safe` flag shall be `true` iff every tool in `group.tool_names` is parallel-safe. The flag is recomputed by `refresh_state`.
* [TOOL-021] Error Recording: The `ToolManager` shall record the most recent error per group via `record_error(group, ToolGroupError)`. A successful `Execution` in the group shall clear an `Execution`-kind error; `Discovery`/`Authentication`/`ConfigInvalid` errors shall remain until the next successful `refresh_state` (or `refresh_mcp_tools` for `Discovery`) for that group.
* [TOOL-022] Catalog Refresh: The `ToolManager` shall provide `refresh_state(&AppConfig)` that recomputes the per-group view (enabled flag, tool names, parallel-safety, preserved `last_error`) without performing I/O. The agent loop and the UI dialog shall call `refresh_state` after every config change and on every dialog open.
* [TOOL-023] MCP Refresh: The `ToolManager` shall provide `refresh_mcp_tools(&AppConfig)` that re-runs `tools/list` against every configured MCP server, registers the discovered tools into the catalog, and records `Discovery` errors on the affected group when a server fails. On success, any prior `Discovery` error on the group shall be cleared.
* [TOOL-024] Single Manager Type: The system shall maintain a single `ToolManager` type that owns the tool catalog, the per-group state, the error tracking, the parallel-safety classification, and the MCP client manager. There shall be no separate `ToolRegistry` type.
