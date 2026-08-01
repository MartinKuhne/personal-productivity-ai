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
| `list_files_by_tag` | List files that contain a specific tag in their front-matter. Paginated via `offset`/`limit` (0-indexed; default `offset=0`, `limit=100`); the response carries `total` and an optional `hint`. |
| `list_files` | List markdown files in a directory (non-recursive). With "/" or "." returns library names. Paginated via `offset`/`limit` (0-indexed; default `offset=0`, `limit=100`); the response carries `total` and an optional `hint`. |
| `read_file` | Read the entire text contents of a file. |
| `read_file_lines` | Read specific line numbers or ranges from a file (1-indexed). |
| `create_file` | Create a new markdown file with the specified content. |
| `insert_lines` | Insert new lines of text into an existing file at a specific 1-indexed position. |
| `delete_lines` | Delete specific lines from a file (1-indexed, inclusive). |
| `replace_text` | Replace exact occurrences of old_string with new_string in a file. |
| `web_fetch` | Fetch content from a URL and convert HTML to Markdown. Paginated by Markdown line via `offset`/`limit` (default `offset=0`, `limit=100` lines); the response carries `total_lines`. Cached for 5 minutes in the shared `ToolCache`; pass `force_refetch=true` to bypass. |
| `web_search` | Search the web using SearXNG. Requires searxng_url config. |
| `web_delegate` | Delegate complex web research to a sub-agent with web_fetch/web_search tools. |
| `browser_navigate` | Drive the persistent headless Firefox page to a URL. State (cookies, JS, scroll) is preserved across calls (BRWS-001). Requires `tool_groups.browser`. |
| `browser_get_page_state` | Return interactable elements plus current `url` and `title`; ReadOnly, parallel-safe (BRWS-002). |
| `browser_click` | Click a single element by CSS selector (BRWS-003). |
| `browser_fill_input` | Fill a single `<input>` or `<textarea>` (BRWS-004). |
| `browser_select_dropdown` | Pick a `<select>` option by its `value` attribute (BRWS-005). |
| `browser_press_key` | Press a single keyboard key on the page (BRWS-006). |
| `browser_evaluate_js` | Evaluate an arbitrary JavaScript expression; true escape hatch (BRWS-007). |
| `browser_screenshot` | Save a PNG to `browser.screenshot_dir`; filename sanitised (BRWS-008). |
| `read_yaml_header` | Parse a YAML header from a markdown file and return its content. |
| `write_yaml_header` | Write or update data in a YAML header to a markdown file. |
| `search_calendar` | Search calendar events by keyword. Requires CalDAV config. |
| `get_calendar` | Get calendar items by date range. Requires CalDAV config. |
| `get_calendar_item` | Get a specific calendar item by its full href. Requires CalDAV config. |
| `add_calendar_item` | Add a new calendar item. Requires CalDAV config. |
| `update_calendar_item` | Update a calendar item. Requires CalDAV config. |
| `delete_calendar_item` | Delete a calendar item. Requires CalDAV config. |
| `search_email` | Search email by keyword, folder, date range, sender, recipient, unread, or flagged status. The first call returns up to 100 matching emails plus a `cursor`; pass the `cursor` back unchanged to get the next page. The full server result set is cached for 5 minutes. When the result set is exhausted the response includes a `hint` and no `cursor`. Requires JMAP config. |
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
* [TOOL-006] Web Fetch Pagination: The `web_fetch` tool shall accept `offset` (default 0) and `limit` (default 100) integer parameters. `offset` is the number of Markdown lines to skip. `limit` is the number of Markdown lines to return. The helper that slices the body MUST NOT cap `limit`; the LLM owns the page-size choice (TOOL-027).
* [TOOL-007] Pagination Total: Every list-paginated tool (`list_files`, `list_files_by_tag`, `web_fetch`) MUST return a `total` field on its response. The value MUST be the item count across all pages. `search_email` MUST also return `total`; the value MUST be the item count across all pages of the same search and MUST be identical across all pages.
* [TOOL-008] Web Fetch Cache: The system shall cache fetched Markdown content for 5 minutes. Subsequent calls to `web_fetch` with the same URL and `force_refetch` set to `false` (default) shall return the cached content without making a network request. The cache is the shared `ToolCache` defined in TOOL-030.
* [TOOL-009] Web Fetch Force Refetch: The `web_fetch` tool shall accept an optional `force_refetch` boolean parameter (default: `false`). When `true`, the system shall invalidate the shared `ToolCache` entry for the URL and fetch fresh content, replacing the cached entry.
* [TOOL-010] Web Fetch Context Efficiency: The `web_fetch` tool description shall state that the LLM can save context by fetching a URL once and then issuing partial reads via `offset` and `limit` to paginate through the Markdown body, rather than re-fetching the same URL. The description MUST use the canonical vocabulary of TOOL-028.

### Grep Tool

* [TOOL-011] Grep Result Cap: The `grep` tool shall return at most 200 matching lines in a single response, capped across all configured content libraries.
* [TOOL-012] Grep Truncation Guidance: When the `grep` result is truncated by the 200-line cap, the response shall indicate the truncation and instruct the caller to refine the query with narrower terms or delegate to a sub-agent to analyse a specific file.
* [TOOL-013] Grep Scope: The `grep` tool shall only search Markdown files with a `.md` extension within configured content libraries, and shall not return matches outside a library's root folder.

### Tool Manager

The system shall maintain a single `ToolManager` type that owns the tool catalog, the per-group state, the error tracking, the parallel-safety classification, and the MCP client manager. There shall be no separate `ToolRegistry` type (TOOL-024). The manager lives in `src/agent/tools/manager/` and exposes free functions (`execute_tool`, `get_tools_schema`, `safety_of`, `init_mcp_on_startup`, `groups_snapshot`, `set_group_enabled`, `tool_char_count_for`, `clear_error`, `mcp_manager`) consumed by the agent loop, the tool executor, and the Tools dialog.

* [TOOL-014] Tool Group Enumeration: The `ToolManager` shall enumerate every tool group — the eight built-in groups (filesystem, web, browser, email, contacts, calendar, csv_db, weather) and every configured MCP server — and expose, for each, its display name, kind label ("Internal" or "MCP"), enabled state, the list of tool names it contains, whether all of its tools are parallel-safe, and its most recent error (if any).
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

### Tool Pagination (offset/limit, cursor, shared cache)

The four paginated tools split into two classes. The list-paginated tools (`list_files`, `list_files_by_tag`, `web_fetch`) use a stateless `offset`/`limit` model. The search tool (`search_email`) uses a stateful cursor model backed by the shared `ToolCache`. The split is deliberate — see TOOL-029 for the rationale.

* [TOOL-025] Pagination Hint: Every paginated tool MUST return a `hint` field on its response. For list-paginated tools, `hint` MUST be set to a human-readable message when `total == 0` or `offset >= total`. For `search_email`, `hint` MUST be set to `"Final page."` when the response has no `cursor`. `hint` MUST be absent (or `null`) otherwise and MUST be `skip_serializing_if = "Option::is_none"`.
* [TOOL-026] Pagination Defaults: `list_files` and `list_files_by_tag` MUST default `offset=0`, `limit=100`. `web_fetch` MUST default `offset=0` (lines), `limit=100` (lines). `search_email` does not use `offset`/`limit`; the page size is fixed by the tool (TOOL-029).
* [TOOL-027] No Pagination Cap: List-paginated tools MUST NOT cap `limit`. If the LLM requests more items than exist, the tool returns all remaining items starting at `offset` and reports the true `total` on the response. The LLM is responsible for choosing a `limit` that fits its context window; the tool's job is to honor the request and report the truth. `search_email` is not subject to this requirement because the LLM does not control the page size.
* [TOOL-028] Pagination Vocabulary: List-paginated tools MUST use the parameter names `offset` and `limit` and the response field names `total` and `hint`. The names `page` and `page_size` MUST NOT appear in any list-paginated tool's schema. `search_email` is exempt from this rule; it uses the cursor parameter and response field per TOOL-029. The LLM-facing description of every list-paginated tool MUST include the canonical paragraph defined in the per-family strings module (`builtin/strings/paging::CANONICAL_DESCRIPTION`).
* [TOOL-029] Search Email Cursor: The `search_email` tool MUST accept a `cursor: Option<String>` input parameter and return a `cursor: Option<String>` output field. The first call (no cursor in input) returns up to 100 matching emails plus a new cursor. Subsequent calls with the same cursor return the next 100 emails (or fewer on the final page). The cursor is opaque and MUST be passed back unchanged. When the result set is exhausted, the response includes a `hint` and no `cursor`. The page size is fixed at 100; the LLM does not control it.
* [TOOL-030] Shared Tool Cache: A process-local `ToolCache` MUST be shared by `search_email` and `web_fetch`. The cache MUST be `Mutex<HashMap<String, CacheEntry>>` wrapped in `LazyLock`. Cache entries MUST be evicted lazily on access after `CACHE_TTL` (5 minutes). A soft cap of `MAX_CACHE_ENTRIES` (1024) MUST be enforced with FIFO eviction once exceeded. The cache MUST NOT be persisted across process restarts. The cache is the single source of truth for both tools' per-URL / per-search result set state.
* [TOOL-031] Search Email Cache Population: The first `search_email` call with a given filter set MUST populate the cache with the full server result set. Subsequent calls with the matching cursor MUST slice from the cache without re-fetching. A `search_email` call with a cursor that does not match a live cache entry MUST return the error `"Cursor expired or unknown; re-run the search with no cursor."`
* [TOOL-032] Web Fetch Cache Migration: The `web_fetch` tool MUST move its cache from a process-local `LazyLock<Mutex<HashMap<String, _>>>` to the shared `ToolCache` (TOOL-030). The URL MUST be the cache key. The `force_refetch` parameter MUST invalidate the cache entry before re-fetching. The 5-minute TTL MUST match the shared cache's TTL exactly.

### Browser Automation Tools

Eight tools under the new Browser group (BRWS-001..008) drive a long-lived headless Firefox process shared across every mutating call. The session is owned by pp::browser::BrowserSession, which is plumbed through AgentContext ? ToolExecutor ? ToolContext as Arc<BrowserSession>. The default group-enable flag is alse to preserve the "no system access" posture described in the README; the user opts in by setting 	ool_groups.browser: true in config.yaml.

* [BRWS-001] Persistent Page: rowser_navigate (and every other browser tool) shall drive a single headless Firefox Page instance held by the long-lived BrowserSession. The page, its BrowserContext (cookie jar), and its storage state shall persist across every tool call inside a single agent turn. A subsequent rowser_navigate to a new URL shall not destroy cookies or JS state from the previous page.
* [BRWS-002] Read-Only Page State: rowser_get_page_state shall be the only ReadOnly tool in the Browser group and shall be parallel-safe. It shall return at minimum the current url, the page 	itle, a 	otal count, and a JSON-encoded elements array of interactable elements (a / button / input / select / textarea) with stable gent_id, 	ag, 	ext, placeholder, 
ame, 	ype, and href fields.
* [BRWS-003] Click: rowser_click shall accept a CSS selector argument and call Playwright's Locator::click on the persistent page. It shall be Mutating.
* [BRWS-004] Fill Input: rowser_fill_input shall accept a CSS selector and a 	ext argument and call Playwright's Locator::fill on the persistent page. It shall replace any existing value. It shall be Mutating.
* [BRWS-005] Select Dropdown: rowser_select_dropdown shall accept a CSS selector and a alue argument and call Playwright's Locator::select_option on the persistent page. It shall be Mutating.
* [BRWS-006] Press Key: rowser_press_key shall accept a key argument and call Playwright's page.keyboard().press on the persistent page. It shall be Mutating.
* [BRWS-007] Evaluate JS: rowser_evaluate_js shall accept a script argument (an expression or arrow function) and call Playwright's page.evaluate. The return value shall be serialised to JSON in the response. It shall be Mutating; the Safety::Mutating default is mandatory because the script is an arbitrary escape hatch.
* [BRWS-008] Screenshot: rowser_screenshot shall accept a ilename argument and an optional ull_page boolean (default alse). The ilename shall be sanitised: only [A-Za-z0-9._-], no .., no path separators, = 128 chars, must not start with .. The screenshot shall be written to <screenshot_dir>/<filename>, where screenshot_dir defaults to <first content library>/browser-screenshots. Any violation shall surface as an error envelope; the LLM shall not be able to write outside the configured directory.

### Browser Session Lifecycle

* [BRWS-SESSION-001] Lazy Launch: BrowserSession::page shall launch the headless Firefox process on the first call after construction. Subsequent calls shall reuse the same Page until either an idle-timeout close or an explicit orget() invocation. A concurrent first call from two threads shall not launch twice; the second call observes the in-progress launch and returns the same Page.
* [BRWS-SESSION-002] Idle Timeout: BrowserSession::tick (called once per UI frame) shall close the live browser after rowser.idle_timeout_seconds of tool-call silence. The next call shall relaunch and reload cookies from storage_state_path, so persistent login survives idle timeouts. idle_timeout_seconds: 0 shall disable the timeout.
* [BRWS-SESSION-003] Persistent Storage: After every mutating browser tool call the session shall call BrowserContext::storage_state and write the result to rowser.storage_state_path (overwriting atomically). The same file shall be reloaded as the initial BrowserContextOptions::storage_state on the next launch.
* [BRWS-SESSION-004] Forget: BrowserSession::forget shall close the live browser, delete rowser.storage_state_path, and reset the idle-timeout clock. The Tools dialog exposes this as the "Forget Browser Session" action; it gives the user a clean logout.
* [BRWS-SESSION-005] Engine: rowser.browser_type shall accept only irefox in this revision. Any other value shall surface a SessionError::UnsupportedBrowserType at the first tool call.
* [BRWS-SESSION-006] Install Requirement: If Playwright's Firefox binary is not installed locally, the first page() call shall fail with a SessionError::Launch whose message tells the user to run playwright install firefox. The app shall NOT auto-install the browser.
* [BRWS-SESSION-007] Sync-to-Async Bridge: Every browser tool's Tool::execute (which is sync) shall drive the underlying Playwright future on the process-wide Tokio runtime via the existing 	ools::blocking::block_on helper, the same bridge the CalDAV / CardDAV tools use. The runtime is shared.

### Browser Configuration

* [BRWS-CONF-001] Default Off: 	ool_groups.browser shall default to alse. The user shall opt in by setting it to 	rue in config.yaml. No code path shall auto-enable it.
* [BRWS-CONF-002] Restricted Screenshot Path: rowser.screenshot_dir shall default to <first content library>/browser-screenshots. An empty value uses the default. The LLM-provided ilename shall be sanitised per BRWS-008 before being joined with the directory; the LLM shall not be able to write elsewhere.
* [BRWS-CONF-003] Browser Type: rowser.browser_type shall default to irefox. The value is a free-form string so a future revision can add other engines without a schema change, but the only supported value today is irefox.
* [BRWS-CONF-004] Idle Timeout Default: rowser.idle_timeout_seconds shall default to 300 (5 minutes).   disables the timeout.
* [BRWS-CONF-005] Storage State Path: rowser.storage_state_path shall default to %APPDATA%\fastmd\browser-storage.json on Windows and the XDG ~/.config/fastmd/ equivalent elsewhere. The file is overwritten on every mutating call; if the JSON is corrupt on reload, the session logs a warning and launches with an empty context.
