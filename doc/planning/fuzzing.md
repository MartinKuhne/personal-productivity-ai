# External-Input Fuzzing — Plan

> Status: proposal
> Date: 2026-08-11
> Reviewer: MiniMax
> Branch: `chore/test-investments`

## Context

`fastmd` is a Markdown viewer + LLM agent desktop app. Every tool result
returned to the LLM is datamarked today (good), but the surface that
parses those results before they're wrapped, the LLM-supplied JSON
arguments to every tool, and the disk-read families (markdown, YAML,
CSV, vCard, iCal, …) have minimal fuzz coverage. The project already
uses `proptest` in 4 sidecars and runs them in default CI; the plan
extends that pattern, adds `cargo-fuzz` for the highest-value pure
parsers, and writes down the corner cases each target covers so a
regression can be traced to a documented input class.

Out of scope: egui UI fuzzing, performance fuzzing, mutation testing,
symbolic execution.

## Conventions established by this plan

- New proptests go in `<file>_proptests.rs` (existing sidecar rule,
  RUST-056 in `src/desktop/AGENTS.md`).
- New `cargo-fuzz` targets go in `src/desktop/fuzz/fuzz_targets/<thing>.rs`,
  with one harness per pure parser. The `fuzz/` crate is a workspace
  member alongside `src/desktop/`.
- Every regression seed lands with a "what input / what property" comment
  in the same format as the existing
  `src/desktop/proptest-regressions/agent/tools/yaml_header_proptests.txt`.
- `#[ignore]` is reserved for tests that take > 2 s or need nightly. Every
  default-CI test is runnable via `cargo nextest run`.

## Tooling additions

- Add `arbitrary` to `[dev-dependencies]` in `src/desktop/Cargo.toml`
  (with `derive` feature). Used by `cargo-fuzz` targets and by proptests
  that benefit from structured fuzzing of `serde_json::Value` and similar.
- New workspace member `src/desktop/fuzz/` with its own `Cargo.toml`
  pointing at `cargo-fuzz = "0.13"` and `fastmd = { path = ".." }`.
- New GitHub workflow `.github/workflows/fuzz.yml` running on
  `schedule: cron: '0 3 * * *'` and `workflow_dispatch`, 30 min per
  target, uploads the `corpus/` artifact on crash. Not in default CI.

## Phased plan

### Phase 0 — Branch + scaffolding

Steps:
1. Branch `feature/fuzzing-coverage` from `chore/test-investments`
   (per AGENTS.md rule 1, no changes on `main`).
2. Add `arbitrary` to dev-deps; verify `cargo check` and
   `cargo nextest run` stay green on the baseline.
3. Add `src/desktop/fuzz/` crate skeleton with `Cargo.toml` and a
   placeholder target; verify it builds with `cargo +nightly fuzz build`.
4. Add `.github/workflows/fuzz.yml` (nightly cron, 30 min/target).

Acceptance criteria:
- `cargo check --quiet` is clean.
- `cargo nextest run --status-level fail --show-progress none` passes with
  the same totals as before scaffolding.
- `cargo +nightly fuzz build` succeeds for the placeholder target.
- The nightly workflow file is syntactically valid (YAML lint passes; no
  cron / runner issues).

### Phase 1 — LLM↔tool boundary (B2, B3, B5) and datamark envelope

Steps:
1. New `src/desktop/src/agent/tools/dtos_proptests.rs`:
   one `*_roundtrips` and one `*_rejects_garbage` property per DTO in
   `tools/dtos.rs` (every `*Input` struct: web, JMAP, DAV, csv_db,
   weather, Trello, browser, batch, schedule, list, write_yaml_header,
   create_csv, add_rows, delete_rows, insert_lines, delete_lines,
   replace_text, …). `cases = 1024` per property.
2. New `src/desktop/src/agent/datamark_proptests.rs`:
   - envelope escape (content appears once, in full)
   - nested `<<<EXTERNAL_DATA>>>` is not itself a valid envelope
   - closing-marker injection does not break the wrapper
   - header-line invariant: every envelope has `provenance=...`
3. New `src/desktop/src/agent/tools/tool_call_dispatch_proptests.rs`:
   - `ToolManager::execute_tool(name, args)` over a `serde_json::Value`
     does not panic on any input
   - returns within 5 s (timeout wrapper) on any input — catches DoS
     predicates from `evalexpr` etc.

Acceptance criteria:
- All three sidecars compile under `cargo clippy -- -D warnings`.
- `cargo nextest run agent::datamark` shows 4+ new passing proptests.
- `cargo nextest run agent::tools::dtos` shows 30+ new passing proptests.
- A pre-test deliberately-bad seed (manual `panic!` in `datamark::wrap`
  for the marker-injection case) is reverted once it's confirmed the
  proptest catches it.
- No new proptest-regression seeds after a 1024-case run on the
  un-modified production code.

### Phase 2 — Network response parsers (C1, C3–C16)

Steps, one proptest sidecar per parser (`cases = 512`):
1. `src/desktop/src/integrations/mcp/sse_proptests.rs` — `parse_sse_body`
   on arbitrary bytes.
2. `src/desktop/src/integrations/mcp/oauth/discovery_proptests.rs` —
   PRM and AS-metadata parsers.
3. `src/desktop/src/integrations/mcp/oauth/pkce_proptests.rs` — verifier
   URL-safe, challenge is 43 chars, state uniqueness.
4. `src/desktop/src/integrations/mcp/oauth/flow_proptests.rs` — arbitrary
   `/authorize` redirect query strings; state-check rejects mismatches.
5. `src/desktop/src/integrations/mcp/session_proptests.rs` — arbitrary
   JSON-RPC frames; response router does not panic on id mismatch.
6. `src/desktop/src/integrations/dav/cal_proptests.rs` — arbitrary
   `text/calendar` strings (line-folding, VTIMEZONE, RDATE/EXDATE,
   leap-seconds).
7. `src/desktop/src/integrations/dav/card_proptests.rs` — arbitrary
   vCard 3.0/4.0.
8. `src/desktop/src/agent/tools/jmap/email_proptests.rs` — arbitrary
   `Email/get` and `Email/query` JSON; includes truncated bodies,
   missing `bodyValues`, deeply-nested HTML.
9. `src/desktop/src/integrations/weather_proptests.rs` — arbitrary
   Open-Meteo-shape JSON; no NaN/Inf in returned strings.
10. `src/desktop/src/integrations/trello/client_proptests.rs` — arbitrary
    Trello JSON via `wiremock`; no panic, no `null`-field crash.
11. `src/desktop/src/integrations/discord/safety_proptests.rs` —
    `SafetyFilter` over message content (bidi, ZWJ, `@everyone`/`@here`).

Acceptance criteria:
- All 11 sidecars compile under `cargo clippy -- -D warnings`.
- All 11 sidecars pass with `cases = 512` on the un-modified production
  code.
- Every sidecar has a `// Covers:` comment listing the corner-case rows
  from the catalogue in Phase 6 it exercises.
- `cargo nextest run` runtime for the new sidecars is < 5 s each.

### Phase 3 — Disk-read family (A3, A4, A5, A7, A8)

Steps:
1. `src/desktop/src/markdown/document_proptests.rs` —
   `Document::new(bytes).events()` and `front_matter()` on arbitrary
   UTF-8.
2. `src/desktop/src/markdown/parser_proptests.rs` —
   `parse_markdown_to_events` on arbitrary UTF-8 (replaces the inline
   `test_parse_markdown_fuzz_property` at `ui/render/tests.rs:828` to
   follow the sidecar convention).
3. `src/desktop/src/utils/tags_proptests.rs` — `extract_tags_from_file`
   on arbitrary content.
4. `src/desktop/src/agent/tools/csv_db/operations_proptests.rs` —
   `add_rows` with arbitrary `Vec<HashMap<String, String>>`.
5. Extend `src/desktop/src/lib/pdf/typst_translator_proptests.rs` to
   cover markdown containing `#{`, `$x = 1$`, `==` highlight, footnote
   refs without definitions, table with 0/1000 columns.

Acceptance criteria:
- 5 sidecars (4 new + 1 extended) compile under
  `cargo clippy -- -D warnings`.
- All pass with `cases = 1024` on un-modified production code.
- `test_parse_markdown_fuzz_property` is removed from
  `ui/render/tests.rs` and replaced with a `mod tests;` reference to the
  new sidecar; `cargo nextest run` shows the same number of markdown
  parser tests, all green.

### Phase 4 — File watcher + indexer (E1, E2)

Steps:
1. `src/desktop/src/app/watcher/file_event_proptests.rs` — synthesise
   a sequence of `notify::Event` records, drive
   `FileEventProcessor`. Covers rapid create/delete pairs, renames that
   swap, modify-during-read, large fan-out.
2. `src/desktop/src/app/background/indexer_proptests.rs` — drive
   `Indexer` against a `tempdir` tree generated by a proptest
   strategy; assert the indexer's output matches a hand-rolled
   reference count for small trees.

Acceptance criteria:
- Both sidecars compile under `cargo clippy -- -D warnings`.
- Both pass with `cases = 256` on un-modified production code.

### Phase 5 — `cargo-fuzz` targets

One target per pure parser, all in `src/desktop/fuzz/fuzz_targets/`:

| Target | Function | Initial corpus |
|---|---|---|
| `fuzz_markdown` | `parse_markdown_to_events` | `tests/fixtures/commonmark-0.31.2-spec.txt` |
| `fuzz_yaml_frontmatter` | `parse_front_matter` | a handful of `*.md` from `src/test/wiki/` |
| `fuzz_typst_translator` | `render_markdown_to_typst` | same markdown corpus |
| `fuzz_sse` | `parse_sse_body` | a captured MCP session transcript |
| `fuzz_mcp_jsonrpc` | response router in `mcp::session` | hand-rolled JSON-RPC frames |
| `fuzz_vpath` | `VirtualPath::parse` | `proptest-regressions/.../virtual_path_proptests.txt` |
| `fuzz_csv_db_query` | `query_csv` (fixed CSV) | the 3-row fixture |
| `fuzz_evalexpr_predicate` | `evalexpr::eval` over arbitrary string | empty (proptest seeds first) |
| `fuzz_datamark` | `datamark::wrap` | `proptest-regressions/.../yaml_header_proptests.txt` |
| `fuzz_ical` | `dav::cal` iCal parser | a few real `.ics` snippets |
| `fuzz_vcard` | `dav::card` vCard parser | a few real `.vcf` snippets |

Steps:
1. Add each target file with an `Arbitrary` adapter where the parser
   needs structured input; otherwise treat bytes as UTF-8 lossy and let
   the parser reject.
2. Add a 30-line `fuzz/Cargo.toml` declaring the targets and pinning
   `fastmd` to the parent crate.
3. Document the local-dev workflow in a 10-line `fuzz/README.md`:
   `cargo +nightly fuzz run <target> -- -max_total_time=60`.
4. Update `.github/workflows/fuzz.yml` to list all targets in a
   `matrix.target` and run them in parallel, 30 min each, cron 03:00 UTC.

Acceptance criteria:
- All 11 targets build with `cargo +nightly fuzz build`.
- All 11 targets run for 60 s on a developer laptop without finding a
  panic on the un-modified production code (smoke run).
- A pre-test deliberately-bad input (e.g. an iCal parser that panics on
  `BEGIN:VEVENT\nEND:VTODO\n`) is reverted once it's confirmed the fuzz
  target catches it.
- The nightly CI workflow runs in < 35 min and uploads a `corpus/`
  artifact on crash.

### Phase 6 — Corner-case catalogue (living document)

Steps:
1. New `doc/qa/external-inputs.md` listing every input from the audit
   table with its known corner cases. Sources of corner cases:
   - **Common to all UTF-8 text**: empty / single-char / `isize::MAX`;
     CRLF / LF / CR; BOM; lone surrogates; the boundary marker
     appearing inside content; Zalgo; bidi `U+202E`; 10 MiB of the same
     byte.
   - **Markdown**: nested blockquotes (10 000), heading with 100 000
     `#`, fenced info string 1 MB, table 10 000 columns × 1 row, ref
     link with `javascript:` URL.
   - **YAML**: billion-laughs, type-ambiguous scalars, multi-doc.
   - **VFS path**: `..` at every position, mixed `\` and `/`, NUL byte,
     `PATH_MAX` overflow, symlink loops, drive letters.
   - **LLM tool-call JSON**: wrong-type for every field, `null`
     expectations, unicode escapes decoding to control chars, truncated
     at every byte.
   - **SSE**: only `event:` / only `id:` / only `data:`, multi `data:`,
     comments, heartbeats, BOM, CR-only.
   - **iCal**: invalid dates, bad RRULE, line-folding, mismatched
     `BEGIN/END`, CRLF in `PRODID`.
   - **vCard**: 3.0 vs 4.0 syntax, empty structured fields, 10 000
     `EMAIL` entries.
   - **Datamark**: content equal to a marker, content with only the
     closing marker, content starting with a nested-looking start,
     fully-formed fake envelope inside content, 1 MB body.
   - **HTTP**: every status class, `Content-Length` mismatch,
     `Transfer-Encoding: chunked` malformed, 1 GB body, never closed.
   - **PKCE**: verifier out of range, non-URL-safe chars, low-entropy
     state.
2. Each proptest sidecar and fuzz target gets a `// Covers: <row ids>`
   comment pointing at this document.

Acceptance criteria:
- `doc/qa/external-inputs.md` is checked in and is referenced from at
  least one proptest sidecar per Phase 1–5 (12 cross-links minimum).
- Every corner case that a sidecar claims to cover has a row in the
  catalogue (no orphans).
- The catalogue includes a "last reviewed" date and the reviewer's name
  (mirrors `doc/planning/AGENTS.md` lifecycle rules).

## Cross-cutting acceptance criteria (whole plan)

- Default-CI runtime: `cargo nextest run` for the full suite increases
  by < 30 s after Phases 1–4 land. Phases 5–6 do not change default
  CI runtime.
- `cargo clippy -- -D warnings` clean across the workspace.
- `cargo fmt --check` clean.
- `cargo doc --no-deps --quiet` clean.
- Total new proptest sidecar files: 17 (4 in Phase 1, 11 in Phase 2, 5
  in Phase 3 — one of which is an extension, 2 in Phase 4 = 17
  *new* files, 1 extension).
- Total new `cargo-fuzz` targets: 11.
- The `proptest-regressions/` directory gains at least one checked-in
  seed per sidecar that finds a regression during the rollout
  (i.e. the sidecars catch something; if none catch anything on the
  initial rollout, that's a signal the property is too weak — flag it).
- A follow-up patch to `doc/planning/prompt-injection-security.md` (in a
  separate PR) cross-links to `doc/qa/external-inputs.md` and marks V5,
  V6, and V12 as "datamark fuzz target added" once Phase 1 lands.

## Risks and judgement calls

- **Adding `arbitrary`**: small, dev-only, no std impact. The
  alternative is hand-rolled `Arbitrary` impls per target, which is
  ~3x more code for the same coverage.
- **Nightly CI for `cargo-fuzz`**: needs `cargo +nightly` on the
  runner. If you want to keep CI strictly stable, run fuzzing on a
  single dedicated machine and skip the workflow — still 80% of the
  value.
- **No mutations are expected to be needed in production code during
  the rollout.** If a proptest or fuzz target catches a real bug, that
  bug fix is a separate PR (per AGENTS.md rule 1, on a feature/ or
  bugfix/ branch). This plan does not change runtime code.

## Rollout order

Phase 0 → Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6,
with each phase merged as its own PR. The corner-case catalogue
(Phase 6) is drafted in Phase 0 alongside scaffolding and grows as each
phase lands.
