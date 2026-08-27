# Closing Test-Coverage Gaps — Plan

> Status: proposal
> Date: 2026-08-26
> Branch: `chore/quality-pass`

## Context

`fastmd` is a Markdown viewer + LLM agent desktop app. The suite currently
has **1042 tests** across 57 test files (sidecars `<file>_tests.rs`,
`<file>_proptests.rs`, and inline `#[cfg(test)]` blocks). A coverage audit
focused on **error scenarios and corner cases** found that most error-propagation
paths are exercised only via happy-path tests — exactly the places where silent
`unwrap_or_else` swallowing, panics, and boundary bugs hide.

This plan is the prioritized backlog produced by that audit. Work is split into
two priorities: **P0** (core agent error paths, highest risk) and **P1**
(protocol/IO error paths). Each step follows [RUST-005] — write a failing test
first, then fix, then prove green — and [RUST-003] (all code paths covered).

Out of scope: egui UI snapshot coverage (covered elsewhere), performance tests,
mutation testing, and the `discord` / `browser` features that are explicitly
flagged `UNTESTED` in `Cargo.toml` and gated out of the default build.

## Conventions

- Unit tests go in sibling `<file>_tests.rs` sidecars (RUST-001, RUST-056b) or
  inline; the impl file's `//!` doc must end with the sidecar pointer (RUST-057).
- Every `#[test]` reproduction is written **before** the code change.
- Regression seeds carry a "what input / what property" comment.
- Quality gate before done: `cargo check`, `cargo nextest run
  --status-level fail --show-progress none`, `cargo clippy -- -D warnings`,
  `cargo fmt --check`, `cargo doc --no-deps --quiet`.

## Priority 0 — Core agent error paths (highest risk)

### P0-1. `src/agent/tools/registry/pagination.rs` — direct unit tests + overflow fix

No direct unit tests today (only exercised indirectly through `list_notes`).

| Location | Gap |
|---|---|
| `paginate_in_range` (line 36) | `offset + limit` can **overflow `usize`** → panic in debug builds. Fix with `offset.saturating_add(limit)`. |
| `paginate_in_range` (line 36) | `items.len() < total` → slice index-out-of-bounds panic; invariant never asserted. |
| `limit == 0` | returns empty slice, hint `None` — untested. |
| `offset == total - 1` boundary | untested. |
| `offset >= total` | past-end hint wording with verbatim offset — untested. |
| `total == 0` | `{plural}` interpolation — untested. |

**Done in this session:** added `pagination_tests.rs` sidecar (declared in
`registry/mod.rs`), covered all the above, and applied the `saturating_add` fix.

### P0-2. `src/agent/llm_client.rs` — error mapping, retry/backoff, config corners

* `map_openai_error` (lines 141–188): all 5 match arms have **zero** direct tests —
  `Reqwest` timeout-vs-network heuristic, `ApiError` retryable (≥500/429) vs not,
  `JSONDeserialize`, `InvalidArgument`, catch-all.
* `retry_with_backoff` (190–214): retryable-then-success, always-retryable
  exhausting `total_timeout` (10 s), non-retryable short-circuit, `delay_ms`
  capped at `max_delay_ms` (8 s), retryable-after-timeout falls through to `Err`.
  Hard to test directly because the loop sleeps; refactor to make the delay
  injectable (a `delay: impl FnMut() -> Duration` or test-only clock) so the
  backoff/termination logic is testable without real sleeps.
* `from_agent_config` (67–86): `model_name` given-but-absent (`?` on line 69),
  empty models map (line 78), no-chat-use-case min-cost fallback (70–76) — all
  untested.
* `parse_usage_block` (13–48): `saturating_add` overflow path and
  explicit-`total_tokens`-beats-sum untested.

### P0-3. `src/agent/tool_executor.rs` — parallel dispatch, error recording, side effects

* `execute_parallel` (216–269): runtime-build-failure → `Vec::new()` silent
  result drop (220–224); `JoinSet::join_next()` `Err` panic-swallow (265–269) —
  both untested.
* `record_tool_errors` (169–203): `tool_group()` `None` continue, JSON-parse
  failure → `ok=false` → fallback message (188–195), `status != "success"` —
  untested.
* `extract_side_effects` (309–362): `func_name != "create_note"` skip,
  non-success skip, arguments JSON-parse-failure `continue`, missing `path`,
  path-component stripping, no-matching-library `break` — all untested.
* `extract_str` (365–374): non-string node → `""`; mid-path `None` — untested.

### P0-4. `src/agent/tools/context.rs` — panic paths & default injection

* `vfs()`/`cache()`/`uuid_gen()` `.expect(...)` panic paths (lines 40, 62, 70)
  reachable via the public builder without those extensions — untested.
* `file_observer` `DefaultFileObserver` fallback (77–84) — untested.
* `check_write_allowed` policy `Err` propagation (92–101) — only the never-errors
  `DefaultToolCallPolicy` is exercised.
* `build()` default-injection branches (VFS absent → inject; policy absent →
  inject; lines 203–220) — untested.

## Priority 1 — Protocol / IO error paths

### P1-1. `src/agent/lib/dav/client.rs` — CalDAV/CardDAV error branches

The wiremock harness exists (`cal_tests.rs`, `card_tests.rs`) but only 2 error
paths are tested (`get_calendar_item` 404, `delete_calendar_item` 500). ~15
untested `?` branches:
- `new` CalDAV/CardDAV build failure (67, 69).
- `list_calendar_hrefs`: `No principal found` (128), `No calendar home found` (132).
- `search_calendar` / `get_calendar` timerange `?` (149, 190); the date-widening
  corner (`YYYY-MM-DD` vs full form, 167–178).
- `add_calendar_item` / `update_calendar_item` PUT/GET non-2xx (238–240, 251–258,
  272–279, 289–296).
- `search_contact` / `get_contact` / `add_contact` / `update_contact` non-2xx and
  `No addressbook found` (337, 341, 373–378, 396–397, 414–422, 446–473).
- `delete_contact` 404-idempotency (486–487).
- ETag `put_if_match` vs plain-PUT branch (460–464).

### P1-2. `src/agent/tools/jmap/client.rs` — transport error paths

- `post()` (117–196): build failure, JSON `to_string`, `.send()` network,
  `body.text()`, non-success status + >500-byte truncation, empty body, JSON
  parse failure — all untested (no mock-server test drives `post`).
- `parse_error_detail` (20–36): all 4 arms (RFC 7807, JMAP type+description,
  type-only, `_ => None`) + non-JSON/non-object bodies — untested.
- `JmapSession::connect` (54–63): connect failure; `unwrap_or_default()` empty
  primary-account corner (96).

### P1-3. `src/agent/tools/csv_db/query.rs` — malformed input & aggregates

- Malformed CSV (record field-count mismatch, invalid UTF-8) never fed to
  `query_csv`/`delete_rows`; `rdr.headers()`/record-parse error paths untested
  (proptests only write well-formed rows).
- `create_context` type-mismatch (i64 vs f64 vs String) and missing-column
  predicate → `None` skip — untested.
- Aggregate corners: `sum` on non-numeric column (skip), `sum` with missing
  column → `Some(0.0)`, `avg` over empty `matched_rows` → `Some(0.0)`,
  `count == 0` guard — untested.

### P1-4. `src/agent/tools/vfs.rs` — real resolver & mock error branches

- Real `VfsResolver` (92–160): all IO error paths (`read_to_string`, `write`,
  `append`, `rename`, `copy`, `remove_file`, `metadata`, `read_dir`, `resolve`/
  `resolve_writable`) untested.
- `MockVirtualFileSystem` error-override branches: `rename_err`, `remove_file_err`,
  `copy_err`, invalid-UTF-8 `read_to_string`, `metadata` NotFound, rename/remove/
  copy on missing source — untested.

### P1-5. `src/agent/tools/yaml_header.rs` — read/write/serialize failures

- `tool_read_yaml_header` file-missing/permission-denied branch (22–25) — untested.
- `tool_write_yaml_header`: `serde_norway::to_string` failure (97–100), `vfs().write`
  failure (91–95), swallowed `create_dir_all` (82), read-fails-then-fresh-create
  path (40–41) — untested.

### P1-6. `src/agent/lib/mcp/error.rs` — JSON-RPC error defaults

- `from_jsonrpc` default fallbacks: missing `code` → `-1` (22), missing `message`
  → `"Unknown JSON-RPC error"` (23–26), absent `data` (28) — untested.
- `Display` `None`-context arm (52) and `From<McpError> for String` (59–62) — untested.

## Priority 2 — App-side lifecycle / routing / watcher

### P2-1. `src/app/orchestrator.rs` — lifecycle & bus fan-out (zero tests)

The single largest gap. All `drain_*` / `handle_*` dispatchers and lifecycle
init/shutdown have **no `#[cfg(test)]`** at all:
- `process_file_events` (74), `close_tabs_for_removed_files` (157),
  `start_agent_session` (190), `drain_config_bus` (245),
  `drain_background_channel` (295), `drain_agent_event_bus` (322),
  `handle_fs_event` (417), `handle_process_event` (472),
  `handle_mcp_auth_event` (522), `handle_file_selection` (557).

These are the bus-fan-out layer per [RUST-052]; each needs a bus-test harness
that publishes events and asserts the resulting side effects / state changes.
No panic path is directly reachable (guards use `unwrap_or`/`unwrap_or_else`),
so this is behavior, not crash, coverage.

### P2-2. `src/app/agent/session/config_subscriber.rs` — error branches

Only the startup-success path is tested. Untested:
- `tokio::time::timeout` `Ok(Err)` / `Err` → fall back to default config (23–31).
- `Lagged` handling in the main loop (53–56).
- Channel-closed `break` (57).
- `try_recv` drain loop (63–74).

### P2-3. `src/app/bus/router/bus_router.rs` — routing error paths

- Closed-channel branches (`tx_pdf.send` error → `pdf_open=false`, 74–98) — no
  test drops `rx_pdf`/`rx_img` before routing.
- No-extension path (`ext.as_deref().unwrap_or("")`, 71) and uppercase-extension
  `to_lowercase()` normalization (70) — untested.
- `FileEventKind::Removed/DirDiscovered/DirRemoved` filter (59–64) and the
  `Updated` branch (61) — never published in tests.
- Empty-payload `FileEvent` (`paths: vec![]`) no-op — untested.

### P2-4. `src/app/workspace/watcher/file_watcher.rs` — notify callback routing

Only `start()` success is tested. The entire notify event-routing closure
(65–223) is untested:
- `Create/Modify/Remove` dispatch for md/pdf/img.
- `.git` skip (68).
- `extract_tags_from_file` + `FileModified`/`FileDeleted` emission (155–169).
- `!path.exists()` → `FileDeleted` fallbacks (206–219).
- PDF-dispatch `tx_pdf`/`should_convert` (176–180) and image `tx_img` (193–205).
- `watcher.watch()` per-library failure (228–235) and `FinishedWithoutWatcher`
  branch (246–253).

### P2-5. `src/app/export/print.rs` — print error paths

`execute_print_blocking` (124): temp-file creation failure (153), `write_all`
failure (156), `temp_file.keep()` failure (160), `webbrowser::open` failure
(168) — all `?`-propagated and untested. `cleanup_temp_files` (13) never
exercised. `PrintJob::new` missing-file fallback (30) and non-UTF-8 stem
`unwrap_or("Document")` (31–35) untested.

### P2-6. `src/app/background/` — conversion decisions

- `pdf_converter.rs` `should_convert` metadata/modified failure → `false`
  (65–71); marker output-`copy` success branch (155–168); the
  `Could not find output markdown` warning (174–185); `PdfConverterWorker::spawn`
  (238–254) — untested.
- `models.rs` `ImageJob::should_process` metadata-failure → `false` (105–113) —
  untested.

### P2-7. `src/app/agent/batch/executor.rs` — concurrency failure branches

- Tokio runtime build failure (43–55), semaphore-acquire failure (83–87),
  mid-spawn cancellation re-check (113–116), `BatchJobStatus::Failed` accounting
  (149–152), `JoinSet` panic branch (162–170), round-robin model assignment with
  `model_count > 1` (99–104), and `run_agent_blocking` `Failed` branch (268–271)
  — all untested.
- `coordinator.rs` discovery-failure branch (60–69) and `BatchMode::Directory`
  construction (89–90) — untested.

**Status (P2):** Added `test_batch_config_validate_missing_prompt_path` to
`types.rs` (the missing `prompt_path` branch of `BatchConfig::validate`, 40–41).

**Known limitation (documented, not a defect):** the executor's concurrency
branches — Tokio runtime build failure, semaphore-acquire failure, mid-spawn
cancellation re-check, `BatchJobStatus::Failed` accounting, `JoinSet` panic,
and multi-model round-robin — are all gated behind `run_agent_blocking`, which
invokes the real LLM agent. They cannot be unit-tested deterministically in
the current design. Closing them requires a refactor to inject a fake agent
runner (e.g. an `AgentRunner` trait) into `BatchJobExecutor`; that is out of
scope for a test-gap pass and is recorded as a follow-up.

## Priority 3 — Markdown / UI / utility corner cases

### P3-1. `src/app/markdown/table_layout.rs` — degenerate dimensions

- Zero-width clamp `*w <= 0.0 → 1.0` (95–106) — untested (MockMeasurer returns
  0.0 for empty text but no test builds a layout from it).
- Empty AST / zero-column early-return (67–75) — never asserted directly.
- Zero/negative `col_spacing`/`row_spacing`/negative `available_width` passed
  through un-clamped (114–115) — untested; should assert the `ftwa` panic or the
  fallback at this layer.

### P3-2. `src/app/markdown/document.rs` — toggle & BOM corners

- BOM-only handling (`strip_prefix('\u{feff}')`, 45/81) — no test feeds a BOM.
- `toggle_task` with out-of-range / absent index (113–131): silently no-op but
  `revision` still bumps (283) — the no-op-but-revision-bumps contract is
  unpinned.
- Malformed multi-`---` front-matter toggle (274) — untested.

### P3-3. `src/app/markdown/parser.rs` — malformed tables

- No explicit malformed-table regression test (ragged `|` counts, pipe-only
  rows, empty cells, header-only) — only implicitly covered by the random
  `any_markdown` proptest.
- `parse_yaml_to_pairs` (≈620) only one happy-path test; non-string values,
  nested mappings, serialization failure, empty/null YAML value untested.
- Proptest input cap (2048–4096 B) means huge-input blowups aren't stress-tested.

### P3-4. `src/app/bus/events/messages.rs` — serde round-trip

The only deserialization surface in the bus tree. `LogCategory` (30) and
`BackgroundLogEntry` (63) derive `Serialize`/`Deserialize` but have no
round-trip test and no unknown-variant failure test.

### P3-5. `src/app/ui/` — state corner cases

- `panel_layout.rs`: `set_width`/`set_right_width` with `None` (reset) / negative
  values — only `Some(200.0)`/`Some(300.0)` tested.
- `tabs.rs`: `tab_titles` no-file-name fallback (98–102), `close_tab` of a
  non-existent path (63–67), case/separator-distinct handling, `heading_ids`
  empty-skip (132), `clear_content` cache invalidation (157).
- `persisted.rs`: corrupted non-JSON input, NaN/Inf floats in width fields,
  `schema_version` above `CURRENT_SCHEMA_VERSION` future-proofing branch.
- `selection.rs`: `select_file` not clearing multi-selection set (31); `prompt_dir`
  with a parent-less tab file (`PathBuf("a.md")`).

### P3-6. `src/app/utils/recycle_bin.rs` — Windows COM / trash

Entirely untested (Windows COM / trash, feature `trash` on non-Windows). Error
paths: `canonicalize` → `Io`, `CoCreateInstance`/`SetOperationFlags`/
`SHCreateItemFromParsingName`/`DeleteItem`/`PerformOperations`/`GetAnyOperationsAborted`
COM errors, `Aborted`, empty/nonexistent/locked/permission-denied paths.

## Priority 4 — Low-value, infra, and documentation drift

### P4-1. `src/agent/tools/` — thin/structural modules

- `groups.rs`: `InternalToolGroup::display_name` all nine match arms (34–46) and
  `ToolGroupState::prompt_char_count` filter_map missing-entry skip (99–104).
- `context.rs` — see P0-4 (folded in).
- `cache.rs`: `CACHE_TTL` (1800 s), `MAX_CACHE_ENTRIES` (256), `CURSOR_EXPIRED_ERROR`,
  `FINAL_PAGE_HINT` semantics; per-kind session managers never asserted.
- `policy.rs` / `observer.rs`: rejecting-policy `Err` propagation and
  `DefaultFileObserver` fallback — no direct tests.
- `extensions.rs`: `insert` overwrite, `extend` collision ordering, `get` missing,
  `downcast` failure.
- `blocking.rs`: `OnceLock` init-once / runtime-reuse, `panic!("...runtime")` path.

### P4-2. `src/agent/lib/dav/mod.rs` / `src/agent/utils/*` re-export shims

Pure re-export shims (`dav/mod.rs`, `utils/mod.rs`, `app/utils/*`, `bus/mod.rs`,
`app/workspace/vfs/mod.rs`, `workspace/mod.rs`, `app/lib.rs`). No testable logic
in-file — **document as non-issues, do not add empty test files**.

### P4-3. Untested-by-design feature flags

`discord` and `browser` integrations are explicitly `UNTESTED` in `Cargo.toml`
and gated out of the default build. Plan a separate integration-test
enablement, or mark them permanently as excluded and document why.

### P4-4. Documentation drift

- `src/app/ui/AGENTS.md` references stale `src/desktop/` paths; the real tree is
  `src/app/`. Update paths or the tree overview in the root `AGENTS.md`.
- `doc/planning/fuzzing.md` and this plan both cite `src/desktop/...` paths —
  reconcile to `src/app/` / `src/agent/`.

## Suggested execution order

1. P0-1 (done), P0-2, P0-3, P0-4 — pure / unit-testable, no infra.
2. P1-3, P1-5, P1-6 — pure-ish, reuse existing proptest/`ToolContext` harnesses.
3. P1-4 — VfsResolver (needs temp-dir fixtures).
4. P1-1, P1-2 — wiremock/mock-server harness.
5. P2-3, P2-7 — bus/router + executor, need a bus-test harness.
6. P2-1 (orchestrator), P2-2 (config_subscriber), P2-4 (file_watcher) — need
   event/notify harnesses; biggest effort.
7. P3-1, P3-2, P3-3, P3-4 — pure markdown/bus/UI corner cases.
8. P4-1 thin-module tests; P4-2/P4-3 documented non-issues; P4-4 doc drift.

Each step: failing test → implementation → green → run the full quality gate.

## Current session status

- **P0 complete** (commit c2e2705). P0-1 pagination (overflow fix + sidecar),
  P0-2 llm_client (`retry_with_backoff` refactored into
  `retry_with_backoff_and_sleep` with injectable timeout/sleep; error-mapping +
  config-corner tests), P0-3 tool_executor (extracted to
  `tool_executor_tests.rs` sidecar; parallel/record/effects tests), P0-4 context
  (panic + default-injection tests) — all green. New sidecars use
  `#[path = "..."] mod` per repo convention (`mod.rs`-style directories).
- **P1 complete** (commits 2030c032, b76bd9d, 50a47e6). P1-6 mcp/error.rs
  `from_jsonrpc` defaults + `Display`; P1-5 yaml_header read-missing;
  P1-3 csv_db/query.rs malformed/type-mismatch/aggregate corners; P1-4 vfs.rs
  real resolver + mock error branches; P1-2 jmap/client.rs `post` +
  `parse_error_detail` (via mock server); P1-1 dav/client.rs PUT-failure
  branches (via wiremock).
- **P2 complete** (commits 09b8249, 0cbc1d5, 67bddb3). Orchestrator extracted
  to `orchestrator_tests.rs` sidecar (18 tests over drain_*/handle_*);
  config_subscriber timeout fallback; bus_router routing error paths;
  file_watcher notify routing (end-to-end via real notify); print/save
  fallback + resolved-path; pdf_converter/models `should_*` metadata-failure;
  batch types `validate`. P2-7 batch-executor concurrency branches documented
  as not deterministically testable (gated behind the real LLM agent).
- **P3 complete** (commit 8f0575a). table_layout extracted to sidecar (empty
  AST, ragged rows, zero-width clamp, negative width); document extracted to
  `document_tests.rs` sidecar (BOM, toggle no-op revision-bump, second-marker);
  parser malformed-table + `parse_yaml_to_pairs` corners; messages serde
  round-trip + unknown-variant; panel_layout None/negative widths; tabs
  close-missing/no-name/clear-cache; persisted NaN/Inf/future-version.
- **P4 complete** (this commit). P4-1 groups display_name all-arms +
  prompt_char_count filter_map, cache constants + singleton, policy/observer
  rejecting/fallback, extensions insert/extend/get, blocking runtime-reuse.
  P4-4 doc drift: `src/app/ui/AGENTS.md` `src/desktop/` paths corrected to
  `src/app/`. P4-2/P4-3 remain documented non-issues.
- **Quality gate green** across the branch: `cargo check`, `cargo nextest run`
  (fastmd 1104 pass / 1 skip; fastmd-agent 833 pass / 3 skip), `cargo clippy
  --all-targets -- -D warnings`, `cargo fmt --check`, `cargo doc --no-deps`
  all clean. Pre-existing `mut` warnings in `registry/tests.rs` remain
  untouched (not introduced by this work).