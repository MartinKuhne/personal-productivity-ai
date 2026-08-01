# Tool Paging: Audit & Migration Plan

Status: proposal
Date: 2026-07-31
Reviewer: Mavis

This document merges the read-only paging audit (Part I) with the migration plan that standardizes the four already-paginated tools on `offset`/`limit` (Part II). Part I establishes the findings; Part II is the plan of record that resolves finding F-6 and, along the way, F-3 and F-4.

Scope: `src/desktop/src/agent/tools/` (built-in tools, registry, executor) and the LLM-facing strings in `registry/builtin/strings/`.

---

# Part I — Agent Tool Paging & LLM-Instruction Audit

A read-only audit of every built-in agent tool in `fastmd`. Each tool is checked for:
1. whether it can return more than one result,
2. whether it exposes paging to the LLM,
3. the paging semantics (1-indexed/0-indexed, page+page_size vs. limit+offset, default value),
4. whether the default batch size is appropriate,
5. whether the LLM-facing description is explicit and consistent about paging,
6. gaps to close.

MCP-discovered tools are out of scope for the per-tool review (their schema comes from the upstream server), but the adapter's treatment of paging is noted in the summary.

## TL;DR (severity-ordered)

| # | Finding | Severity | Affected tools |
|---|---------|----------|----------------|
| F-1 | Multi-result search/listing tools have **no paging at all** — return all matches as one opaque JSON string | **High** | `search_calendar`, `get_calendar`, `get_calendar_item`, `search_contact`, `get_contact` |
| F-2 | `web_search` paging is **delegated to the SearXNG server** with no client-side cap and no `total`/next-page signal | **High** | `web_search` |
| F-3 | `grep` truncation footer uses a hard-coded "200 matches" string that is **not derived from the constant** — drift risk if `DEFAULT_GREP_MAX_RESULTS` changes | **Medium** | `grep` |
| F-4 | `web_fetch` default `limit` is **100 lines** in code, but the description does not say so and the default `offset` is 0 (so the first call always returns 100 lines) | **Medium** | `web_fetch` |
| F-5 | `search_email` truncates each email body to **10 lines silently** with no `body_truncated` field on each item; the LLM only learns about it from the in-body footer | **Medium** | `search_email` |
| F-6 | **Inconsistent paging vocab across tools**: `page`/`page_size` (1-indexed) for fs/email vs. `offset`/`limit` (0-indexed) for `web_fetch`; both are valid but the LLM has to learn two idioms | **Medium** | all paginated tools |
| F-7 | `get_weather` returns a JSON array of NWS forecast periods but has **no paging** and no documented cap; NWS provides ~7 days × 2 periods/day = ~14 items but the value is hard-coded inside `weather.rs` and the LLM has no way to ask for fewer | **Medium** | `get_weather` |
| F-8 | `read_tags` returns the full de-duped set with no cap; a workspace with thousands of unique tags will blow context on a single call | **Low** | `read_tags` |
| F-9 | `csv` `query` returns the full matched set with no cap; the only saving grace is the spec says TOOL-003 covers aggregates, not paging | **Low** | `query` (csv) |
| F-10 | `list_files`/`list_files_by_tag` accept a `page_size` with **no documented upper bound**; the LLM can ask for 10,000 rows in a single page. The audit originally called this a gap; Part II decided the LLM owns the page-size choice, so this is now a non-issue. | **Low (closed)** | `list_files`, `list_files_by_tag` |
| F-11 | `DeleteEmailInput`/`DeleteEmailResponse` DTOs exist in `dtos.rs` but no `delete_email` tool is registered in `register_all_builtins` — dead code | **Low** | `delete_email` (absent) |
| F-12 | Pagination instructions in `prompt_builder.rs` are absent — the only "context bloat" hint is `read_file`/`read_yaml_header`; nothing about `page`/`offset`/`limit`/truncation handling | **Low** | all paginated tools |
| F-13 | LLM-facing `description` fields mix conventions: some embed the default in prose, some reference the constant name (`DEFAULT_GREP_MAX_RESULTS`), one (weather) embeds the API fact "NWS only provides ~7 days of forecast" in the wrong place (the error string instead of the description) | **Low** | `grep`, `weather` |

## Inventory of all built-in tools

Total: **32 built-in tools** registered in `src/desktop/src/agent/tools/registry/builtin/mod.rs::register_all_builtins`.

```
fs (8):          replace_text, grep, read_tags, list_files_by_tag,
                 list_files, read_file, read_file_lines, create_file,
                 insert_lines, delete_lines         (10)
web (3):         web_delegate, web_fetch, web_search
yaml (2):        read_yaml_header, write_yaml_header
caldav (6):      search_calendar, get_calendar, get_calendar_item,
                 add_calendar_item, update_calendar_item, delete_calendar_item
jmap (3):        search_email, get_email_by_id, send_email
carddav (3):     search_contact, add_contact, get_contact
csv (5):         create_csv, list_csv, add_rows, delete_rows, query
weather (1):     get_weather
```

Plus dynamic MCP tools via `McpToolAdapter` (paging depends on the upstream server's schema; the adapter does not synthesize page params).

## Per-tool review

For each tool, the columns are:

- **Multi-result?** — does a single call return more than one row/item?
- **Paging?** — are `page`/`page_size`, `offset`/`limit`, or similar exposed to the LLM?
- **Default batch** — what the LLM gets if it omits the paging arg.
- **Default appropriate?** — does the default fit a typical 8k–32k-token context window?
- **LLM instructions** — are the description and per-field strings explicit about paging?
- **Notes** — anything else worth flagging.

### Filesystem family (`fs.rs`)

| Tool | Multi? | Paging? | Default | OK? | LLM instructions | Notes |
|------|--------|---------|---------|-----|-------------------|-------|
| `replace_text` | no | — | — | — | ok | mutating |
| `grep` | yes (capped) | no (cap only) | 200 matches, across all libraries | yes for the cap; **no page for the rest** | partial — description says "refine query" but does not say "no follow-up pages" | see F-3 |
| `read_tags` | yes | no | all unique tags | **no** (F-8) | description says "all unique tags"; no mention of cap | |
| `list_files_by_tag` | yes | `page` / `page_size` (1-indexed) | 20 / page | yes | explicit | uses `paginate_in_range` |
| `list_files` | yes | `page` / `page_size` (1-indexed) | 20 / page | yes | explicit | uses `paginate_in_range`; also handles `/` and `.` to enumerate libraries |
| `read_file` | no | — | — | — | ok | mutating, "entire text contents" |
| `read_file_lines` | no | implicit (`start_line`/`end_line`) | — | — | description does not warn that very wide ranges will be unreadable | not paging in the list-result sense; pairs with `read_file` |
| `create_file`, `insert_lines`, `delete_lines` | no | — | — | — | ok | mutating |

**`grep` cap behavior** — `registry/builtin/fs.rs:88` truncates to `DEFAULT_GREP_MAX_RESULTS` (200), and `:90–93` appends a literal string `"(results truncated at 200 matches; refine the query with narrower terms or delegate to a sub-agent to analyse a specific file)"`. The string `200 matches` is hard-coded — if `DEFAULT_GREP_MAX_RESULTS` ever moves, the message and the description will drift. `registry/builtin/strings/fs.rs:14` references `DEFAULT_GREP_MAX_RESULTS` by name in the schema description, which is also brittle (the LLM cannot resolve that symbol; it just sees the literal text `DEFAULT_GREP_MAX_RESULTS`).

**`list_files_by_tag` and `list_files` paging semantics** — both use `paginate_in_range(...)` in `registry/pagination.rs`. The helper:
- 1-indexed `page` (page 0 is normalized to page 1 — see `registry/tests.rs:391 test_list_by_tag_page_zero_is_normalised_to_page_one`).
- `page_size` has no documented upper bound — the LLM could ask for `page_size: 100000` and the helper would return it. (F-10)
- `total` is always returned.
- A `hint` field appears on the response when `page` is past the end or there are zero matches; otherwise it is `None` (skip-serialized).

### Web family (`web.rs`)

| Tool | Multi? | Paging? | Default | OK? | LLM instructions | Notes |
|------|--------|---------|---------|-----|-------------------|-------|
| `web_delegate` | no (returns summarized text) | — | — | — | ok | delegates to a sub-agent |
| `web_fetch` | yes (paginated lines) | `offset` / `limit` (0-indexed lines) | offset 0, **limit 100** | partial — see F-4 | description tells the LLM to use `limit`/`offset` and rely on `total_lines`; does not say what the default `limit` is | |
| `web_search` | yes (server-driven) | no (client side) | whatever SearXNG returns (default `search.max_results` = 10) | **no** — F-2 | description is one line: "Search the web using SearXNG." | server-side limit; no `total`; no client-side cap; comment in `web.rs:128–132` says "we don't slice on the client because the operator asked to see whatever the server actually returned" |

**`web_fetch` default drift** — `web.rs:103`:
```rust
let limit = limit.unwrap_or(100);
```
The description in `strings/web.rs:9` says:
> "Supports pagination via limit/offset to save context — fetch once, then read sections. Response includes total_lines for pagination. Content is cached for 5 minutes; use force_refetch=true to bypass cache."

The LLM is not told that omitting `limit` returns 100 lines, not "all lines". With `offset` defaulting to 0, a first call without args will return 100 lines and a `total_lines` figure; the LLM has to learn that the rest is unconsumed. The 100-line default is reasonable for a Markdown page (most rendered pages are 100–500 lines), but it should be stated in the description.

**`web_search` is the largest gap** — the LLM gets whatever SearXNG returns with no client-side cap and no `total`/`next` signal. A SearXNG instance with many engines and a high `search.max_results` setting can easily return 30–50 results, each with a title, URL, and a 200–500-character snippet — that is ~10–25k tokens in a single tool response. The `format_tool_result_message` only counts `\n\n`-separated blocks (`response_formatter.rs:133-140`), which is a fragile heuristic. (F-2)

### CalDAV family (`caldav.rs`)

| Tool | Multi? | Paging? | Default | OK? | LLM instructions | Notes |
|------|--------|---------|---------|-----|-------------------|-------|
| `search_calendar` | yes | **none** | every event containing the keyword, across every configured CalDAV server, every calendar, with no date constraint | **no** — F-1 | description is one word: "Search the calendar by keyword." | |
| `get_calendar` | yes | **none** | every event in `[start_date, end_date]` across every server | **no** — F-1 | description: "Get calendar items by date range." | |
| `get_calendar_item` | yes (1 result per server) | — | one row per configured server, but practically 1 | — | description is explicit about using the full `href` | "Use the exact, full 'href' value" is the only good description in the family |
| `add_calendar_item`, `update_calendar_item`, `delete_calendar_item` | no | — | — | — | thin descriptions | |

**The CalDAV search/listing gap is the worst in the codebase** — `search_calendar` will happily return every event across every calendar that contains a common word ("meeting", "lunch", "standup") and the LLM has no way to ask for fewer. The result is wrapped in `serde_json::to_string_pretty` of `{results, errors}`, embedded in a `String` field, and the LLM has to parse that to know how many events came back. (F-1)

### CardDAV family (`carddav.rs`)

| Tool | Multi? | Paging? | Default | OK? | LLM instructions | Notes |
|------|--------|---------|---------|-----|-------------------|-------|
| `search_contact` | yes | **none** | every contact whose vCard contains the keyword, across every configured server, every addressbook | **no** — F-1 | description: "Search contacts by keyword." | |
| `get_contact` | yes (1 per server) | — | — | — | description: "Get contact by id." | |
| `add_contact` | no | — | — | — | ok | |

Same gap as CalDAV. The `carddav.rs:67–84` helper `fetch_contacts_from_book` uses `sync_collection(..., Some(10000), ...)` to pull up to 10,000 items in a single call — there is no cap in the search path.

### JMAP email family (`jmap.rs`)

| Tool | Multi? | Paging? | Default | OK? | LLM instructions | Notes |
|------|--------|---------|---------|-----|-------------------|-------|
| `search_email` | yes | `page` / `page_size` (1-indexed) | 10 / page | yes for paging; **see F-5 for body truncation** | explicit (the most detailed description in the codebase) | hard-codes `Some(10)` for body truncation in `email.rs:397` |
| `get_email_by_id` | no | — | — | — | ok | |
| `send_email` | no | — | — | — | ok | |

**`search_email` body truncation (F-5)** — `email.rs:397` calls `simplify_email(&mut email, Some(10))`, which caps each email body to 10 lines and appends `"\n... (truncated - use the get_email_by_id tool with the email id to read the full content)"` if truncated (`email.rs:188–190`). The LLM has no field on each email object that says "this body is truncated" — it has to parse the in-body footer. There is no equivalent of `web_fetch.total_lines` for email; the LLM cannot tell from the response shape which emails are truncated vs. which fit, only by reading the body string. This is exactly the kind of signal that would be hard for an LLM to learn by accident.

`MAX_BODY_VALUE_BYTES` is set to `10 * 1024 * 1024` (`email.rs:34`) — a 10 MiB cap on a single MIME part. That is a server-side bound on what JMAP will return, not a context budget; a single 10 MiB email body is far more than any LLM context will accept. This is paired with the 10-line `simplify_email` cap, so the practical limit is 10 lines + a `is_truncated: true` flag from the JMAP server. The pairing is good but the description should note "bodies are truncated to 10 lines per email; fetch a single email by id to see the full body."

`search_email` also has an odd two-step layout — the function fetches **all** matching email IDs from JMAP, then `email_get`s each one in full, then paginates the resulting list. With `page_size=10` and 100 matches that's 100 round-trips for one tool call. Worth noting for performance, though not for paging semantics. (Not in the findings table — separate concern.)

### CSV family (`csv.rs` + `csv_db/`)

| Tool | Multi? | Paging? | Default | OK? | LLM instructions | Notes |
|------|--------|---------|---------|-----|-------------------|-------|
| `create_csv` | no | — | — | — | ok | |
| `list_csv` | yes (all CSVs in db dir) | **none** | every `.csv` file in the db dir | reasonable — usually a handful | description: "List all CSV file databases." | |
| `add_rows` | no | — | — | — | ok | |
| `delete_rows` | no (returns count) | — | — | — | ok | |
| `query` | yes | **none** | every row matching the predicate (plus optional aggregate) | **no** — F-9 | description mentions aggregates but not row-count cap | |

`query` will dump every row of a multi-thousand-row CSV into the response, then add the aggregate. The spec only requires aggregates (TOOL-002/TOOL-003), not paging, so this is a gap rather than a violation — but it is the same gap class as CalDAV/CardDAV.

### Weather family (`weather.rs`)

| Tool | Multi? | Paging? | Default | OK? | LLM instructions | Notes |
|------|--------|---------|---------|-----|-------------------|-------|
| `get_weather` | yes (NWS periods) | **none** | every NWS forecast period that matches the date filter; up to ~14 | **partial** — F-7 | description says "~7 days" and "optionally filter by a date (YYYY-MM-DD)"; does not say the result is a list | the actual NWS limit is documented in the *error message* (`weather.rs:179`), not the description |

`get_weather` returns a JSON array of NWS forecast periods (each with `period_name`, `start_time`, `temperature`, `detailed_forecast`). With no `date_range` it returns all 14 periods; with a `date_range` it filters server-side. The description says "optionally filter by a date (YYYY-MM-DD)" but the field is a free-form string that supports both an ISO date and substring matching — a richer description ("date filter is matched as substring; pass `YYYY-MM-DD` to scope to a single day") would help. (F-7)

### YAML family (`yaml.rs`)

| Tool | Multi? | Paging? | Default | OK? | LLM instructions | Notes |
|------|--------|---------|---------|-----|-------------------|-------|
| `read_yaml_header` | no | — | — | — | good — explicitly recommends it over `read_file` to save context | |
| `write_yaml_header` | no | — | — | — | ok | |

### Single-result family

| Tool | Multi? | Notes |
|------|--------|-------|
| `replace_text`, `create_file`, `insert_lines`, `delete_lines`, `read_file`, `read_file_lines`, `web_delegate`, `add_calendar_item`, `update_calendar_item`, `delete_calendar_item`, `add_contact`, `add_rows`, `delete_rows`, `get_calendar_item`, `get_contact`, `get_email_by_id`, `send_email`, `read_yaml_header`, `write_yaml_header`, `create_csv` | no | n/a |

## Cross-tool findings

### Paging vocabulary (F-6)

Three different idioms appear in the schema:

| Idiom | Tools | Indexing | Default |
|-------|-------|----------|---------|
| `page` + `page_size` | `list_files`, `list_files_by_tag`, `search_email` (audit state, pre-migration) | 1-indexed `page` | 20 (fs) / 10 (email) |
| `offset` + `limit` | `web_fetch` | 0-indexed `offset`; `limit` is the line count | offset 0, limit 100 |
| hard cap (no paging) | `grep` (200), `read_tags` (none), `web_search` (server), all CalDAV/CardDAV search, all CSV search, `get_weather`, `query` | — | — |

A unified convention would help the LLM. The LLM-facing description should at minimum state which idiom applies. Today:

- `web_fetch` description says "limit/offset" — ok.
- `list_files` / `list_files_by_tag` / `search_email` use `page` / `page_size` and the per-field strings mention "1-indexed page number. Defaults to 1" — ok.
- No tool explains *why* two idioms exist.

This is not a blocker, but it adds friction and is the kind of thing an LLM will quietly get wrong. Post-migration, the four paginated tools converge on two idioms (offset/limit for list tools, cursor for search_email) plus one optional `next_offset`/`has_next` pattern; the rest remain a tracked gap.

### Prompt-level guidance (F-12)

`prompt_builder.rs:86` has one bullet:
> "CRITICAL: Avoid context bloat! Do NOT use the read_file tool on multiple files in a single step. Always prefer read_yaml_header to survey documents, or grep to extract specific information without reading entire files."

Nothing in the system prompt tells the LLM:
- that `grep` will truncate to 200 lines and what to do about it,
- that `web_fetch` paginates lines and how to walk them,
- that `list_files`/`search_email` are page-based,
- that `web_search` has no follow-up page,
- that `get_weather` returns a list.

Each of these is a footgun the LLM has to learn from the per-tool description (or from a failed turn). Given the brevity of some of the descriptions (`web_search`, `search_calendar`, `get_calendar`, `search_contact`), the LLM will not learn the paging semantics from the schema alone.

### Description quality (F-13)

Ranked by LLM-helpfulness (post-migration):

1. `search_email` (post-migration) — describes the cursor flow, the 100-item page size, the 5-minute cache, and the final-page hint. The LLM gets a single paragraph that walks it through the entire interaction.
2. `web_fetch` (post-migration) — explicit about limit/offset, total_lines, default `limit` of 100 lines, and the 5-minute cache.
3. `list_files`, `list_files_by_tag` (post-migration) — explicit about offset, limit, total, hint, and the "default 100" rule.
4. `grep` — explicit about the 200 cap and what to do when truncated.
5. `read_yaml_header` — useful hint about preferring it over `read_file` to save context.
6. `web_delegate` — gives the high-level reason ("protects your context window") but no specifics.
7. `read_file_lines` — one sentence; doesn't mention start_line/end_line semantics or 1-indexing.
8. `search_calendar`, `get_calendar`, `search_contact`, `get_contact`, `list_csv`, `add_rows`, `delete_rows`, `add_calendar_item`, `update_calendar_item`, `delete_calendar_item`, `add_contact`, `create_csv`, `web_search`, `send_email`, `get_email_by_id` — single-sentence descriptions that do not mention return shape or limits.
9. `read_tags` — "Get all unique tags" with no mention of size or cap.
10. `query` (csv) — mentions aggregates, not the (unbounded) row count.
11. `get_weather` — mentions the date filter but not the list shape or the "NWS only provides ~7 days" fact (that fact lives in the error path).

### Default page sizes

| Default | Value | Tool | Reasoning that supports it |
|---------|-------|------|----------------------------|
| `page_size` for fs listing | 20 | `list_files`, `list_files_by_tag` | Matches what fits in a 4–8k-token list response |
| `page_size` for email | 10 | `search_email` | Email bodies are heavier (incl. body preview); 10 is conservative |
| `limit` for web_fetch | 100 lines | `web_fetch` | One Markdown page is usually 100–500 lines; 100 lets the LLM see the first screen |
| `MAX_BODY_VALUE_BYTES` | 10 MiB | `search_email` server-side | Required by RFC 8621 to override the 0 default; not a context budget |
| `simplify_email` body lines | 10 | `search_email` | Reasonable; flagged because it is silent — F-5 |
| `DEFAULT_GREP_MAX_RESULTS` | 200 | `grep` | Standard in the IDE/search world |

All defaults are defensible. The gaps are around (a) what the LLM is told about the default, and (b) what happens when the LLM asks for a larger page.

### Truncation signaling

| Tool | Truncation signal on the response | Adequate? |
|------|-----------------------------------|-----------|
| `grep` | `truncated: bool` + a footer line in `matches` | yes — explicit field, but footer text duplicates the cap (F-3) |
| `list_files` / `list_files_by_tag` | `hint: Option<String>` only when page is past end or no matches; otherwise `None` | ok |
| `search_email` | `cursor: Option<String>` for "more pages exist"; `hint: "Final page."` for "exhausted" | yes for paging; **no signal for per-email body truncation** — F-5 |
| `web_fetch` | `total_lines: usize` always | yes; combined with the formatted-result line in `response_formatter.rs:108–127` that says "X of Y markdown lines returned" |
| `web_search` | none | n/a (no cap) |
| `get_weather` | none | n/a (~14 items) |
| CalDAV/CardDAV/CSV | none | n/a (no cap) |

### MCP tools (out of scope, but worth noting)

`McpToolAdapter` (in `mcp/mod.rs`) passes the server's schema straight through. If the upstream MCP server declares `cursor`/`nextCursor` or `offset`/`limit` params, the LLM will see them in the schema; if not, the LLM has no paging affordance. The adapter does **not** synthesize or wrap paging params. That is the correct behavior for an MCP client (don't lie about what the server supports), but it means the agent loop has no uniform contract for MCP tools and the LLM has to read each server's schema.

### Dead code (F-11)

`src/desktop/src/agent/tools/dtos.rs:298–304` defines `DeleteEmailInput` and `DeleteEmailResponse`, but `register_all_builtins` in `registry/builtin/mod.rs:21–54` does not register a `delete_email` tool. The struct, derive, and JsonSchema impls are unused. Either delete the DTOs or register a tool — leaving it dead is a small but real maintenance trap (any future `delete_email` reference will assume the wiring is there).

## Paging semantics summary (the matrix the LLM needs)

| Tool | Schema params | Indexing | Default | Total field | Hint when past end |
|------|---------------|----------|---------|-------------|---------------------|
| `grep` | none (cap only) | n/a | first 200 matches | `total: usize` (count of all matches) + `truncated: bool` | n/a (no paging) |
| `list_files` | `offset: Option<usize>`, `limit: Option<usize>` | 0-indexed | offset 0, limit 100 | `total: usize` (cross-library) | `hint: Option<String>` |
| `list_files_by_tag` | `offset: Option<usize>`, `limit: Option<usize>` | 0-indexed | offset 0, limit 100 | `total: usize` (cross-library) | `hint: Option<String>` |
| `search_email` | `cursor: Option<String>` | cursor | first call returns first 100; subsequent calls return next 100 | `total: usize` (cross-client, identical across pages) | `cursor` absent + `hint: "Final page."` |
| `web_fetch` | `offset: Option<usize>`, `limit: Option<usize>` | 0-indexed lines | offset 0, limit 100 | `total_lines: usize` (line count of full content) | n/a (offset past end returns empty content) |
| `web_search` | none | n/a | server's `search.max_results` (~10) | none | n/a |
| `read_tags` | none | n/a | all unique tags | none | n/a |
| `search_calendar` / `get_calendar` | none | n/a | every match | none | n/a |
| `search_contact` | none | n/a | every match | none | n/a |
| `get_weather` | none | n/a | every matching period | none | n/a |
| `query` (csv) | none | n/a | every matching row | none | n/a |
| `read_file_lines` | `start_line: usize`, `end_line: usize` | 1-indexed, inclusive | — | `content: String` (no line count) | n/a (error if out of range) |

## Recommended remediations

Ordered by impact. This list is the audit's original proposal. Status indicates how Part II (the migration) disposes of each item. The migration covers only the four tools that already page (F-6, plus F-3 and F-4 along the way); everything else remains a tracked gap.

1. **Add paging to the CalDAV/CardDAV search and CSV `query` tools (F-1, F-9).** Same `page`/`page_size` + `total` + `hint` shape as `list_files`. Default 20. Reuse `paginate_in_range`. Update `SearchCalendarResponse`/`SearchContactResponse`/`QueryResponse` to expose the fields. This is the single biggest context-bloat risk in the current set. — *Status: out of scope for Part II (see Part II "Out of scope").*
2. **Add a `web_search` paging story (F-2).** Two options:
   - **Client-side cap**: take a `count` param (default 10), take the first N from SearXNG, return them with a `total_results: usize` and a `next_offset: Option<usize>`. Loses SearXNG's "more results" metadata but gives the LLM a contract.
   - **SearXNG pagination**: forward `pageno` and `time_range` to SearXNG. More work; depends on the operator's SearXNG config.
   - **Cursor with shared cache (recommended)**: same pattern as the new `search_email` mechanism. Cache SearXNG's full result set in the shared `ToolCache`; return the first page plus a cursor. This is the most consistent choice but requires `web_search` to be a paginated tool and to participate in the cache. — *Status: out of scope for Part II.*
3. **Plumb the `simplify_email` body cap into the per-email response (F-5).** Add a `body_truncated: bool` to `simplify_email`'s output. The LLM should be able to read a single field instead of parsing the in-body footer. The DTO for `search_email` is currently `results: String` — promoting that to a proper structured DTO would also let the LLM iterate by index instead of parsing JSON inside a string. — *Status: out of scope for Part II.*
4. **Add a `total` and `next_offset` (or `page`/`total_pages`) signal to `web_search` once the paging is added.** — *Status: out of scope for Part II.*
5. **Update the `web_fetch` description (F-4)** to state the default `limit` is 100 lines, and the default `offset` is 0. Also document that `total_lines` is the count of *Markdown* lines after HTML conversion, not the source HTML. — *Status: addressed by Part II* (canonical vocabulary + `web_fetch` domain sentence).
6. **Constrain `page_size` (F-10).** Add a `const MAX_PAGE_SIZE: usize = 100` and clamp in `paginate_in_range`. Reject (or clamp) the LLM's request silently — the test in `registry/tests.rs:289` already covers the past-end case, so the helper can grow a clamp without breaking tests. — *Status: rejected by Part II. The migration deliberately does NOT cap `limit`; the LLM owns the page-size choice (see Part II "Decision" → "Defaults per tool"). F-10 is accepted as a non-issue.*
7. **De-hard-code the grep truncation footer (F-3).** Use `format!("... (results truncated at {} matches; ...)", DEFAULT_GREP_MAX_RESULTS)`. Replace the literal `DEFAULT_GREP_MAX_RESULTS` in the schema description with the actual number — the LLM has no way to resolve that symbol. — *Status: addressed by Part II* (see "Constraints").
8. **Document the date filter on `get_weather` (F-7).** Move the "NWS only provides ~7 days of forecast" fact from the error path to the description, where the LLM can read it before calling. — *Status: out of scope for Part II.*
9. **Cap or paginate `read_tags` (F-8).** A workspace with 5,000 unique tags is a single tool call that returns 5,000 strings. Either add a `page`/`page_size` here too, or document a "tags returned, deduplicated, sorted" cap. — *Status: out of scope for Part II.*
10. **Standardize paging vocabulary (F-6).** Pick one of `page`/`page_size` (1-indexed) or `offset`/`limit` (0-indexed). Recommendation: `page`/`page_size` because (a) three of the four already-paginated tools use it, (b) it pairs naturally with `total` and `hint`, (c) `web_fetch`'s line-based `offset`/`limit` is a special case (line slicing, not item slicing) and is fine to keep as-is — but state that explicitly in the description. — *Status: superseded by Part II. The migration chose `offset`/`limit` (0-indexed) for every paginated tool, reversing this recommendation. See Part II "Decision".*
11. **Strengthen the system prompt (F-12).** Add a short paragraph to `prompt_builder.rs`:
    - "When a tool returns `total` and `hint`, prefer paginating with `page`/`page_size` over re-querying with narrower filters, unless you have a reason to think the filter is wrong."
    - "When a tool returns `truncated: true` (e.g. `grep`), do not ask for the next page — refine the query or delegate to a sub-agent."
    - "For `web_fetch`, fetch once and then page through the result with `offset`/`limit`. Do not re-fetch the same URL." — *Status: out of scope for Part II.*
12. **Tighten weak descriptions (F-13).** Specifically: `web_search`, `search_calendar`, `get_calendar`, `search_contact`, `get_contact`, `read_tags`, `read_file_lines`, `list_csv`, `add_rows`, `delete_rows`, `query`. Each should mention its return shape and (where relevant) the implicit cap. — *Status: partially addressed by Part II* (only the paginated tools' descriptions are rewritten; the rest remain tracked gaps).
13. **Remove or register `DeleteEmailInput`/`DeleteEmailResponse` (F-11).** — *Status: out of scope for Part II.*
14. **Consider exposing `MAX_BODY_VALUE_BYTES` in the schema description** for `search_email` — even a hint like "long bodies are capped at 10 MiB server-side and truncated to 10 lines client-side" would help. — *Status: out of scope for Part II.*

## What is *not* in scope for this audit

- MCP server-side paging contracts (depends on each server).
- Internal pagination inside `csv_db::query::query_csv` (single-shot is fine; per-tool slicing lives in the tool layer, not the implementation).
- LLM-side heuristics for "stop after N pages" (a separate concern, lives in the agent loop / `process_turn`).
- Performance of `search_email` fetching every email body up front (a `find_n+1` problem worth a separate ticket).

## Test coverage observations

- `registry/tests.rs` covers `list_files` and `list_files_by_tag` paging thoroughly (default size, past-end, multi-library, page-0 normalization, no-match). Good.
- `registry/tests.rs` covers `grep` (no-match, cap, scope, non-markdown) but not the "exactly 200 matches" boundary (off-by-one risk if the cap is moved).
- No tests cover `search_email` paging — the helper at `email.rs:428–448` is a parallel implementation of the same logic, not a reuse of `paginate_in_range`. Consolidating would let the same test cover both, and remove the duplication risk.
- No tests cover `web_search` paging (because there is none).
- No tests cover `web_fetch` `offset` past end, or `limit=0`.
- No tests cover the F-3 case (truncation message matches the constant).

## Appendix: file locations

| Concern | File |
|---------|------|
| Tool trait + registry | `src/desktop/src/agent/tools/mod.rs`, `src/desktop/src/agent/tools/registry/mod.rs` |
| Pagination helper | `src/desktop/src/agent/tools/registry/pagination.rs` |
| Built-in tool impls | `src/desktop/src/agent/tools/registry/builtin/*.rs` |
| LLM-facing strings (single source of truth) | `src/desktop/src/agent/tools/registry/builtin/strings/*.rs` |
| Tool input/output DTOs | `src/desktop/src/agent/tools/dtos.rs`, `src/desktop/src/agent/tools/csv_db/schema.rs` |
| Paging tests | `src/desktop/src/agent/tools/registry/tests.rs` |
| Tool executor (parallel/sequential) | `src/desktop/src/agent/tool_executor.rs` |
| System prompt | `src/desktop/src/agent/prompt_builder.rs` |
| Result presentation to the UI | `src/desktop/src/agent/response_formatter.rs` |
| Spec requirements (TOOL-001..013) | `src/desktop/src/agent/tools/SPEC.md` |

---

# Part II — Tool Paging Migration: `page`/`page_size` → `offset`/`limit` (and `search_email` → cursor)

Supersedes: TOOL-006, TOOL-007 (in `src/desktop/src/agent/tools/SPEC.md`)

> **Two classes, one PR.** Three list-paginated tools (`list_files`, `list_files_by_tag`, `web_fetch`) move to `offset`/`limit`. The fourth (`search_email`) moves to a cursor-based mechanism backed by a shared `ToolCache`. The shared cache also replaces the existing local `WEB_FETCH_CACHE`. See "Search email: cursor-based paging" below for the cursor spec.

## Context

The agent tool family has two paging idioms today (pre-migration):

- `page` + `page_size`, 1-indexed — used by `list_files`, `list_files_by_tag`, `search_email`.
- `offset` + `limit`, 0-indexed — used by `web_fetch`.

Each tool invents its own per-field description. The LLM has to learn two vocabularies and four parameter-name patterns. The audit in Part I flagged this as F-6.

The fix has two parts:

1. Migrate the three list-paginated tools (`list_files`, `list_files_by_tag`, `web_fetch`) to a shared `offset` + `limit` model. Write one canonical description block reused across all of them.
2. Switch `search_email` to a **cursor-based** paging model backed by a shared in-memory cache. This is a deliberate divergence — see "Search email: cursor-based paging" below for the rationale and full spec.

This plan covers the four tools that already page. The seven multi-result tools that have no paging at all (`search_calendar`, `get_calendar`, `get_calendar_item`, `search_contact`, `get_contact`, `get_weather`, `query` (csv)) are out of scope here and are tracked as a separate gap in the audit (F-1, F-7, F-9).

## Scope

| Item | Path |
|------|------|
| Pagination helper | `src/desktop/src/agent/tools/registry/pagination.rs` |
| Shared tool cache (new) | `src/desktop/src/agent/tools/registry/cache.rs` |
| Filesystem tool impls | `src/desktop/src/agent/tools/registry/builtin/fs.rs` |
| JMAP tool impl | `src/desktop/src/agent/tools/registry/builtin/jmap.rs` |
| JMAP email body | `src/desktop/src/agent/tools/jmap/email.rs` |
| Web fetch impl | `src/desktop/src/agent/tools/web.rs` (migrate from local `WEB_FETCH_CACHE` to shared cache) |
| Input/output DTOs | `src/desktop/src/agent/tools/dtos.rs` |
| LLM-facing strings | `src/desktop/src/agent/tools/registry/builtin/strings/{fs,jmap,web}.rs`, new `registry/builtin/strings/cursor.rs` |
| Tests | `src/desktop/src/agent/tools/registry/tests.rs`, `src/desktop/src/agent/tools/jmap/email.rs` (unit tests), new `src/desktop/src/agent/tools/registry/cache.rs` tests |
| Spec | `src/desktop/src/agent/tools/SPEC.md` |
| Tool list docstring | `src/desktop/src/agent/tools/SPEC.md` (LLM Tools Table) |

The `web_fetch` tool is already on `offset`/`limit`; this plan only updates its spec entry to use the canonical vocabulary and any description drift.

## Decision

### Two paging models, by tool class

The four paginated tools split into two classes. The list-paginated tools use a stateless `offset`/`limit` model. The search tool (`search_email`) uses a stateful cursor model backed by a shared in-memory cache. The split is deliberate — see "Search email: cursor-based paging" for the rationale.

| Class | Tools | Paging params | Paging state |
|-------|-------|---------------|--------------|
| Stateless list paging | `list_files`, `list_files_by_tag`, `web_fetch` | `offset`, `limit` | None — every call recomputes the slice |
| Stateful cursor paging | `search_email` | `cursor` (input + output) | Held in the shared `ToolCache` for 5 minutes |

### The model — stateless (list tools)

Every list-paginated tool MUST expose:

- `offset: Option<usize>` — number of items to skip from the start. 0-indexed. Default 0.
- `limit: Option<usize>` — number of items to return. Default per tool (see below).
- `total: usize` on the response — number of items across all pages. MUST always be present.
- `hint: Option<String>` on the response — human-readable message. Set when `total == 0` or `offset >= total`. `None` otherwise. MUST skip-serialize when `None`.

### Defaults — stateless (list tools)

| Tool | Default `limit` | Default `offset` |
|------|-----------------|------------------|
| `list_files` | 100 | 0 |
| `list_files_by_tag` | 100 | 0 |
| `web_fetch` | 100 (lines) | 0 |

The tool MUST NOT cap `limit`. If the LLM passes `limit: 100_000`, the tool returns up to 100_000 items (or `total`, whichever is smaller). The tool MUST still report the true `total` on every response, so the LLM can size follow-up requests based on what it has seen. The LLM owns the context-bloat tradeoff; the tool's job is to honor the request and report the truth.

### The model — stateful (search_email)

`search_email` uses a **cursor** instead of `offset`/`limit`. The full spec is in "Search email: cursor-based paging" below. Summary:

- Input: `cursor: Option<String>` — opaque token returned by a prior call. `None` on the first call.
- Output: `cursor: Option<String>` — opaque token for the next call. `None` when the result set is exhausted.
- Output: `total: usize` — total number of matching emails across all pages. Same on every page of the same search.
- Output: `hint: Option<String>` — human-readable message. Set to `"Final page."` when the cursor is absent. `None` otherwise.
- Page size: 100 emails per page. Fixed by the tool; the LLM does not control it.
- Server result set: cached in the shared `ToolCache` for 5 minutes (matches `web_fetch`'s TTL). Cursor and cache key are the same string.
- The cache MUST survive across tool calls. A subsequent call with the same `cursor` returns the next page from the cache without re-fetching from the server.

The cursor is opaque to the LLM. The LLM MUST pass back whatever the tool returned. The LLM MUST NOT attempt to parse or modify the cursor.

### Migration rules

#### Stateless (list tools: `list_files`, `list_files_by_tag`, `web_fetch`)

- The input field `page` MUST be removed (where it exists).
- The input field `page_size` MUST be removed (where it exists).
- The input field `offset` MUST be added with type `Option<usize>`.
- The input field `limit` MUST be added with type `Option<usize>`.
- The `serde(default)` and `JsonSchema` derives on the DTOs MUST be preserved.
- The `paginate_in_range` helper MUST change its parameter list from `(items, page, page_size, total, plural)` to `(items, offset, limit, total, plural)` and keep returning `(Vec<T>, Option<String>)`. The helper MUST NOT clamp `limit`.
- The hint text MUST change from `"... page {page} ..."` to `"... offset {offset} ..."`.
- The test file `registry/tests.rs` MUST be updated to use the new params. Test names SHOULD change from `test_list_by_tag_pagination_dispatch` to `test_list_by_tag_paging_dispatch` and from `test_list_by_tag_page_zero_is_normalised_to_page_one` to `test_list_by_tag_offset_zero_is_normalised` (or similar — see Test Changes).

#### Stateful (`search_email`)

- The input fields `page`, `page_size`, `offset`, and `limit` MUST be removed.
- The input field `cursor: Option<String>` MUST be added.
- The output field `cursor: Option<String>` MUST be added.
- The shared `ToolCache` MUST be introduced. The `search_email` impl MUST populate it on first call and consult it on subsequent calls.
- The 5-minute TTL MUST match the existing `web_fetch` cache TTL.
- A new module `src/desktop/src/agent/tools/registry/cache.rs` MUST hold the shared cache. The existing `WEB_FETCH_CACHE` in `web.rs` MUST be migrated into the shared cache as part of the same PR.

### Shared cache rules (apply to both `search_email` and `web_fetch`)

- The cache key for `search_email` MUST be the cursor string (generated by the helper as a UUID v4).
- The cache key for `web_fetch` MUST be the URL (replacing the existing `WEB_FETCH_CACHE` key).
- Cache entries MUST be evicted after 5 minutes (`CACHE_TTL`) on access (lazy eviction). A background sweep is NOT required.
- The cache MUST be process-local (no IPC). The desktop app is single-process; multi-process is out of scope.
- The cache MUST be `Mutex<HashMap<String, CacheEntry>>` wrapped in `LazyLock`. No `RwLock` (writes are frequent, contention is irrelevant for single-process).
- The cache MUST NOT grow without bound. A soft cap of 1024 entries SHOULD be enforced with FIFO eviction once exceeded. Reviewer: confirm.

### Constraints

- The migration MUST NOT change the underlying result ordering for any tool.
- The migration MUST NOT change the `truncated`/`total`/`hint` semantics; only the param names and the helper internals move.
- The `grep` tool's hard 200-match cap is unchanged. The truncation footer text MUST change from `"... at 200 matches ..."` to be derived from the constant. (Audit F-3; minor in this PR but cheap to land alongside.)
- The helper MUST NOT introduce a `MAX_PAGE_SIZE` constant or any other limit clamp. The LLM owns the page-size choice.
- The cursor mechanism for `search_email` MUST NOT change the underlying JMAP fetch — the helper MUST still fetch every matching email once and cache the result set. The cursor MUST NOT cause a second JMAP round-trip.

## Canonical vocabulary

The LLM sees the tool description and per-field strings through `registry/builtin/strings/`. The same wording MUST appear in every paginated tool. The text below is normative.

### Tool description (one paragraph, used in every paginated tool's `*_DESCRIPTION`)

> "Returns a paginated list. Use `offset` to skip items and `limit` to set the page size. The response includes `total` (item count across all pages) and `hint` (set to a message when the offset is past the end or there are no matches; absent otherwise)."

Rules:
- This paragraph MUST appear verbatim in every paginated tool's `*_DESCRIPTION` const.
- Each tool's description MUST add a single trailing sentence about its domain (e.g. "Defaults: `offset=0`, `limit=100`." for the fs tools).
- Description text MUST NOT exceed 60 words after the domain sentence is added.

### `offset` field string

Used on every `offset` parameter.

> "Number of items to skip from the start (0-indexed). Default 0."

### `limit` field string

Used on every `limit` parameter. The default value MUST be substituted per tool.

> "Number of items to return. Default {N}."

### `total` field string

Used on every `total` response field.

> "Number of items across all pages."

### `hint` field string

Used on every `hint` response field.

> "Set to a message when the offset is past the end of the result or there are no matches. Absent otherwise."

### Cursor description (used for `search_email` only)

`search_email` does NOT use `offset`/`limit`. It uses a cursor. The `*_DESCRIPTION` for `search_email` MUST use the paragraph below; the canonical offset/limit paragraph MUST NOT appear in `search_email`'s description.

> "Searches email by any combination of keyword, folder, date range, sender, recipient, unread, or flagged. Filters combine with AND. At least one filter MUST be provided. The first call returns up to 100 matching emails plus a `cursor`; pass the same `cursor` back in a follow-up call to get the next page. The full server result set is cached for 5 minutes. When the result set is exhausted, the response includes a `hint` and no `cursor`."

### `cursor` field string (input and output)

Used on both the input and output `cursor` fields of `search_email`. The same wording applies to both because the LLM passes back whatever the tool returned.

> "Opaque pagination token. Pass it back unchanged in a follow-up call to get the next page. Generated by the tool on the first call. Absent when the result set is exhausted."

### Strings to delete

The following consts MUST be removed from `strings/fs.rs` and `strings/jmap.rs` because the underlying fields no longer exist:

- `FIELD_LIST_FILES_BY_TAG_INPUT_PAGE`
- `FIELD_LIST_FILES_BY_TAG_INPUT_PAGE_SIZE`
- `FIELD_LIST_FILES_INPUT_PAGE`
- `FIELD_LIST_FILES_INPUT_PAGE_SIZE`
- `FIELD_SEARCH_EMAIL_INPUT_PAGE`
- `FIELD_SEARCH_EMAIL_INPUT_PAGE_SIZE`
- `FIELD_SEARCH_EMAIL_INPUT_KEYWORD` description text referring to "page/page_size" — replaced by the new cursor description.

The following consts MUST be added (or kept, with new wording) in their place:

- `FIELD_OFFSET_DESCRIPTION` (canonical "Number of items to skip…")
- `FIELD_LIMIT_DESCRIPTION` (canonical "Number of items to return…")
- `FIELD_TOTAL_DESCRIPTION` (canonical "Number of items across all pages.")
- `FIELD_HINT_DESCRIPTION` (canonical "Set to a message when…")
- `FIELD_CURSOR_DESCRIPTION` (canonical "Opaque pagination token…")

The first four MUST live in a new module `registry/builtin/strings/paging.rs` so every list-paginated tool family can `use super::super::strings::paging`. The cursor string MUST live in a new module `registry/builtin/strings/cursor.rs` for the same reason. This is the single source of truth.

### Per-tool domain sentences (added to the canonical paragraph)

| Tool | Paging class | Domain sentence |
|------|---------------|-----------------|
| `list_files` | offset/limit | "Lists Markdown files in a directory (non-recursive). With `path` set to `/` or `.` returns the configured content libraries. Defaults: `offset=0`, `limit=100`." |
| `list_files_by_tag` | offset/limit | "Lists Markdown files that contain the given tag in their front-matter. Defaults: `offset=0`, `limit=100`." |
| `web_fetch` | offset/limit | "Fetches the URL and converts HTML to Markdown. Cached for 5 minutes; pass `force_refetch=true` to bypass. Defaults: `offset=0` lines, `limit=100` lines. The response `total_lines` is the line count of the full Markdown body." |
| `search_email` | cursor | Use the cursor description block above. No offset/limit paragraph. |

Each combined description (canonical paragraph + domain sentence) MUST stay under 60 words.

## Per-tool changes

### `list_files` (`registry/builtin/fs.rs`, `dtos.rs`, `strings/fs.rs`)

**DTO** (`dtos.rs`):
```rust
pub struct ListFilesInput {
    pub path: String,
    #[schemars(description = strings::paging::FIELD_OFFSET_DESCRIPTION)]
    pub offset: Option<usize>,
    #[schemars(description = strings::paging::FIELD_LIMIT_DESCRIPTION)]
    pub limit: Option<usize>,
}
```

The `ListFilesResponse` keeps `files: Vec<String>`, `total: usize`, `hint: Option<String>`. Field descriptions switch to `strings::paging::FIELD_TOTAL_DESCRIPTION` and `strings::paging::FIELD_HINT_DESCRIPTION`.

**Tool impl** (`registry/builtin/fs.rs:225–262`):
- Replace `let page = input.page.unwrap_or(1).max(1);` with `let offset = input.offset.unwrap_or(0);`.
- Replace `let page_size = input.page_size.unwrap_or(DEFAULT_LIST_FILES_BY_TAG_PAGE_SIZE).max(1);` with `let limit = input.limit.unwrap_or(DEFAULT_LIST_FILES_BY_TAG_LIMIT);`.
- Call `paginate_in_range(&all_matches, offset, limit, total, plural)`.

**Strings** (`strings/fs.rs`):
- Delete `LIST_FILES_DESCRIPTION` and replace with the canonical paragraph + domain sentence above.
- Delete `FIELD_LIST_FILES_INPUT_PAGE`, `FIELD_LIST_FILES_INPUT_PAGE_SIZE`.
- Add `FIELD_LIST_FILES_INPUT_OFFSET = strings::paging::FIELD_OFFSET_DESCRIPTION` (re-export, for back-compat with any test that imports it).
- Add `FIELD_LIST_FILES_INPUT_LIMIT = strings::paging::FIELD_LIMIT_DESCRIPTION`.
- Switch `FIELD_LIST_FILES_RESPONSE_TOTAL` and `FIELD_LIST_FILES_RESPONSE_HINT` to point at the paging module.

### `list_files_by_tag` (same files)

Mirror of `list_files` changes. The default `limit` is also 100.

**Tool impl** (`registry/builtin/fs.rs:149–202`):
- Same `offset`/`limit` swap as above.
- Call `paginate_in_range(&all_matches, offset, limit, total, "tagged files")`.

### `search_email` (`registry/builtin/jmap.rs`, `jmap/email.rs`, `strings/jmap.rs`)

`search_email` uses the cursor mechanism, not `offset`/`limit`. See "Search email: cursor-based paging" below for the full design.

**DTO** (`dtos.rs:244–266`):
```rust
pub struct SearchEmailInput {
    #[schemars(description = strings::jmap::FIELD_SEARCH_EMAIL_INPUT_KEYWORD)]
    pub keyword: Option<String>,
    // ... other filter fields unchanged ...
    #[schemars(description = strings::cursor::FIELD_CURSOR_DESCRIPTION)]
    pub cursor: Option<String>,
}

pub struct SearchEmailResponse {
    pub results: String,
    #[schemars(description = strings::paging::FIELD_TOTAL_DESCRIPTION)]
    pub total: usize,
    #[schemars(description = strings::cursor::FIELD_CURSOR_DESCRIPTION)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[schemars(description = strings::paging::FIELD_HINT_DESCRIPTION)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
}
```

**Tool impl** (`registry/builtin/jmap.rs:33–56`):
- Drop the `let page = …; let page_size = …;` lines.
- Read `input.cursor`.
- Pass it through to `tool_search_email(..., SearchEmailCursor { cursor })`.
- Return shape MUST include the new `cursor` and `hint` fields.

**Body** (`jmap/email.rs`):
- Replace `SearchEmailPagination { page, page_size }` with `SearchEmailCursor { cursor: Option<String> }`.
- The fetch logic MUST consult the shared `ToolCache` for the cursor before going to JMAP.
- On cache miss with a non-None cursor, the helper MUST return an error `"Cursor expired or unknown; re-run the search with no cursor."` and the LLM will retry.
- On cache hit, the helper MUST slice the cached list at the cursor's recorded offset and return the next page. The next cursor is the same key; the offset advances.

**Strings** (`strings/jmap.rs`):
- Replace `SEARCH_EMAIL_DESCRIPTION` with the cursor description block.
- Delete `FIELD_SEARCH_EMAIL_INPUT_PAGE`, `FIELD_SEARCH_EMAIL_INPUT_PAGE_SIZE`.
- Add re-export shims for any test that imports them.
- Add `FIELD_SEARCH_EMAIL_RESPONSE_CURSOR = strings::cursor::FIELD_CURSOR_DESCRIPTION`.

### `web_fetch` (`registry/builtin/web.rs`, `web.rs`, `strings/web.rs`)

The `web_fetch` DTO is unchanged (it already uses `offset`/`limit`). The string update and the cache migration are the changes:

- `WEB_FETCH_DESCRIPTION` MUST use the canonical paragraph + the `web_fetch` domain sentence.
- The description MUST add the line-count default (`limit` is in lines, not items).
- `web.rs:215` (the inline tool definition passed to the sub-agent in `tool_web_delegate`) MUST use the same wording. That keeps `web_delegate`'s sub-agent in sync.
- The local `static WEB_FETCH_CACHE: LazyLock<Mutex<HashMap<String, CacheEntry>>>` in `web.rs:14–15` MUST be deleted. The `tool_web_fetch` function MUST consult the shared `ToolCache` instead. The `force_refetch` parameter MUST clear the cache entry, not bypass it. The 5-minute TTL MUST match the shared cache's TTL exactly.

## Search email: cursor-based paging

This section is the normative spec for `search_email`'s cursor mechanism. It supersedes the offset/limit migration for `search_email` only; the other three paginated tools stay on `offset`/`limit`.

### Rationale

`search_email` is the only paginated tool that fetches from a remote server (JMAP). The other three are local filesystem or HTTP-cache reads. For local reads, the cost of a stateless `(offset, limit)` request is small — the helper just slices a `Vec`. For JMAP, the cost of the `Email/query` + `Email/get` round-trip is large (one round-trip per email; hundreds of round-trips for a 100-result search), so we want to fetch once and slice from memory.

A cursor mechanism backed by a shared in-memory cache gives the LLM the same overall interface (one call returns a page; the next call returns the next page) without re-querying the server.

### Mechanism

1. **First call** — LLM passes filter set, no cursor.
   - Helper computes a hash of the filter set (a stable, deterministic representation).
   - Helper looks up the shared `ToolCache` by hash.
   - On cache miss: helper queries JMAP for every matching email ID, fetches each email body, simplifies it, and stores the full list in the cache keyed by the hash. Cache TTL is 5 minutes.
   - Helper generates a cursor string (UUID v4). Cursor == cache key.
   - Helper returns the first 100 items plus the cursor.
2. **Subsequent call** — LLM passes filter set + cursor.
   - Helper looks up the cache by cursor.
   - On cache hit: helper reads the recorded offset (initially 0), returns items `[offset..offset+100]`, advances the recorded offset by the number of items returned.
   - On cache miss (entry evicted or TTL expired): helper returns an error `"Cursor expired or unknown; re-run the search with no cursor."` The LLM is expected to retry with no cursor.
3. **Final page** — cursor offset reaches the end of the list.
   - Helper returns the remaining items, records the new offset as `total`, and does NOT generate a new cursor. The response includes `hint: "Final page."`.

### Cursor string format

- Format: a UUID v4 string, e.g. `"550e8400-e29b-41d4-a716-446655440000"`.
- The cursor is opaque to the LLM. It MUST be passed back unchanged.
- The cursor is also the cache key. The LLM has no other way to address a cache entry.

### Cache entry shape

```rust
pub struct SearchEmailCacheEntry {
    /// The full server result set, in the order the server returned.
    pub items: Vec<SearchEmailItem>,
    /// The number of items already returned to the LLM. The next call
    /// returns items starting at this offset.
    pub cursor_offset: usize,
    /// Total number of items in the result set, captured at first fetch.
    pub total: usize,
    /// Insertion time, for TTL eviction.
    pub fetched_at: Instant,
    /// Per-client error messages collected during the first fetch.
    pub errors: Vec<String>,
}

pub struct SearchEmailItem {
    pub client: String,
    pub email: serde_json::Value,
}
```

### Shared cache module

New module `src/desktop/src/agent/tools/registry/cache.rs`:

```rust
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::{Duration, Instant};

/// TTL for cache entries. MUST match the previous `web_fetch` cache TTL.
pub const CACHE_TTL: Duration = Duration::from_secs(300);

/// Soft cap on cache entries. Enforced FIFO once exceeded.
pub const MAX_CACHE_ENTRIES: usize = 1024;

pub struct ToolCache {
    inner: Mutex<HashMap<String, CacheEntry>>,
}

pub enum CacheEntry {
    WebFetch {
        content: String,
        response_headers: HashMap<String, String>,
        fetched_at: Instant,
    },
    SearchEmail(SearchEmailCacheEntry),
}

static TOOL_CACHE: LazyLock<ToolCache> = LazyLock::new(|| ToolCache {
    inner: Mutex::new(HashMap::new()),
});

impl ToolCache {
    /// Get a clone of the entry if it exists and is not expired.
    pub fn get(&self, key: &str) -> Option<CacheEntry> { /* ... */ }

    /// Insert or replace the entry under the given key.
    pub fn put(&self, key: String, value: CacheEntry) { /* ... */ }

    /// Remove the entry under the given key.
    pub fn invalidate(&self, key: &str) { /* ... */ }

    /// Evict entries older than `CACHE_TTL` and enforce `MAX_CACHE_ENTRIES` with FIFO.
    pub fn evict_expired(&self) { /* ... */ }
}

pub fn cache() -> &'static ToolCache { &TOOL_CACHE }
```

### Response shape for `search_email`

```jsonc
// First call (no cursor in input)
{
  "status": "success",
  "data": {
    "results": "[{...}, {...}, ...]",   // up to 100 emails
    "total": 247,                       // total across all pages
    "cursor": "550e8400-e29b-41d4-..."  // present when more pages exist
    // "hint" absent
  }
}

// Subsequent call (cursor in input)
{
  "status": "success",
  "data": {
    "results": "[{...}, ...]",          // next 100 emails
    "total": 247,                       // same total
    "cursor": "550e8400-e29b-41d4-..."  // same key; offset advanced
    // "hint" absent
  }
}

// Final page (cursor offset reached end)
{
  "status": "success",
  "data": {
    "results": "[{...}, ...]",          // remaining items
    "total": 247,                       // same total
    // "cursor" absent
    "hint": "Final page."
  }
}

// Cache miss on cursor (expired or evicted)
{
  "status": "error",
  "message": "Cursor expired or unknown; re-run the search with no cursor."
}
```

### Behavior invariants

- A `search_email` call with a cursor MUST NOT trigger a JMAP round-trip if the cache hit succeeds.
- A `search_email` call with no cursor MUST trigger at most one JMAP round-trip per matching email (the existing behavior). The cache is populated as a side effect.
- The `total` field MUST be identical across all pages of the same search (same cache entry).
- The cursor offset MUST be a multiple of 100 on entry, modulo the final page. The helper MUST NOT serve a partial first page.
- The cache MUST be evicted on entry access if older than `CACHE_TTL`. A background sweeper is NOT required.
- The `force_refetch` style parameter does NOT exist for `search_email` (no DTO change for invalidation). If the LLM wants a fresh fetch, it MUST omit the cursor.

## Helper refactor

`registry/pagination.rs` becomes:

```rust
//! Paginator helper for tool results.

/// Default `limit` for `list_files` and `list_files_by_tag`.
pub const DEFAULT_LIST_FILES_BY_TAG_LIMIT: usize = 100;

/// Default `limit` for `search_email`.
pub const DEFAULT_SEARCH_EMAIL_LIMIT: usize = 10;

pub fn paginate_in_range<T: Clone>(
    items: &[T],
    offset: usize,
    limit: usize,
    total: usize,
    plural: &str,
) -> (Vec<T>, Option<String>) {
    let offset = offset.min(total);
    if total == 0 {
        return (Vec::new(), Some(format!("No matching {plural} found.")));
    }
    if offset >= total {
        return (
            Vec::new(),
            Some(format!(
                "No {plural} at offset {offset} (showing 0 of {total} total, limit: {limit})."
            )),
        );
    }
    let end = (offset + limit).min(total);
    (items[offset..end].to_vec(), None)
}
```

Notes:
- The helper MUST NOT cap `limit`. The end-of-slice clamp `(offset + limit).min(total)` is a bound check, not a cap.
- The two new `DEFAULT_*_LIMIT` consts MUST live next to the helper so tests can import them.
- The existing test `registry/tests.rs:573` (which asserts `DEFAULT_LIST_FILES_BY_TAG_PAGE_SIZE == 20`) MUST move to assert the new const value (`DEFAULT_LIST_FILES_BY_TAG_LIMIT == 100`).

## Spec changes (`src/desktop/src/agent/tools/SPEC.md`)

| Existing ID | Action | New text |
|-------------|--------|----------|
| TOOL-006 | REPLACE | "Web Fetch Pagination: The `web_fetch` tool MUST accept `offset` (default 0) and `limit` (default 100) integer parameters. `offset` is the number of Markdown lines to skip. `limit` is the number of Markdown lines to return." |
| TOOL-007 | REPLACE | "Pagination Total: Every list-paginated tool (`list_files`, `list_files_by_tag`, `web_fetch`) MUST return a `total` field on its response. The value MUST be the item count across all pages. `search_email` MUST also return `total`; the value MUST be the item count across all pages of the same search and MUST be identical across all pages." |
| — | ADD (TOOL-014) | "Pagination Hint: Every paginated tool MUST return a `hint` field on its response. For list-paginated tools, `hint` MUST be set to a human-readable message when `total == 0` or `offset >= total`. For `search_email`, `hint` MUST be set to `'Final page.'` when the response has no `cursor`. `hint` MUST be absent (or `null`) otherwise." |
| — | ADD (TOOL-015) | "Pagination Defaults: `list_files` and `list_files_by_tag` MUST default `offset=0`, `limit=100`. `web_fetch` MUST default `offset=0` (lines), `limit=100` (lines). `search_email` does not use `offset`/`limit`; see TOOL-018." |
| — | ADD (TOOL-016) | "No Pagination Cap: List-paginated tools MUST NOT cap `limit`. If the LLM requests more items than exist, the tool returns all remaining items starting at `offset` and reports the true `total` on the response. The LLM is responsible for choosing a `limit` that fits its context window; the tool's job is to honor the request and report the truth. `search_email` is not subject to this requirement because the LLM does not control the page size." |
| — | ADD (TOOL-017) | "Pagination Vocabulary: List-paginated tools MUST use the parameter names `offset` and `limit` and the response field names `total` and `hint`. The names `page` and `page_size` MUST NOT appear in any tool schema. `search_email` is exempt from this rule; it uses the cursor parameter and response field per TOOL-018." |
| — | ADD (TOOL-018) | "Search Email Cursor: The `search_email` tool MUST accept a `cursor: Option<String>` input parameter and return a `cursor: Option<String>` output field. The first call (no cursor in input) returns up to 100 matching emails plus a new cursor. Subsequent calls with the same cursor return the next 100 emails (or fewer on the final page). The cursor is opaque and MUST be passed back unchanged. When the result set is exhausted, the response includes a `hint` and no `cursor`. The page size is fixed at 100; the LLM does not control it." |
| — | ADD (TOOL-019) | "Shared Tool Cache: A process-local `ToolCache` MUST be shared by `search_email` and `web_fetch`. The cache MUST be `Mutex<HashMap<String, CacheEntry>>` wrapped in `LazyLock`. Cache entries MUST be evicted lazily on access after 5 minutes. A soft cap of 1024 entries MUST be enforced with FIFO eviction once exceeded. The cache MUST NOT be persisted across process restarts." |
| — | ADD (TOOL-020) | "Search Email Cache Population: The first `search_email` call with a given filter set MUST populate the cache with the full server result set. Subsequent calls with the matching cursor MUST slice from the cache without re-fetching. A `search_email` call with a cursor that does not match a live cache entry MUST return the error `'Cursor expired or unknown; re-run the search with no cursor.'`" |
| — | ADD (TOOL-021) | "Web Fetch Cache Migration: The `web_fetch` tool MUST move its cache from a process-local `LazyLock<Mutex<HashMap<String, _>>>` in `web.rs` to the shared `ToolCache`. The URL MUST be the cache key. The `force_refetch` parameter MUST clear the cache entry before re-fetching. The 5-minute TTL MUST match the shared cache's TTL." |
| TOOL-010 | UPDATE | Replace "fetching a page once and issuing partial reads via `limit` and `offset` to paginate through the content" with wording that uses the canonical vocabulary block. (Functionally identical, but the description must use the same words.) |

The LLM Tools Table at the top of the SPEC MUST be updated to use `offset`/`limit` for the four paginated tools.

## Test changes

`registry/tests.rs` needs the following updates.

| Test name (current) | Test name (new) | Param swap |
|---------------------|------------------|------------|
| `test_list_by_tag_default_page_size_is_20` | `test_list_by_tag_default_limit_is_100` | drop params; the test MUST create 150 files so the default actually clips the response to 100 |
| `test_list_by_tag_pagination_dispatch` | `test_list_by_tag_paging_dispatch` | `page` → `offset`, `page_size` → `limit`; the test MUST keep its explicit `page_size: 20` cases (they exercise the `page_size` param, not the default) |
| `test_list_by_tag_pagination_is_global_across_libraries` | `test_list_by_tag_paging_is_global_across_libraries` | `page` → `offset`, `page_size` → `limit` |
| `test_list_by_tag_no_matches_reports_zero_total` | unchanged | unchanged |
| `test_list_by_tag_page_zero_is_normalised_to_page_one` | `test_list_by_tag_offset_zero_returns_first_page` | use `offset: 0` (no normalization needed) |
| `test_list_files_default_page_size_is_20` | `test_list_files_default_limit_is_100` | drop params; the test MUST create 150 files so the default actually clips the response to 100 |
| `test_list_files_pagination_dispatch` | `test_list_files_paging_dispatch` | `page` → `offset`, `page_size` → `limit`; the test MUST keep its explicit `page_size: 20` cases (they exercise the `page_size` param, not the default) |
| `test_list_files_root_path_returns_libraries` | unchanged | unchanged |
| `test_list_files_multiple_libraries_paginated_globally` | `test_list_files_multiple_libraries_paging_global` | `page` → `offset`, `page_size` → `limit` |
| `test_list_files_returns_json_array_not_string` | unchanged | unchanged |
| `test_grep_*` | unchanged (grep does not page) | n/a |
| `test_csv_tools_in_schema` / `test_csv_tools_excluded` | unchanged | n/a |
| `test_get_weather_tool_*` | unchanged | n/a |

New tests to add:

- `test_list_by_tag_honors_large_limit` — pass `limit: 10_000` against a small fixture, expect all items back (no clamping). The test SHOULD also assert `total` matches `len(files)`.
- `test_list_files_offset_past_end_returns_hint` — pass `offset: 999`, expect empty `files` and a `hint` containing the word "offset".
- `test_list_files_offset_zero_is_not_normalised` — pass `offset: 0`, expect the first `limit` items (this is the natural meaning; the old "page 0 normalises to page 1" rule goes away).

For the `SearchEmailInput` path, the email body tests at `jmap/email.rs:1130+` are integration-style and use `simplify_jmap_emails`, which does not touch pagination. The cursor mechanism is new and needs its own test coverage. Add these tests in `src/desktop/src/agent/tools/jmap/email.rs` (or in a new `cache.rs` test module — Reviewer: please confirm placement):

- `test_search_email_first_call_returns_cursor` — call `tool_search_email` with no cursor; assert response has a `cursor: String` and the first 100 items.
- `test_search_email_subsequent_call_advances_offset` — call with no cursor, then call with the returned cursor; assert second response has different items and the same `total`.
- `test_search_email_final_page_has_no_cursor_and_hint` — populate a cache entry with 150 items, advance the cursor offset to 100 via two calls, make a third call; assert the response has no `cursor` and `hint == "Final page."`.
- `test_search_email_cache_hit_avoids_jmap_round_trip` — mock the JMAP client to count calls; make two `search_email` calls; assert the second call did not trigger a new `Email/query` or `Email/get`.
- `test_search_email_expired_cursor_returns_error` — populate a cache entry, manually expire it (set `fetched_at` to a time older than `CACHE_TTL`), call with the cursor; assert the response is an error with the expected message.
- `test_search_email_different_filter_sets_different_cursors` — make a call with `keyword: "foo"`, then a call with `keyword: "bar"`; assert the cursors differ and the second call did not hit the first cache entry.

Add these tests in `src/desktop/src/agent/tools/registry/cache.rs` (or `web.rs` if the migration is staged):

- `test_web_fetch_uses_shared_cache` — call `tool_web_fetch` twice with the same URL; assert the second call does not hit the network.
- `test_web_fetch_force_refetch_clears_cache_entry` — call `tool_web_fetch`, call again with `force_refetch: true`; assert the second call cleared and re-populated the cache entry.
- `test_cache_eviction_after_ttl` — insert an entry, set `fetched_at` to old, call `get`; assert the entry is gone.
- `test_cache_max_entries_fifo_eviction` — insert 1025 entries; assert the oldest is gone.

The hard-coded "200" in the grep truncation footer (audit F-3) MUST also be fixed in the same PR: the literal MUST become `format!("...at {} matches...", DEFAULT_GREP_MAX_RESULTS)`. Add a test that asserts the constant and the footer text agree.

## Quality gate

Per `src/desktop/AGENTS.md` §6, the following MUST all pass before the PR is done:

- `cargo check` — clean, no warnings.
- `cargo nextest run` — all tests pass.
- `cargo clippy -- -D warnings` — clean.
- `cargo fmt --check` — clean.
- `cargo doc --no-deps --quiet` — clean.

Additional checks specific to this change:

- `rg -n '\bpage_size\b|\bpage:\b|\bpage =' src/desktop/src/agent/tools` MUST return no hits in code paths. (Test names may still contain the word "page" in their natural English meaning; that is fine.)
- `rg -n 'page_size|"page"|"page_size"' src/desktop/src/agent/tools` MUST return no hits at all.
- A live schema probe (via `get_tools_schema` on a sample config) MUST show `offset` and `limit` in the parameter list for the three list-paginated tools and MUST show `cursor` (not `page` or `page_size`) in the `search_email` parameter list.
- The local `WEB_FETCH_CACHE` static MUST be deleted from `web.rs` and all references to it MUST resolve to `ToolCache::cache()`.
- A live trace check (via `tracing` events) MUST show exactly one JMAP `Email/query` + `Email/get` round-trip for the first `search_email` call, and zero round-trips for the second call with a cursor.

## Risks

- **Caller break.** Any LLM that has learned the old `page`/`page_size` parameter names from past conversations will now produce invalid tool calls. The error path is "invalid args", which is already surfaced to the model. The model SHOULD recover on the next turn. If a smoother migration is needed, add a brief period during which the executor accepts both `page` and `offset` (with `page` taking precedence if both are present) and emits a one-time warning. This is RECOMMENDED for the first release after the migration and SHOULD be removed in the release after that. The same shim does NOT apply to `search_email`'s cursor — the old `page`/`page_size` is replaced by a fundamentally different model.
- **MCP tool confusion.** MCP-sourced tools (via `McpToolAdapter`) keep whatever schema the upstream server advertises. A server that uses `page`/`page_size` will look different from our built-ins. This is a known consequence of MCP's "be the server's schema" contract and is out of scope.
- **Test count.** `registry/tests.rs` has ~25 tests touching `list_files` / `list_files_by_tag`. A mass rename is a one-shot risk; reviewers SHOULD skim the diff to confirm only param names changed.
- **Stale docs.** Any external doc that references `page`/`page_size` (user-facing help, blog posts, `wiki/architecture/architecture-summary.md`) MUST be updated. The PR description MUST list a "docs to update" section.
- **No cap means the LLM owns the context-bloat tradeoff.** If the LLM asks for `limit: 100_000` and the result has 100_000 items, the tool returns all of them. The `total` field on every response lets the LLM size follow-up requests based on what it has seen. The recommended pattern is: start with the default `limit`, then re-page using the returned `total` to choose a `limit` that fits the context. The system prompt SHOULD NOT need to remind the LLM of this — the canonical description tells it.
- **Cursor caching is a state machine.** The shared `ToolCache` holds state across tool calls. A bug that returns the wrong slice, advances the wrong offset, or fails to evict can cause silent data loss or duplication. The cache is also a single-process `Mutex<HashMap>` — if the agent ever becomes multi-process, the cache will need a redesign.
- **Cache memory footprint.** A `search_email` call with 10,000 matches will cache 10,000 simplified email objects in memory for 5 minutes. The soft cap of 1024 entries caps the total cache, but a single entry can still be large. A per-entry size cap SHOULD be added. Reviewer: confirm. For reference, a typical simplified email is 2–5 KB, so 10,000 emails ≈ 20–50 MB.
- **5-minute TTL is invisible to the LLM.** If the LLM pauses for 6 minutes between two cursor calls, the second call returns an error and the LLM must retry. The error message tells it to retry, but the LLM may not understand the cause.
- **No cross-process or cross-restart cache.** The cache is process-local. A `force_refetch` style parameter does NOT exist for `search_email`. If the user closes and reopens the app, the cursor is invalidated and the LLM must re-run.

## Out of scope

- Adding paging to the seven multi-result tools that lack it (audit F-1, F-7, F-9).
- Adding a `next_offset` or `has_next` field to the list-paginated tools' responses. The cursor mechanism covers `search_email`; the list-paginated tools rely on `offset`/`total` math.
- Adding a `body_truncated` field to `search_email` results (audit F-5). Open question #6 asks whether to address F-5 in the same PR; current proposal is no.
- Refactoring `CalDavResponse` / `CardDavResponse` from a JSON-stringified blob into a structured DTO.
- The `web_search` paging story (audit F-2). The audit's recommended pattern is now "cursor with shared cache," same as `search_email`.

## Open questions

1. Should `paginate_in_range` clamp `offset` to `total` (current proposal) or pass it through and let the LLM see an empty result with no hint? The current proposal is "clamp and emit a hint." Reviewer: please confirm.
2. Should we land the dual-accept `page`/`offset` shim for one release? Reviewer: please confirm.
3. Should the shared cache live in a new `registry/cache.rs` module, or in `registry/pagination.rs`? Current proposal: new module. Reviewer: please confirm.
4. Should the cursor be a UUID v4 string, or some shorter opaque token (e.g. base64-encoded hash)? Current proposal: UUID v4 for debuggability. Reviewer: please confirm.
5. Should the per-entry size cap (see "Cache memory footprint" in Risks) be a hard limit (refuse to cache) or a soft warning? Current proposal: hard cap of 10,000 items per entry; refuse and return an error if exceeded. Reviewer: please confirm.
6. Should `search_email`'s `simplify_email` body truncation (audit F-5) be addressed in the same PR? Current proposal: out of scope. Reviewer: please confirm.
