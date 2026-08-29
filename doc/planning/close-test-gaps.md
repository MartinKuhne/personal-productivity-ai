# Closing Test-Coverage Gaps — Plan

> Status: proposal (audit updated, no code changed)
> Date: 2026-08-27
> Branch: `chore/quality-pass`
> Previous: 2026-08-26 error-path audit (P0–P4 complete per §History)

## Context

`fastmd` is a Markdown viewer + LLM agent desktop app (workspace: `fastmd` lib `src/app/lib.rs`, `fastmd-agent`, `fastmd-tool-macros`). The suite currently has **1104 tests** (4 binaries, 1 skipped via `cargo nextest run` default profile, 26s) and **85 sidecars** (`<file>_tests.rs` + `<file>_proptests.rs` + inline `#[cfg(test)]`). `cargo llvm-cov --workspace --summary-only` reports **83.61% line** (50771 total / 8320 missed), **81.00% func**, **82.76% region**.

The global 83.6% hides a heavy tail: **~30 files <60% line, 10 files <20%, 4 at 0%** — concentrated in MCP/OAuth, tool-registry builtins, app wiring (`init`/`render`), platform file-trash, and the proc-macro crate. The earlier 2026-08-26 audit focused on *error-propagation* paths (P0–P4, now complete — see §History). This update is a *line-coverage* audit — the complement — and proposes a remediation plan to hit **≥88–90% line** with **no file <60%** (excluding explicitly `UNTESTED` features).

Out of scope for this pass (no edits): code changes, `discord`/`browser`/`image-library` features flagged `UNTESTED` in `Cargo.toml` (gated out of default build), perf/mutation testing. Reference specs: `SPEC.md` (`MCP-001..021`, `TOOL-*`, `CONFIG-001..013`, `UI-001..066`), `AGENTS.md` quality gate, `doc/planning/AGENTS.md`.

## Conventions

- Unit tests live in sibling `<file>_tests.rs` sidecars (`[RUST-001]`, `[RUST-056]` >150 lines → sidecar, `[RUST-056b]` integration → `tests/<name>.rs`). Impl `//!` doc ends with sidecar pointer (`[RUST-057]`).
- Every change follows `[RUST-005]` (failing test first) + `[RUST-003]` (happy/corner/failure) + `[RUST-002]` (`cargo nextest`) + `[RUST-004]` narrow integration where IO.
- `facade-only lib.rs` (`[RUST-054]`), bounded subsystems (`[RUST-050]`), event fan-out `Bus<T>` (`[RUST-052]`), `RUST-020..023` modularity respected.
- Quality gate before done (from `/`): `cargo check --quiet`, `cargo nextest run --status-level fail --show-progress none` (`ci` profile `fail-fast=true, retries=0`), `cargo clippy -- -D warnings`, `cargo fmt --check`, `cargo doc --no-deps --quiet`.
- Coverage gate proposed in §Phase 0: `cargo llvm-cov --workspace --summary-only` fails build if line `<85%` (ratchet to 88% after P1) or any non-excluded file `<60%`.

## Baseline (2026-08-27)

| Signal | Value |
|---|---|
| `cargo nextest` default | `1104 run: 1104 passed, 1 skipped` (fastmd 1098 + commonmark_spec 6) |
| `cargo test` raw | same; `cargo llvm-cov` harness 1099 tests |
| `llvm-cov` line / func / region | `83.61%` / `81.00%` / `82.76%` |
| Impl files (`src/**/*` excl. `target`, `*_tests.rs`, `*_proptests.rs`, `e2e_tests/`) | `225` |
| Sidecars `*_tests.rs` | `85` |
| Impl files with *no* test companion (no sidecar + no inline `#[cfg(test)]`) | `82` |
| Quality gate today | `check` ✓, `nextest` ✓, `clippy` 6 `unused_mut` warnings in `src/agent/tools/registry/tests.rs:716,733,747,761,834,856`, `fmt`/`doc` not run |

## Coverage by Bounded Subsystem

Roll-up from `llvm-cov` (lines, `total miss cov`):

```
agent/lib            9186  2498  72.8%  ← worst
agent/tools          8440  1888  77.6%
fastmd-tool-macros    237    69  70.9%
app/config            692   175  74.7%
app/export           1131   247  78.2%
agent/context.rs      237    64  73.0%
app/utils             605    93  84.6%  (recycle_bin 0% drags)
app/agent            2297   330  85.6%
app/ui              16814  2053  87.8%  (init 47%, render 37%, tree/render 57%)
app/orchestrator      543    69  87.3%
app/background        894    48  94.6%
app/bus              1703   120  93.0%
app/markdown         2761   194  93.0%
app/workspace        1277    68  94.7%
agent: agent_impl 94.6%, llm_client 96.9%, session 96.4%, vfs 94.6%, tool_executor 92%
```

Hot paths (`agent_impl`, `llm_client`, `bus`, `markdown`, `workspace`) are healthy. Gaps cluster in **MCP/OAuth**, **tool-registry builtins**, **app lifecycle**, **platform trash**, **proc-macro**.

## Gap Inventory

### Gaps by test presence — 82 files with zero companion

Largest (LOC desc, real impl only — `*_tests.rs` helpers `agent/lib/mcp/tests.rs:1966`, `app/ui/app/tests.rs:1386`, `registry/tests.rs:993`, `render/tests.rs:915` excluded):

```
735  app/agent/session/browser_session.rs        — browser feature (#[cfg(feature=browser)], UNTESTED)
509  app/integrations/discord/gateway.rs         — discord feature UNTESTED
501  agent/lib/dav/client.rs                     — CalDAV (501 LOC, 72.33% via indirect)
468  agent/tools/jmap/mock_server.rs             — mock
465  agent/tools/registry/builtin/fs.rs          — 63.6% but no sidecar
432  agent/tools/registry/builtin/strings.rs     — 132 pub items, 18% uncovered regions
409  agent/lib/mcp/clients.rs
375  app/ui/tree/context.rs                      — 51 pubs, 88% but no isolated Id tests
330  fastmd-tool-macros/src/lib.rs               — 27% func
304  agent/tools/registry/builtin/trello.rs      — 11.3%
... + 71 facades/mods (app/lib.rs, bus/mod.rs, vfs/mod.rs, etc.)  [RUST-001] violation
```

### Gaps by line % — `llvm-cov` files <85% (missed lines)

```
  0.00   87  app/utils/recycle_bin.rs              — Windows COM IFileOperation + trash fallback
  0.00  131  app/main.rs                           — eframe entry (exclude from gate)
  0.00   35  app/bin/deploy.rs                     — deploy bin (exclude)
  0.00  140  agent/lib/mcp/oauth/test_support.rs   — test helper (exclude)
  5.38  123  app/agent/session/browser_session.rs  — browser ext
 11.30  204  agent/tools/registry/builtin/trello.rs
 13.41  155  agent/tools/registry/builtin/caldav.rs
 16.30  113  agent/tools/registry/builtin/csv.rs
 17.31  129  agent/tools/registry/builtin/carddav.rs
 17.65   84  agent/tools/registry/builtin/jmap.rs
 18.18   72  agent/tools/registry/builtin/yaml.rs
 18.18   81  agent/tools/registry/builtin/web.rs
 31.82   15  agent/tools/provider.rs
 35.00   26  agent/tools/registry/builtin/weather.rs
 37.06   90  app/ui/app/render.rs
 37.50    5  app/agent/session/bus_observer.rs
 45.51   91  app/export/pdf/save.rs
 46.98  202  app/ui/app/init.rs                    — 381 total
 47.06    9  app/ui/os_shell.rs
 54.40   83  app/export/print.rs
 54.75  319  agent/lib/mcp/oauth/flow.rs           — 705 total
 55.02  103  agent/lib/mcp/oauth/client.rs
 56.89  247  app/ui/tree/render.rs
 57.84  788  agent/lib/mcp/session.rs              — 1869 total
 58.24   38  app/agent/session/config_subscriber.rs
 59.39  147  agent/lib/mcp/mod.rs
 60.87  144  app/ui/tools_dialog.rs
 60.98   48  app/ui/app/mod.rs
 63.62  167  agent/tools/registry/builtin/fs.rs
 65.71   24  agent/tools/mcp/adapter.rs
 67.87  160  app/ui/modals.rs
 68.20   90  app/ui/batch_dialog.rs
 68.24  148  app/ui/agent_debug_window.rs
 70.73  331  agent/lib/dav/card.rs                 — 1131 total
 70.89   69  fastmd-tool-macros/src/lib.rs         — 27% func
 71.38  154  agent/tools/registry/mod.rs
 71.79   22  agent/lib/trello/client.rs
 72.33  166  agent/lib/dav/client.rs
 72.50   55  app/ui/render/mod.rs
 73.00   64  agent/context.rs
 73.52   94  app/agent/batch/prompts.rs
 74.14  113  app/ui/panels/center.rs
 74.71  175  app/config/config.rs                  — 692 total
 74.73   69  app/bus/events/agent.rs
 74.89  171  agent/lib/mcp/oauth/redirect.rs
 78.21   34  app/workspace/watcher/file_processor.rs
 78.77  210  agent/tools/jmap/email.rs
 80.61   19  app/export/pdf/mod.rs
```

Top 15 `<60%` account for **~1999 missed lines (24% of all misses in 6% of files)** — fixing them moves total `83.6% → ~87.5%`.

## Thematic Gaps (mapped to SPEC / NFR)

### T0 — Security / integrity (P0)

- `agent/lib/mcp/session.rs:57.84%` (788 missed), `oauth/flow:54.75%` (319), `oauth/client:55%`, `oauth/redirect:74.9%`, `oauth/types:75.5%`, `mcp/mod:59%` — MCP spec §2.2/2.5/3.4/5.1 state machine: init handshake, version negotiation `SUPPORTED_PROTOCOL_VERSIONS` (`src/agent/lib/mcp/session.rs:169`), `CREATE_NO_WINDOW` (`src/agent/lib/mcp/session.rs:86`), timeout cap `MAX_REQUEST_TIMEOUT 600s` (`src/agent/lib/mcp/session.rs:161`), `MCP-Session-Id` DELETE 405 ack (`src/agent/lib/mcp/session.rs:578`), progress-token drop, `probe_legacy_transport`, `mark_stdio_dead`. Only ~55% exercised. `SPEC.md` `MCP-001..021` at risk, `[NFR-001][NFR-005][NFR-008]` (ERROR log + stack trace + spans for external deps) unproven.
- `agent/lib/dav/{client,card}:72/70%` (dav/card 331 missed), `trello/client:71.8%` — external deps without span/error-contract tests (`[NFR-008]`).
- `app/utils/recycle_bin.rs:0%` (`src/app/utils/recycle_bin.rs:11,137`) — destructive file mutation (Windows COM `CoInitializeEx`/`CoUninitialize`, `\\?\` verbatim prefix strip `src/app/utils/recycle_bin.rs:61`, `FOF_ALLOWUNDO`). 0 tests.

### T1 — Product-critical wiring (P0/P1)

- `app/ui/app/init.rs:46.98%` (202 missed, 381 total) + `app/ui/app/{mod,render}:60/37%` — `FastMdApp::new` (`src/app/ui/app/init.rs:70`) + `empty_state_via_bus` (`src/app/ui/app/init.rs:293`). Bus subscription order, `ToolRegistry → ArcSwap`, `spawn_config_subscription`, `BrowserSession` lazy init, persisted-state migration `schema_version` (`src/app/ui/app/init.rs:208`). `SPEC.md` `UI-001..066` drift risk.
- `app/config/config.rs:74.7%` (175 missed, 72 funcs 24 missed) — YAML schema, token limits, cost routing `CONFIG-001..013`, `[NFR-009]` PII redaction.
- `app/orchestrator.rs:87.3%` (69 missed) — watcher/file-event fan-out `[RUST-052]` Bus.
- `agent/context.rs:73%`, `agent/tools/provider.rs:31.82%` — provider delegation thin.

### T2 — Tool surface (P1)

- All `registry/builtin/*` `11–63%` — `ToolDescriptor` impls enumerated in `registry/mod.rs:71%` (`src/agent/tools/registry/builtin/*`). **No sidecars** (`[RUST-001]`). Each must have happy/corner/failure (`[RUST-003]`) + safe/unsafe parallelism `AGENT-033`. Worst: `trello/caldav/csv/carddav/jmap/yaml/web <20%`; `fs 63%` only above 50%; `vector_search`, `web`, `jmap/email` low. Related to `SPEC.md` `TOOL-001..047`.

### T3 — Desktop UI polish (P2)

- `app/ui/tree/render.rs:56.89%` (247 missed), `tools_dialog:60.8%`, `modals:67.8%`, `batch_dialog:68%`, `center:74%`, `editor_egui:76%`, `agent_debug_window:68%`, `render/mod:72%`. Egui `Id` stability (`src/app/ui/AGENTS.md` §5) and conditional-render shape (`src/app/ui/AGENTS.md` §6) covered only via some `egui_kittest` snapshots (`bottom 94%, left 92%, right 97%, top 85%` OK). `tree/context.rs:88%` lacks dedicated `Id` salt regressions.

### T4 — Background / export (healthy but narrow)

- `file_processor 78%`, `pdf/save 45%`, `print 54%`, `typst_translator 91%` — `save.rs`/`print.rs` Typst + `rfd` dialog error paths lack failure-mode tests `[RUST-003]`. `background/models 100%` (trivial).

### T5 — Intentionally low / feature-gated

- `browser_session:5.38%`, `browser.rs:21 LOC` — feature `browser` `UNTESTED` (`Cargo.toml:36`).
- `discord/gateway:509 LOC` `UNTESTED` (`Cargo.toml:33`) — `safety_proptests` covers `safety.rs` but not gateway.
- `image-library` empty, `vector-search` conditional, `app/main.rs`/`bin/*` entry points — expected 0%, exclude from gate.

### T6 — Test-infra quality

- `fastmd-tool-macros/lib.rs:70.89% line, 27.27% func` — proc-macro `#[derive(ToolDescriptor)]` (`src/fastmd-tool-macros/src/lib.rs:330`). Func coverage crates; needs `trybuild` UI tests.
- `[RUST-056]` oversized `#[cfg(test)]` blocks that should be sidecars: `registry/tests.rs:993`, `render/tests.rs:915`, `app/tests.rs:1386` — per `[RUST-056b]` prefer `tests/<name>.rs`.
- `clock:50%`, `bus_observer:37%`, `config_subscriber:58%` — small doctrinal gaps.

## Remediation Plan — Phases (no code changed in this audit)

Each phase lists: objective, files/tasks, strategy, SPEC trace, acceptance.

### Phase 0 — Guardrails (1–2 days) — *do first*

- **0.1** Add `cargo llvm-cov` to CI (`.github/workflows`) with `--summary-only` artifact + fail if line `<85%` (ratchet to 88% after P1) or any non-excluded file `<60%` or `fastmd-tool-macros` func `<50%`. Document threshold in `DEVELOPMENT.md`. Evidence: currently 83.61% so gate ratchets up.
- **0.2** Coverage exclusions: `app/main.rs`, `app/bin/*`, `agent/lib/mcp/oauth/test_support.rs`, and feature-gated `browser`/`discord` when feature off — via `llvm-cov` ignore or `#[coverage(off)]`. Keeps entry points from punishing gate.
- **0.3** Fix 6 `unused_mut` warnings in `src/agent/tools/registry/tests.rs:716,733,747,761,834,856` so `cargo clippy -- -D warnings` is green (prereq per Quality Gate).
- **0.4** Confirm `cargo fmt --check` + `cargo doc --no-deps --quiet` in CI matrix.

*SPEC:* process gate for all `REQ-xxx`. *Accept:* CI fails on dip, `nextest --profile ci` green. *Effort:* small.

### Phase 1 — Security & data integrity (P0) — 1 week

- **1.1 `recycle_bin` 0% → 95%** (`src/app/utils/recycle_bin.rs:190`) — `RECYCLE-001`, `[RUST-005]`. Sidecar `recycle_bin_tests.rs`. Cases: canonicalize failure (non-existent → `Io`), success via temp-file, `Aborted` branch, COM errors mocked via `#[cfg(test)] trait TrashImpl` injection (avoid real COM during CI). Linux `trash::delete` mocked. Covers `[NFR-001]`.
- **1.2 `agent/lib/mcp/session` 57.8% → 85% + `oauth/flow+client+redirect+types` 54–75% → 85%** (`src/agent/lib/mcp/session.rs:1869`, `oauth/flow.rs:705`, `client.rs:229`, `redirect.rs:681`) — `MCP-001..021`, `[NFR-001,005,008]`. Sidecars `session_tests.rs` expansion + `oauth/flow_tests.rs`, `client_tests.rs`. Narrow integration with `wiremock` (already `dev-dependency`) for HTTP: 401 `WWW-Authenticate` → `run_flow` → retry with bearer; 404 session reset; 405 DELETE ack; `probe_legacy_transport` GET `event: endpoint`; stdio `build_stdio_command` `CREATE_NO_WINDOW` flag; timeout cap `MAX_REQUEST_TIMEOUT`; progress-token drop on error; unsupported version disconnect `SUPPORTED_PROTOCOL_VERSIONS`. No real MCP server needed.
- **1.3 `agent/lib/dav/{client,card,cal}` 72/70% → 85% + `trello/client 71.8% →85%`** (`src/agent/lib/dav/client.rs:501`, `card.rs:1131`) — `TOOL-*` DAV/Trello. Sidecar for `dav/client.rs` (currently none); cover parsers, auth, error mapping, `[NFR-008]` spans.

*Impact:* closes ~1293 missed lines (recycle 87 + mcp 788+319+103+171 + dav 331+166). Moves total `83.6% → ~86%` alone.

### Phase 2 — Orchestration & config (P0/P1) — 1 week

- **2.1 `app/ui/app/init 47% →80%` + `render 37% →75%` + `app/mod 60% →80%`** (`src/app/ui/app/init.rs:418`, `render.rs:143`, `mod.rs:123`) — `UI-001..066`, `[RUST-052]`. Expand `init_tests.rs` (20 tests) + offscreen harness `src/app/ui/test_helpers/offscreen.rs:76.92%`. Cases: `configure_dark_theme` visuals, `new` bus-order, `ToolRegistry` swap assertion, `empty_state_via_bus` migration `schema_version < CURRENT` clears `font_size_scale` (`src/app/ui/app/init.rs:208`), `agent` vs `config_reader` drain, `render` `request_repaint` tick.
- **2.2 `app/config/config.rs 74.7% →85%` + `bus/events/agent.rs 74.7%`** (`src/app/config/config.rs:692`, `src/app/bus/events/agent.rs:273`) — `CONFIG-001..013`, `AGENT-005`. Parameterized cases: missing `config.yaml`, bad YAML, token-limit validation, model routing (`AppConfig::default()`), secret redaction `[NFR-009]`.
- **2.3 `app/workspace/watcher/file_processor 78% →90%` + `app/export/{pdf/save 45%, print 54%}`** (`src/app/workspace/watcher/file_processor.rs:156`, `src/app/export/pdf/save.rs:167`, `src/app/export/print.rs:182`) — `REQ-301..504`, `VFS-001..130`. Tests via temp-dir: debounce, `..` traversal rejection, Typst engine failure, `rfd` cancel, `pdf_backing_tracker` races. Existing `save_tests.rs:45%`, `print.rs` expand.

### Phase 3 — Tool surface (P1) — 1.5 weeks

*Goal:* every `registry/builtin/*` gets `<builtin>_tests.rs` sidecar, bringing builtin avg `~18% → >85%` and `agent/tools 77.6% → 85%+`.

Order by risk:

1. `fs 63% →90%` (`src/agent/tools/registry/builtin/fs.rs:465`) — local FS, highest blast radius: permission errors, `..` rejection, parallel policy safe/unsafe (`TOOL-001..`), cancel/stop.
2. `web 18% →85%` (`src/agent/tools/registry/builtin/web.rs:126`, `src/agent/tools/web.rs:81%`) — timeout, redirect, non-2xx diagnostic.
3. `yaml 18% + csv 16% →85%` (`src/agent/tools/registry/builtin/{yaml,csv}.rs`) — malformed input, ragged rows, schema inference.
4. `jmap 17% + jmap/email 78% →90%` (`src/agent/tools/registry/builtin/jmap.rs:124`, `src/agent/tools/jmap/email.rs:78%`, `mock_server.rs:468`) — mock_server contracts, 401/403, pagination.
5. `caldav/carddav 13/17% →85%` (`src/agent/tools/registry/builtin/{caldav,carddav}.rs:202/189`) — DAV XML failure modes.
6. `trello 11% →85%` (`src/agent/tools/registry/builtin/trello.rs:304`) — board/card CRUD mocks.
7. `weather 35% →85%` (`src/agent/tools/registry/builtin/weather.rs:55`) — geocoding/open-meteo mocks.

For each: `cargo nextest` unit + `wiremock`/`mock_server` narrow integration (`[RUST-004]`) + `proptest` where input-driven. Also `provider.rs:31% →85%` (`src/agent/tools/provider.rs:22`), `registry/mod.rs:71% →85%` (`src/agent/tools/registry/mod.rs:538`), `registry/groups` (`src/agent/tools/registry/groups.rs:105`).

*Also:* `fastmd-tool-macros 27% func →75%` (`src/fastmd-tool-macros/src/lib.rs:330`) — `trybuild` UI tests for derive macro happy/error diagnostics.

*Impact:* top 15 `<60%` hold ~1999 missed lines; closing them → `83.6% → ~87.5%`. Full P1 `~88–90%`.

### Phase 4 — UI panels & editors (P2) — 1 week

- **4.1 `ui/tree/render 56.8% →85%` + `tree/context 88% →95%`** (`src/app/ui/tree/render.rs:573`, `context.rs:168`) — `Id` stability (`src/app/ui/AGENTS.md` §5 salted keys), `flatten`/`handlers` 95/98% but render lags. Align with `doc/distill/egui-kittest.md` snapshot pattern. Then `tools_dialog 60% →85%` (`src/app/ui/tools_dialog.rs:368`), `modals 67% →85%` (`src/app/ui/modals.rs:498`), `batch_dialog 68%` (`src/app/ui/batch_dialog.rs:283`), `center 74%` (`src/app/ui/panels/center.rs:437`), `editor_egui 76%` (`src/app/ui/editor_egui.rs:178`), `agent_debug_window 68%` (`src/app/ui/agent_debug_window.rs:466`), `render/mod 72%` (`src/app/ui/render/mod.rs:200`).
- **4.2 `ui/test_helpers/offscreen 76% →90%`** (`src/app/ui/test_helpers/offscreen.rs:247`) — enabler for 4.1.
- **4.3 `render/table/*`, `yaml_table`, `selection` etc.** — `cell 91%` OK but `configured 99%` vs `render 72%` shows harness gap.

### Phase 5 — Feature-gated & residual (P3 backlog)

- **5.1** Feature flag decision: `discord UNTESTED`, `browser UNTESTED`, `image-library UNFINISHED` (`Cargo.toml:31,36,39`). Either exclude from gate + document `SPEC.md` Optional Feature, or schedule `browser_session 5% →70%` with Playwright mock if the feature ships. Current audit excludes them; drift flagged per `[RUST-041]`.
- **5.2** Extract oversized `tests.rs` into sidecars/tests/: `ui/app/tests:1386`, `registry/tests:993`, `render/tests:915` per `[RUST-056]/[RUST-056b]` (facade-only `lib.rs` `[RUST-054]`).
- **5.3** Add missing proptests for `csv_db/schema`, `registry/builtin/*`, `tool_call_dispatch` (`64%` `tool_call_dispatch_proptests`).
- **5.4** `clock:50%` (`src/app/utils/clock.rs:6`), `bus_observer:37%` (`src/app/agent/session/bus_observer.rs:21`), `config_subscriber:58%` (`src/app/agent/session/config_subscriber.rs:91`) — 3–5 tests each to 95%.

## Suggested Execution Order

1. Phase 0 guardrails (stop bleed).
2. Phase 1.1 recycle_bin + 1.2 MCP OAuth/session (security).
3. Phase 2.1 init/render (wiring) — unblocks UI.
4. Phase 3. fs/web + provider/registry, then yaml/csv, jmap, caldav/carddav, trello/weather, macro.
5. Phase 2.2 config + 2.3 file_processor/save/print.
6. Phase 4 tree/render + panels/editors (needs offscreen first).
7. Phase 5 residual + flag decision + doc drift.

Each step: failing test → implementation → green → full quality gate. Estimate: **P0 (0+1) ~1.5w**, **P1 (2+3) ~2.5w**, **P2 (4) ~1w**, total ~5 weeks to 88–90%.

## Estimated Impact

| Milestone | Missed closed | Line % | Function % |
|---|---|---|---|
| Baseline | — | 83.61% | 81.00% |
| After 0 fix clippy + exclusions | 0 | ~83.8% | ~82% |
| After top 15 `<60%` (P1) | ~1200 | ~86% | ~83% |
| After full P0+P1 (Phases 1–3) | ~2500 | ~88.5% | ~87% |
| After P2 UI polish | ~3500 | ~90.5% | ~89% |

Per-file gate `<60%` eliminated after Phase 3 (only `main/bin/browser/discord` excluded).

## Verification (for implementers)

```powershell
cargo check --quiet
cargo nextest run --profile ci --status-level fail --show-progress none  # 1104+ new, no flake (default retries 2)
cargo llvm-cov --workspace --summary-only  # gate: line >=85% now, >=88% after P1, per-file >=60%
cargo clippy -- -D warnings
cargo fmt --check
cargo doc --no-deps --quiet
```

Each new sidecar must: live as `<file>_tests.rs` sibling (`[RUST-001]`), impl `//!` doc ends with `//! Unit tests live in the sibling '<filename>.rs' sidecar.` (`[RUST-057]`), keep modules `<4096 lines` (`[RUST-053]`), split large functions (`[RUST-023]`), use `pure` helpers (`[RUST-020]`), and cite `REQ-xxx`/`TOOL-xxx`/`MCP-xxx` in `//!`/`///` (`[RUST-040]`). Happy + corner + failure paths required (`[RUST-003]`).

## History (2026-08-26 error-path plan — complete)

Summary: the 2026-08-26 audit split gaps into P0 core-agent error paths and P1 protocol/IO paths, then P2 lifecycle and P3 markdown/UI corners. As of `349a0d6` all were completed on `chore/quality-pass`:

- **P0 complete** (c2e2705): `pagination` overflow fix + sidecar; `llm_client` `retry_with_backoff_and_sleep` injectable; `tool_executor` sidecar; `context` panic/default-injection.
- **P1 complete** (2030c032, b76bd9d, 50a47e6): `mcp/error` defaults; `yaml_header` missing; `csv_db/query` malformed/aggregates; `vfs` resolver + mock; `jmap/client` `post` + `parse_error_detail`; `dav/client` PUT failures (wiremock).
- **P2 complete** (09b8249, 0cbc1d5, 67bddb3): orchestrator sidecar (18 tests); config_subscriber timeout; bus_router routing; file_watcher notify routing; print/save fallback; pdf_converter/models `should_*`; batch `validate` (+ executor concurrency documented as not deterministically testable without injected `AgentRunner`).
- **P3 complete** (8f0575a): table_layout sidecar; document BOM/toggle; parser malformed + `parse_yaml_to_pairs`; messages serde; panel_layout/tabs/persisted.
- **P4 complete** (349a0d6): `groups` display_name, `cache` constants, `policy`/`observer`/`extensions`/`blocking`, doc drift `src/desktop/` → `src/app/` in `src/app/ui/AGENTS.md`.
- **Quality gate then:** `cargo check`, `cargo nextest` (fastmd 1104 pass/1 skip; fastmd-agent 833 pass/3 skip), `clippy --all-targets -D warnings`, `fmt`, `doc` clean.

This plan was archived from `doc/planning/close-test-gaps.md:378` on 2026-08-27 for reference; the new coverage plan above supersedes it for the coverage-percentage goal.

## Appendix — Raw Evidence

- `cargo llvm-cov --workspace --summary-only` output saved locally to `C:\Users\mkuhn\AppData\Local\Temp\opencode\cov.txt` (not committed): `TOTAL 50771 8320 83.61%`, worst files listed above.
- Sidecar scan: `Get-ChildItem -Recurse -Filter "*.rs" src` → 336 files; 85 `*_tests.rs`; 82 impl files with no companion (see GAP inventory).
- Previous audit file cross-links updated: `ARCHITECTURE_C4.md` authoritative for module boundaries (`[RUST-040]`), `SPEC.md` not edited per `[RUST-043]`.
