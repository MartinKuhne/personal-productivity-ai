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
| F-10 | `list_files`/`list_files_by_tag` accept a `page_size` with **no documented upper bound**; the LLM can ask for 10,000 rows in a single page | **Low** | `list_files`, `list_files_by_tag` |
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
| `page` + `page_size` | `list_files`, `list_files_by_tag`, `search_email` | 1-indexed `page` | 20 (fs) / 10 (email) |
| `offset` + `limit` | `web_fetch` | 0-indexed `offset`; `limit` is the line count | offset 0, limit 100 |
| hard cap (no paging) | `grep` (200), `read_tags` (none), `web_search` (server), all CalDAV/CardDAV search, all CSV search, `get_weather`, `query` | — | — |

A unified convention would help the LLM. The LLM-facing description should at minimum state which idiom applies. Today:

- `web_fetch` description says "limit/offset" — ok.
- `list_files` / `list_files_by_tag` / `search_email` use `page` / `page_size` and the per-field strings mention "1-indexed page number. Defaults to 1" — ok.
- No tool explains *why* two idioms exist.

This is not a blocker, but it adds friction and is the kind of thing an LLM will quietly get wrong.

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

Ranked by LLM-helpfulness:

1. `search_email` — the gold standard. Mentions all filter fields, AND-semantics, the "at least one filter" rule, pagination defaults, and where `total` is in the response.
2. `list_files`, `list_files_by_tag` — explicit about page, page_size, total, hint; says "default 20".
3. `grep` — explicit about the 200 cap and what to do when truncated.
4. `web_fetch` — explicit about limit/offset and total_lines, but does not say the default limit is 100.
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
| `search_email` | `hint: Option<String>` only when past end | ok for the page case; **no signal for per-email body truncation** — F-5 |
| `web_fetch` | `total_lines: usize` always | yes; combined with the formatted-result line in `response_formatter.rs:108–127` that says "X of Y markdown lines returned" |
| `web_search` | none | n/a (no cap) |
| `get_weather` | none | n/a (~14 items) |
| CalDAV/CardDAV/CSV | none | n/a (no cap) |

### MCP tools (out of scope, but worth noting)

`McpToolAdapter` (in `mcp/mod.rs`) passes the server's schema straight through. If the upstream MCP server declares `cursor`/`nextCursor` or `offset`/`limit` params, the LLM will see them in the schema; if not, the LLM has no paging affordance. The adapter does **not** synthesize or wrap paging params. That is the correct behavior for an MCP client (don't lie about what the server supports), but it means the agent loop has no uniform contract for MCP tools and the LLM has to read each server's schema.

### Dead code (F-11)

`src/desktop/src/agent/tools/dtos.rs:298–304` defines `DeleteEmailInput` and `DeleteEmailResponse`, but `register_all_builtins` in `registry/builtin/mod.rs:21–54` does not register a `delete_email` tool. The struct, derive, and JsonSchema impls are unused. Either delete the DTOs or register a tool — leaving it dead is a small but real maintenance trap (any future `delete_email` reference will assume the wiring is there).

## Paging semantics summary (the matrix the LLM needs)

| Tool | Schema params | Indexing | Default | Cap / Total field | Hint when past end |
|------|---------------|----------|---------|-------------------|---------------------|
| `grep` | none (cap only) | n/a | first 200 matches | `total: usize` (count of all matches) + `truncated: bool` | n/a (no paging) |
| `list_files` | `page: Option<usize>`, `page_size: Option<usize>` | 1-indexed | 20 | `total: usize` (cross-library) | `hint: Option<String>` |
| `list_files_by_tag` | `page: Option<usize>`, `page_size: Option<usize>` | 1-indexed | 20 | `total: usize` (cross-library) | `hint: Option<String>` |
| `search_email` | `page: Option<usize>`, `page_size: Option<usize>` | 1-indexed | 10 | `total: usize` (cross-client) | `hint: Option<String>` |
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
   Either way, the `web_search` description needs to stop being a one-liner. — *Status: out of scope for Part II.*
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
- Internal pagination inside `csv_db::query::query_csv` (single-shot is fine; the cap belongs on the tool layer).
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

# Part II — Tool Paging Migration: `page`/`page_size` → `offset`/`limit`

Supersedes: TOOL-006, TOOL-007 (in `src/desktop/src/agent/tools/SPEC.md`)

## Context

The agent tool family has two paging idioms today:

- `page` + `page_size`, 1-indexed — used by `list_files`, `list_files_by_tag`, `search_email`.
- `offset` + `limit`, 0-indexed — used by `web_fetch`.

Each tool invents its own per-field description. The LLM has to learn two vocabularies and four parameter-name patterns. The audit in Part I flagged this as F-6.

The fix: migrate every paginated tool to `offset` + `limit` and write one canonical description block reused across all of them.

This plan covers only the four tools that already page. The seven multi-result tools that have no paging at all (`search_calendar`, `get_calendar`, `get_calendar_item`, `search_contact`, `get_contact`, `get_weather`, `query` (csv)) are out of scope here and are tracked as a separate gap in the audit (F-1, F-7, F-9).

## Scope

| Item | Path |
|------|------|
| Pagination helper | `src/desktop/src/agent/tools/registry/pagination.rs` |
| Filesystem tool impls | `src/desktop/src/agent/tools/registry/builtin/fs.rs` |
| JMAP tool impl | `src/desktop/src/agent/tools/registry/builtin/jmap.rs` |
| JMAP email body | `src/desktop/src/agent/tools/jmap/email.rs` |
| Input/output DTOs | `src/desktop/src/agent/tools/dtos.rs` |
| LLM-facing strings | `src/desktop/src/agent/tools/registry/builtin/strings/{fs,jmap}.rs` |
| Tests | `src/desktop/src/agent/tools/registry/tests.rs` |
| Spec | `src/desktop/src/agent/tools/SPEC.md` |
| Tool list docstring | `src/desktop/src/agent/tools/SPEC.md` (LLM Tools Table) |

The `web_fetch` tool is already on `offset`/`limit`; this plan only updates its spec entry to use the canonical vocabulary and any description drift.

## Decision

### The model

Every paginated tool MUST expose:

- `offset: Option<usize>` — number of items to skip from the start. 0-indexed. Default 0.
- `limit: Option<usize>` — number of items to return. Default per tool (see below).
- `total: usize` on the response — number of items across all pages. MUST always be present.
- `hint: Option<String>` on the response — human-readable message. Set when `total == 0` or `offset >= total`. `None` otherwise. MUST skip-serialize when `None`.

### Defaults per tool

| Tool | Default `limit` | Default `offset` |
|------|-----------------|------------------|
| `list_files` | 20 | 0 |
| `list_files_by_tag` | 20 | 0 |
| `search_email` | 10 | 0 |
| `web_fetch` | 100 (lines) | 0 |

The tool MUST NOT cap `limit`. If the LLM passes `limit: 100_000`, the tool returns up to 100_000 items (or `total`, whichever is smaller). The tool MUST still report the true `total` on every response, so the LLM can size follow-up requests based on what it has seen. The LLM owns the context-bloat tradeoff; the tool's job is to honor the request and report the truth.

### Migration rules

- The input field `page` MUST be removed.
- The input field `page_size` MUST be removed.
- The input field `offset` MUST be added with type `Option<usize>`.
- The input field `limit` MUST be added with type `Option<usize>`.
- The `serde(default)` and `JsonSchema` derives on the DTOs MUST be preserved.
- The `paginate_in_range` helper MUST change its parameter list from `(items, page, page_size, total, plural)` to `(items, offset, limit, total, plural)` and keep returning `(Vec<T>, Option<String>)`. The helper MUST NOT clamp `limit`.
- The hint text MUST change from `"... page {page} ..."` to `"... offset {offset} ..."`.
- The test file `registry/tests.rs` MUST be updated to use the new params. Test names SHOULD change from `test_list_by_tag_pagination_dispatch` to `test_list_by_tag_paging_dispatch` and from `test_list_by_tag_page_zero_is_normalised_to_page_one` to `test_list_by_tag_offset_zero_is_normalised` (or similar — see Test Changes).

### Constraints

- The migration MUST NOT change the underlying result ordering for any tool.
- The migration MUST NOT change the `truncated`/`total`/`hint` semantics; only the param names and the helper internals move.
- The `grep` tool's hard 200-match cap is unchanged. The truncation footer text MUST change from `"... at 200 matches ..."` to be derived from the constant. (Audit F-3; minor in this PR but cheap to land alongside.)
- The helper MUST NOT introduce a `MAX_PAGE_SIZE` constant or any other limit clamp. The LLM owns the page-size choice.

## Canonical vocabulary

The LLM sees the tool description and per-field strings through `registry/builtin/strings/`. The same wording MUST appear in every paginated tool. The text below is normative.

### Tool description (one paragraph, used in every paginated tool's `*_DESCRIPTION`)

> "Returns a paginated list. Use `offset` to skip items and `limit` to set the page size. The response includes `total` (item count across all pages) and `hint` (set to a message when the offset is past the end or there are no matches; absent otherwise)."

Rules:
- This paragraph MUST appear verbatim in every paginated tool's `*_DESCRIPTION` const.
- Each tool's description MUST add a single trailing sentence about its domain (e.g. "Defaults: `offset=0`, `limit=20`." for the fs tools).
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

### Strings to delete

The following consts MUST be removed from `strings/fs.rs` and `strings/jmap.rs` because the underlying fields no longer exist:

- `FIELD_LIST_FILES_BY_TAG_INPUT_PAGE`
- `FIELD_LIST_FILES_BY_TAG_INPUT_PAGE_SIZE`
- `FIELD_LIST_FILES_INPUT_PAGE`
- `FIELD_LIST_FILES_INPUT_PAGE_SIZE`
- `FIELD_SEARCH_EMAIL_INPUT_PAGE`
- `FIELD_SEARCH_EMAIL_INPUT_PAGE_SIZE`

The following consts MUST be added (or kept, with new wording) in their place:

- `FIELD_OFFSET_DESCRIPTION` (canonical "Number of items to skip…")
- `FIELD_LIMIT_DESCRIPTION` (canonical "Number of items to return…")
- `FIELD_TOTAL_DESCRIPTION` (canonical "Number of items across all pages.")
- `FIELD_HINT_DESCRIPTION` (canonical "Set to a message when…")

These MUST live in a new module `registry/builtin/strings/paging.rs` so every tool family can `use super::super::strings::paging`. This is the single source of truth.

### Per-tool domain sentences (added to the canonical paragraph)

| Tool | Domain sentence |
|------|-----------------|
| `list_files` | "Lists Markdown files in a directory (non-recursive). With `path` set to `/` or `.` returns the configured content libraries. Defaults: `offset=0`, `limit=20`." |
| `list_files_by_tag` | "Lists Markdown files that contain the given tag in their front-matter. Defaults: `offset=0`, `limit=20`." |
| `search_email` | "Searches email by any combination of keyword, folder, date range, sender, recipient, unread, or flagged. Filters combine with AND. At least one filter MUST be provided. Defaults: `offset=0`, `limit=10`." |
| `web_fetch` | "Fetches the URL and converts HTML to Markdown. Cached for 5 minutes; pass `force_refetch=true` to bypass. Defaults: `offset=0` lines, `limit=100` lines. The response `total_lines` is the line count of the full Markdown body." |

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

Mirror of `list_files` changes. The default `limit` is also 20.

**Tool impl** (`registry/builtin/fs.rs:149–202`):
- Same `offset`/`limit` swap as above.
- Call `paginate_in_range(&all_matches, offset, limit, total, "tagged files")`.

### `search_email` (`registry/builtin/jmap.rs`, `jmap/email.rs`, `strings/jmap.rs`)

**DTO** (`dtos.rs:244–266`):
```rust
pub struct SearchEmailInput {
    #[schemars(description = strings::jmap::FIELD_SEARCH_EMAIL_INPUT_KEYWORD)]
    pub keyword: Option<String>,
    // ... other filter fields unchanged ...
    #[schemars(description = strings::paging::FIELD_OFFSET_DESCRIPTION)]
    pub offset: Option<usize>,
    #[schemars(description = strings::paging::FIELD_LIMIT_DESCRIPTION)]
    pub limit: Option<usize>,
}
```

**Tool impl** (`registry/builtin/jmap.rs:33–56`):
- Replace `let page = input.page.unwrap_or(1).max(1);` with `let offset = input.offset.unwrap_or(0);`.
- Replace `let page_size = input.page_size.unwrap_or(10).max(1);` with `let limit = input.limit.unwrap_or(DEFAULT_SEARCH_EMAIL_LIMIT);`.
- Pass through to `tool_search_email(..., SearchEmailPagination { offset, limit })`.

**Body** (`jmap/email.rs:235–248`):
```rust
pub struct SearchEmailPagination {
    pub offset: usize,
    pub limit: usize,
}

impl Default for SearchEmailPagination {
    fn default() -> Self {
        Self { offset: 0, limit: 10 }
    }
}
```

**Body** (`jmap/email.rs:265–266`):
- Replace `let page = pagination.page.max(1);` and `let page_size = pagination.page_size.max(1);` with `let offset = pagination.offset;` and `let limit = pagination.limit;`.

**Body** (`jmap/email.rs:428–448`):
- Replace the inline page-slicing block with a call to `paginate_in_range(&all_items, offset, limit, total, "emails")`.
- The result type is `Vec<(String, serde_json::Value)>` (client name + email). Group by client as before.
- Drop the duplicated `(total, page, page_size)` math.

**Strings** (`strings/jmap.rs`):
- Replace `SEARCH_EMAIL_DESCRIPTION` with the canonical paragraph + domain sentence.
- Delete `FIELD_SEARCH_EMAIL_INPUT_PAGE`, `FIELD_SEARCH_EMAIL_INPUT_PAGE_SIZE`.
- Add re-export shims for any test that imports them.

### `web_fetch` (`registry/builtin/web.rs`, `web.rs`, `strings/web.rs`)

No DTO or impl changes. The string update is the only thing:

- `WEB_FETCH_DESCRIPTION` MUST use the canonical paragraph + the `web_fetch` domain sentence.
- The description MUST add the line-count default (`limit` is in lines, not items).
- `web.rs:215` (the inline tool definition passed to the sub-agent in `tool_web_delegate`) MUST use the same wording. That keeps `web_delegate`'s sub-agent in sync.

## Helper refactor

`registry/pagination.rs` becomes:

```rust
//! Paginator helper for tool results.

/// Default `limit` for `list_files` and `list_files_by_tag`.
pub const DEFAULT_LIST_FILES_BY_TAG_LIMIT: usize = 20;

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
- The existing test `registry/tests.rs:573` (which asserts `DEFAULT_LIST_FILES_BY_TAG_PAGE_SIZE == 20`) MUST move to assert the new const value.

## Spec changes (`src/desktop/src/agent/tools/SPEC.md`)

| Existing ID | Action | New text |
|-------------|--------|----------|
| TOOL-006 | REPLACE | "Web Fetch Pagination: The `web_fetch` tool MUST accept `offset` (default 0) and `limit` (default 100) integer parameters. `offset` is the number of Markdown lines to skip. `limit` is the number of Markdown lines to return." |
| TOOL-007 | REPLACE | "Pagination Total: Every paginated tool (`list_files`, `list_files_by_tag`, `search_email`, `web_fetch`) MUST return a `total` field on its response. The value MUST be the item count across all pages." |
| — | ADD (TOOL-014) | "Pagination Hint: Every paginated tool MUST return a `hint` field on its response. `hint` MUST be set to a human-readable message when `total == 0` or `offset >= total`. `hint` MUST be absent (or `null`) otherwise." |
| — | ADD (TOOL-015) | "Pagination Defaults: `list_files` and `list_files_by_tag` MUST default `offset=0`, `limit=20`. `search_email` MUST default `offset=0`, `limit=10`. `web_fetch` MUST default `offset=0` (lines), `limit=100` (lines)." |
| — | ADD (TOOL-016) | "No Pagination Cap: Paginated tools MUST NOT cap `limit`. If the LLM requests more items than exist, the tool returns all remaining items starting at `offset` and reports the true `total` on the response. The LLM is responsible for choosing a `limit` that fits its context window; the tool's job is to honor the request and report the truth." |
| — | ADD (TOOL-017) | "Pagination Vocabulary: All paginated tools MUST use the parameter names `offset` and `limit` and the response field names `total` and `hint`. The names `page` and `page_size` MUST NOT appear in any tool schema." |
| TOOL-010 | UPDATE | Replace "fetching a page once and issuing partial reads via `limit` and `offset` to paginate through the content" with wording that uses the canonical vocabulary block. (Functionally identical, but the description must use the same words.) |

The LLM Tools Table at the top of the SPEC MUST be updated to use `offset`/`limit` for the four paginated tools.

## Test changes

`registry/tests.rs` needs the following updates.

| Test name (current) | Test name (new) | Param swap |
|---------------------|------------------|------------|
| `test_list_by_tag_default_page_size_is_20` | `test_list_by_tag_default_limit_is_20` | drop params |
| `test_list_by_tag_pagination_dispatch` | `test_list_by_tag_paging_dispatch` | `page` → `offset`, `page_size` → `limit` |
| `test_list_by_tag_pagination_is_global_across_libraries` | `test_list_by_tag_paging_is_global_across_libraries` | `page` → `offset`, `page_size` → `limit` |
| `test_list_by_tag_no_matches_reports_zero_total` | unchanged | unchanged |
| `test_list_by_tag_page_zero_is_normalised_to_page_one` | `test_list_by_tag_offset_zero_returns_first_page` | use `offset: 0` (no normalization needed) |
| `test_list_files_default_page_size_is_20` | `test_list_files_default_limit_is_20` | drop params |
| `test_list_files_pagination_dispatch` | `test_list_files_paging_dispatch` | `page` → `offset`, `page_size` → `limit` |
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

For the `SearchEmailInput` path, the email body tests at `jmap/email.rs:1130+` are integration-style and use `simplify_jmap_emails`, which does not touch pagination. No test changes are required there, but a new unit test in `email.rs` SHOULD cover the offset/limit math on the inline `paginate_in_range` call (now a real call, not inline math). Add `test_tool_search_email_paging_clamps_limit` and `test_tool_search_email_paging_offset_past_end`.

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
- A live schema probe (via `get_tools_schema` on a sample config) MUST show `offset` and `limit` in the parameter list for the four paginated tools and MUST NOT show `page` or `page_size`.

## Risks

- **Caller break.** Any LLM that has learned the old `page`/`page_size` parameter names from past conversations will now produce invalid tool calls. The error path is "invalid args", which is already surfaced to the model. The model SHOULD recover on the next turn. If a smoother migration is needed, add a brief period during which the executor accepts both `page` and `offset` (with `page` taking precedence if both are present) and emits a one-time warning. This is RECOMMENDED for the first release after the migration and SHOULD be removed in the release after that.
- **MCP tool confusion.** MCP-sourced tools (via `McpToolAdapter`) keep whatever schema the upstream server advertises. A server that uses `page`/`page_size` will look different from our built-ins. This is a known consequence of MCP's "be the server's schema" contract and is out of scope.
- **Test count.** `registry/tests.rs` has ~25 tests touching `list_files` / `list_files_by_tag`. A mass rename is a one-shot risk; reviewers SHOULD skim the diff to confirm only param names changed.
- **Stale docs.** Any external doc that references `page`/`page_size` (user-facing help, blog posts, `wiki/architecture/architecture-summary.md`) MUST be updated. The PR description MUST list a "docs to update" section.

## Out of scope

- Adding paging to the seven multi-result tools that lack it (audit F-1, F-7, F-9).
- Adding a `next_offset` or `has_next` field to the response.
- Adding a `body_truncated` field to `search_email` results (audit F-5).
- Refactoring `CalDavResponse` / `CardDavResponse` from a JSON-stringified blob into a structured DTO.
- The `web_search` paging story (audit F-2).

## Open questions

1. Should `paginate_in_range` clamp `offset` to `total` (current proposal) or pass it through and let the LLM see an empty result with no hint? The current proposal is "clamp and emit a hint." Reviewer: please confirm.
2. Should the `web_fetch` `MAX_PAGE_SIZE` cap be 1000 lines or 500? 1000 is the current proposal. Reviewer: please confirm.
3. Should we land the dual-accept `page`/`offset` shim for one release? Reviewer: please confirm.
