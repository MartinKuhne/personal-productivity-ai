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
| `create_file` | Create a new markdown file with the specified content. Fails if the file already exists — this tool can only create new files. |
| `insert_lines` | Insert new lines of text into an existing file at a specific 1-indexed position. |
| `replace_text` | Replace exact occurrences of old_string with new_string in a file. |
| `web_fetch` | Fetch content from a URL and convert HTML to Markdown. Cursor-based pagination: returns up to 100 lines and a `cursor` token; pass the `cursor` back unchanged to get the next page. The full content is cached for 5 minutes in the shared `ToolCache`; pass `force_refetch=true` to bypass. |
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
| `add_rows` | Add rows to a CSV database. |
| `delete_rows` | Delete rows from a CSV database using an expression. |
| `create_csv` | Create a CSV database with specified column headers. |
| `list_csv` | List all CSV databases. |
| `query` | Query a CSV database using an expression or aggregate function. |
| `trello_get_boards` | Fetch all Trello boards for the authenticated user. Requires Trello config. |
| `trello_get_board` | Fetch details of a Trello board by its ID. Requires Trello config. |
| `trello_get_lists` | Fetch all lists in a Trello board by its ID. Requires Trello config. |
| `trello_get_cards` | Fetch all cards in a Trello list by its ID. Requires Trello config. |
| `trello_create_card` | Create a new card in a specific Trello list. Instructs the LLM to include detailed requirements, priority, and links. Requires Trello config. |
| `trello_update_card` | Update an existing Trello card (e.g. name, description, move to list). Requires Trello config. |
| `trello_delete_card` | Delete a Trello card by its ID. Requires Trello config. |

#### Trello Tools

* [TOOL-033] Tool Availability: The Trello tools shall only be offered to the LLM if the user configures the `trello_client` with `token` and `apiKey` fields, and the `trello` group is enabled.
* [TOOL-034] FastMD Label: When creating a new card using `trello_create_card`, the system shall automatically attempt to attach a "FastMD" label (in blue) to the card, creating the label on the parent board if it does not already exist.
* [TOOL-035] Detailed Card Creation: The `trello_create_card` tool description shall instruct the LLM to specify what needs to be accomplished, when, how, and by whom, as well as an estimated priority and relevant context/links.

#### CSV Database Tools

* [TOOL-001] Tool Availability: The CSV database tools (`add_rows`, `delete_rows`, `create_csv`, `list_csv`, `query`) shall only be offered to the LLM if the user's query contains any of the tool names, "table", "csv", or "database".
* [TOOL-002] Query Evaluation: The `query` tool shall parse and execute query predicates as dynamic expressions against CSV rows.
* [TOOL-003] Aggregate Functions: The query system shall allow `sum` and `average` as aggregate functions over a specified column.
* [TOOL-004] The system shall store all csv databases in a user specified location. Default to %APPDATA%\fastmd\db\ if not configured.

### Web Fetch Pagination & Caching

* [TOOL-005] Web Fetch Headers: The `web_fetch` tool shall accept an optional `headers` boolean parameter (default: `false`). When `true`, the response shall include the HTTP response headers as a JSON object alongside the content.
* [TOOL-006] Web Fetch Pagination: The `web_fetch` tool shall use cursor-based pagination. It accepts an optional `cursor: Option<String>` input parameter and returns a `cursor: Option<String>` output field. The first call (no cursor in input) returns up to 100 Markdown lines plus a new cursor. Subsequent calls with the same cursor return the next 100 lines (or fewer on the final page). The cursor is opaque and MUST be passed back unchanged. When the result set is exhausted, the response includes a `hint` and no `cursor`. The page size is fixed at 100 lines; the LLM does not control it.
* [TOOL-007] Pagination Total: Every cursor-paginated tool (`web_fetch`, `search_email`) MUST return a `total_lines` / `total` field on its response. The value MUST be the item count across all pages and MUST be identical across all pages.
* [TOOL-008] Web Fetch Cache: The system shall cache fetched Markdown content for 5 minutes. Subsequent calls to `web_fetch` with the same URL and `force_refetch` set to `false` (default) shall return the cached content without making a network request.
* [TOOL-009] Web Fetch Force Refetch: The `web_fetch` tool shall accept an optional `force_refetch` boolean parameter (default: `false`). When `true`, the system shall invalidate the shared cache entry for the URL and fetch fresh content, replacing the cached entry.
* [TOOL-010] Web Fetch Context Efficiency: The `web_fetch` tool description shall state that the LLM can save context by fetching a URL once and then using the cursor token to paginate through the Markdown body, rather than re-fetching the same URL.

### Grep Tool

* [TOOL-011] Grep Result Cap: The `grep` tool shall return at most 200 matching lines in a single response, capped across all configured content libraries.
* [TOOL-012] Grep Truncation Guidance: When the `grep` result is truncated by the 200-line cap, the response shall indicate the truncation and instruct the caller to refine the query with narrower terms or delegate to a sub-agent to analyse a specific file.
* [TOOL-013] Grep Scope: The `grep` tool shall only search Markdown files with a `.md` extension within configured content libraries, and shall not return matches outside a library's root folder.

### Tool Manager

The system shall maintain a single tool manager that owns the tool catalog, the per-group state, the error tracking, the parallel-safety classification, and the MCP client manager. The manager exposes functions consumed by the agent loop, the tool executor, and the Tools dialog.

* [TOOL-014] Tool Group Enumeration: The tool manager shall enumerate every tool group — the eight built-in groups (filesystem, web, browser, email, contacts, calendar, csv_db, weather) and every configured MCP server — and expose, for each, its display name, kind label ("Internal" or "MCP"), enabled state, the list of tool names it contains, whether all of its tools are parallel-safe, and its most recent error (if any).
* [TOOL-015] Per-Tool Char Counting: For each tool, the system shall compute the character count as the byte length of the JSON serialization of `{"type":"function","function":{"name":<name>,"description":<description>,"parameters":<schema>}}` — the exact fragment the tool contributes to the LLM `tools` array.
* [TOOL-016] Group Char Sum: The character count for a group shall be the sum of the per-tool char counts of every tool in the group that is currently enabled by both the group's own enable flag and any prompt-content rule (`is_enabled(config, prompt)` returns `true`).
* [TOOL-017] Enable Toggle — Internal: Toggling a built-in tool group's checkbox in the UI shall flip the corresponding field on `AppConfig::tool_groups` and shall take effect on the next tool schema fetch. The change shall be persisted to `config.yaml` immediately.
* [TOOL-018] Enable Toggle — MCP: Toggling a configured MCP server's checkbox in the UI shall flip `AppConfig::mcp_servers[server_name].enabled` (CONFIG-012) and shall take effect on the next tool schema fetch. The server's `transport`, `command`, `url`, `headers`, and `oauth` fields shall remain intact. The change shall be persisted to `config.yaml` immediately.
* [TOOL-019] Parallel Safety Lookup: The tool manager shall expose tool safety lookup and list all parallel-safe tools. A tool is parallel-safe iff its safety classification is ReadOnly.
* [TOOL-020] Group Parallel Safety: A group's `parallel_safe` flag shall be `true` iff every tool in the group is parallel-safe.
* [TOOL-021] Error Recording: The tool manager shall record the most recent error per group. A successful execution in the group shall clear an execution-kind error; discovery/authentication/config invalid errors shall remain until the next successful refresh for that group.
* [TOOL-022] Catalog Refresh: The tool manager shall provide a state refresh capability that recomputes the per-group view (enabled flag, tool names, parallel-safety, preserved error) without performing I/O.
* [TOOL-023] MCP Refresh: The tool manager shall provide an MCP refresh capability that re-runs tool discovery against every configured MCP server, registers discovered tools into the catalog, and records discovery errors on affected groups when a server fails.
* [TOOL-024] Single Manager Type: The system shall maintain a single unified tool manager that owns the tool catalog, per-group state, error tracking, parallel-safety classification, and MCP client manager.

### Tool Pagination (cursor, shared cache)

The cursor-paginated tools (`search_email`, `web_fetch`) use a stateful cursor model backed by the shared tool cache.

* [TOOL-025] Pagination Hint: Every paginated tool MUST return a `hint` field on its response. For cursor-paginated tools (`search_email`, `web_fetch`), `hint` MUST be set to `"Final page."` when the response has no `cursor`. `hint` MUST be absent (or `null`) otherwise and MUST be `skip_serializing_if = "Option::is_none"`.
* [TOOL-026] Pagination Defaults: The page size is fixed at 100 for both `search_email` (emails) and `web_fetch` (Markdown lines). The LLM does not control the page size.
* [TOOL-027] No Pagination Cap: Cursor-paginated tools are not subject to limit caps because the LLM does not control the page size.
* [TOOL-028] Pagination Vocabulary: Cursor-paginated tools MUST use the parameter name `cursor` and the response field names `cursor`, `total`/`total_lines`, and `hint`. The names `page`, `page_size`, `offset`, and `limit` MUST NOT appear in any cursor-paginated tool's schema. The LLM-facing description of every cursor-paginated tool MUST include the standardized cursor-based paging description paragraph.
* [TOOL-029] Search Email Cursor: The `search_email` tool MUST accept a `cursor: Option<String>` input parameter and return a `cursor: Option<String>` output field. The first call (no cursor in input) returns up to 100 matching emails plus a new cursor. Subsequent calls with the same cursor return the next 100 emails (or fewer on the final page). The cursor is opaque and MUST be passed back unchanged. When the result set is exhausted, the response includes a `hint` and no `cursor`. The page size is fixed at 100; the LLM does not control it.
* [TOOL-030] Shared Tool Cache: An in-memory process-local cache MUST be shared by `search_email` and `web_fetch`. Cache entries MUST be evicted lazily on access after 5 minutes. A capacity cap of 1024 entries MUST be enforced with FIFO eviction once exceeded. The cache MUST NOT be persisted across process restarts. The cache is the single source of truth for both tools' per-URL / per-search result set state.
* [TOOL-031] Search Email Cache Population: The first `search_email` call with a given filter set MUST populate the cache with the full server result set. Subsequent calls with the matching cursor MUST slice from the cache without re-fetching. A `search_email` call with a cursor that does not match a live cache entry MUST return the error `"Cursor expired or unknown; re-run the search with no cursor."`
* [TOOL-032] Web Fetch Cache Shared Integration: The `web_fetch` tool MUST utilize the shared tool cache (TOOL-030) for fetched content. The URL maps to a cursor UUID which maps to the full content. The `force_refetch` parameter MUST invalidate the cache entry before re-fetching. The 5-minute TTL MUST match the shared cache's TTL exactly.

### Browser Automation Tools

Eight tools under the Browser group (BRWS-001..008) drive a long-lived headless browser session shared across mutating calls. The default group-enable flag is false; the user opts in by setting `tool_groups.browser: true` in config.yaml.

* [BRWS-001] Persistent Page: `browser_navigate` (and every other browser tool) shall drive a single headless browser instance. The page, cookie jar, and storage state shall persist across every tool call inside a single agent turn. A subsequent `browser_navigate` to a new URL shall not destroy cookies or JS state from the previous page.
* [BRWS-002] Read-Only Page State: `browser_get_page_state` shall be the only ReadOnly tool in the Browser group and shall be parallel-safe. It shall return at minimum the current URL, page title, total count, and a JSON-encoded array of interactable elements with stable attributes.
* [BRWS-003] Click: `browser_click` shall accept a CSS selector argument and trigger a click event on the persistent page element. It shall be Mutating.
* [BRWS-004] Fill Input: `browser_fill_input` shall accept a CSS selector and a text argument and populate the input on the persistent page. It shall replace any existing value. It shall be Mutating.
* [BRWS-005] Select Dropdown: `browser_select_dropdown` shall accept a CSS selector and a value argument and select the dropdown option on the persistent page. It shall be Mutating.
* [BRWS-006] Press Key: `browser_press_key` shall accept a key argument and send the key press to the persistent page. It shall be Mutating.
* [BRWS-007] Evaluate JS: `browser_evaluate_js` shall accept a script argument and evaluate it on the persistent page. The return value shall be serialised to JSON in the response. It shall be Mutating.
* [BRWS-008] Screenshot: `browser_screenshot` shall accept a filename argument and an optional full_page boolean (default false). The filename shall be sanitised. The screenshot shall be written to `<screenshot_dir>/<filename>`.

### Browser Session Lifecycle

* [BRWS-SESSION-001] Lazy Launch: The browser session shall launch the headless browser process on the first call after construction. Subsequent calls shall reuse the same page session until closed.
* [BRWS-SESSION-002] Idle Timeout: The session shall close the live browser after `browser.idle_timeout_seconds` of tool-call silence. The next call shall relaunch and reload cookies from persistent storage state.
* [BRWS-SESSION-003] Persistent Storage: After every mutating browser tool call the session shall save session storage state. The storage state shall be reloaded on the next launch.
* [BRWS-SESSION-004] Forget: The session reset action shall close the live browser, delete persistent storage state, and reset idle timeout state.
* [BRWS-SESSION-005] Engine: `browser.browser_type` shall support configured browser engine selection.
* [BRWS-SESSION-006] Install Requirement: If the browser binary is not installed locally, the launch call shall return an actionable error informing the user to install the browser requirement.
* [BRWS-SESSION-007] Sync-to-Async Bridge: Browser tools shall execute synchronously with downstream asynchronous browser automation without blocking the host application.

### Browser Configuration

* [BRWS-CONF-001] Default Off: `tool_groups.browser` shall default to false.
* [BRWS-CONF-002] Restricted Screenshot Path: `browser.screenshot_dir` shall default to `<first content library>/browser-screenshots`.
* [BRWS-CONF-003] Browser Type: `browser.browser_type` shall default to `firefox`.
* [BRWS-CONF-004] Idle Timeout Default: `browser.idle_timeout_seconds` shall default to 300 (5 minutes). 0 disables the timeout.
* [BRWS-CONF-005] Storage State Path: `browser.storage_state_path` shall default to standard user application data paths.
