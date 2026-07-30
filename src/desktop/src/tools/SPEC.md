# LLM Tools Specification

> Part of [`SPEC.md`](../../SPEC.md) (FastMD crate). See the
> [Requirements Index](../../SPEC.md#requirements-index) for the full
> REQ-xxx → file map.
>
> Owns TOOL-001..010. Cross-cutting requirements that also touch other
> modules are listed at the bottom of this file.

## Requirements

The requirements below have been formatted using the **Easy Approach to Requirements Syntax (EARS)**, utilizing Ubiquitous, Event-Driven (When), State-Driven (While), Unwanted Behavior (If), and Optional Feature (Where) templates.

### LLM Tools Table

| Tool | Description |
|---|---|
| `grep` | Search for a specific pattern within files across all libraries. |
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

> Conditionally available — only offered to the LLM when the user prompt contains "table", "csv", "database", or a CSV tool name.

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