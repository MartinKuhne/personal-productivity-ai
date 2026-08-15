# Keep the local agent runtime instead of adopting `rig`

Status: accepted (retroactive)
Date: 2026-08-14

## Context

`fastmd-agent` (crate at `src/desktop/src/agent/`, lib path `mod.rs`) is a
hand-written LLM tool-loop engine composed of bounded subsystems:

- `llm_client.rs` — `LLMClient`, an OpenAI-compatible `/chat/completions`
  HTTP client with `backon` exponential retry (factor 2, 1s→8s, 10s total;
  retries 5xx / 429 / timeouts) and 60s-connect / 1800s-request timeouts.
- `agent_impl.rs` — `run_agent` / `process_turn`: the synchronous agent
  loop (turn counter, message threading, tool-call dispatch, usage
  emission, reasoning/content extraction, debug entries). Driven from a
  long-lived std-thread driver spawned by `AgentSession`
  (`session.rs`), which drains an mpsc `AgentPrompt` channel.
- `tools/` — the `Tool` trait (`tools/mod.rs`), the `#[derive(ToolDescriptor)]`
  proc-macro (`fastmd-tool-macros`), `ToolDescriptor` metadata (safety,
  group, config-spec enable rules, prompt rules), `ToolProvider` /
  `RegisteredTool` registration, and `ToolRegistry`
  (`tools/registry/mod.rs`) which computes the per-run enabled tool
  schema (`get_schema` / `schema_fragment`), per-group state for the UI
  dialog, MCP tool discovery, and the tool cache.
- `tool_executor.rs` — `ToolExecutor`: partitions tool calls by
  `Safety` into a **ReadOnly-parallel** batch and a **Mutating-sequential**
  batch, records per-group errors for UI badges, and collects file side
  effects for the watcher.
- `datamark.rs` — the Spotlighting-style security envelope
  (`<<<EXTERNAL_DATA>>>` markers + `provenance=` header) that wraps
  untrusted tool output before it joins the prompt, plus the
  `SECURITY_HEADER` prepended to system prompts.
- `lib/mcp/*` — a full custom MCP client: stdio/SSE transports,
  OAuth 2.1 flow + persisted token store, `McpClients`,
  `DynamicToolSource`, and the `McpToolAdapter` that implements `Tool`.
- `events.rs` + `app/events.rs` — `AgentEventObserver` / `BusAgentEventObserver`
  publish ~10 lifecycle events onto `Bus<AgentEvent>` consumed by the UI.

A candidate replacement was evaluated: the `rig` crate family
(version 0.41.0, published 2026-07-28). `rig` is an async/tokio,
provider-neutral framework: `rig-core` supplies completion models,
typed `Message`, portable tool contracts, and 20+ providers (including an
OpenAI-compatible client with a custom base URL); `rig-agent` supplies
`Agent`/`AgentBuilder`, a hook-aware `AgentRunner`, a steppable sans-IO
`AgentRun` state machine, the typed `Tool` trait plus `ToolSet` /
`DynamicTool`, and `tool::rmcp` MCP integration.

Despite the breadth of `rig`, a gap analysis against the behaviours above
shows that the local implementation provides several features `rig` does
not cover (see below). Adopting `rig` would require retaining and
re-wiring those pieces onto a foreign execution model (sync→async, and a
foreign loop / tool-concurrency model), while deleting spec'd, tested
subsystems. On the balance of effort and risk, the local implementation
was kept.

## Decision

Keep the local agent runtime and tool machinery for now. Do not add
`rig` (or `rig-core` / `rig-agent`) as a dependency, and do not delete or
rewrite the local `agent/` loop, `LLMClient`, `Tool`/registry/executor
layers, datamark envelope, or MCP client.

Re-evaluate adoption of `rig` if the missing capabilities below are
either provided upstream or explicitly accepted as losses, or when a
multi-provider / RAG requirement emerges that justifies the migration
cost.

### Gaps `rig` does NOT cover (critical comparison)

| Capability (local impl) | What it does | `rig` 0.41.0 | Impact of adopting |
|-------------------------|--------------|--------------|--------------------|
| ReadOnly-parallel / Mutating-sequential dispatch (`tool_executor.rs` `ToolExecutor::execute_all`, `Safety` classifier) | Runs safe (read-only) tool calls in parallel, mutating calls sequentially in LLM-emission order so file/email/calendar side effects stay ordered | `AgentRunner`'s built-in loop uses a single execution path for all tool calls; no per-tool concurrency policy | Must hand-drive the sans-IO `AgentRun` and re-implement the safety batching in the driver; the built-in runner cannot express it |
| `ToolDescriptor` metadata beyond the JSON schema (`tools/descriptor.rs` + `#[derive(ToolDescriptor)]` from `fastmd-tool-macros`) | Carries `safety`, `group`, config-spec enable rules and prompt rules that drive the UI dialog, per-group badges, parallel-safety classification and the char-count budget (TOOL-015) | Typed `Tool` / `#[derive(Tool)]` carry only name, description, parameters, args | The registry/provider/descriptor layer must remain as the catalog; `rig` adds no metadata surface for it |
| Per-config / per-prompt dynamic tool enablement (`ToolRegistry::get_schema`/`schema_fragment`, `is_enabled_for`, CSV TOOL-001 gate; refreshed on `ConfigArrived` / MCP discovery through `ArcSwap<AgentToolContext>`) | The enabled tool set is recomputed per run against the live `AgentConfig` and the prompt text | `AgentBuilder` / `ToolSet` is a static ordered collection built once | Tool definitions must be rebuilt per run and again on config/MCP events; mid-run enablement flips are not expressible |
| datamark / Spotlighting security envelope (`datamark.rs`, `SECURITY_HEADER`) | Wraps every untrusted tool result in `<<<EXTERNAL_DATA>>>` + `provenance=...` envelopes to mitigate prompt injection, and prepends a security header to system prompts | No equivalent | Wrapping must be re-applied at the bridge/driver boundary for every tool output; trivially lost if tools are ported one-by-one |
| Raw request/response JSON debug transcript (`AgentDebugEntry`, Debug panel) | Logs the verbatim API request and response JSON every turn | Typed, provider-neutral `Message`/hook events; no raw wire JSON | Raw capture must live at the provider layer (or the old client be kept solely for debug) |
| Reasoning / thinking extraction (`handle_reasoning` reads `reasoning_content` from the raw response → `Thinking` events) | Surfaces model reasoning tokens to the UI | Typed completion model does not surface provider-specific reasoning generically | Needs a provider-specific seam / custom `CompletionModel` shim — [VERIFY] exact behaviour |
| Custom MCP client (`lib/mcp/*`: stdio/SSE transports, OAuth 2.1 + persisted token store, `McpClients`, `DynamicToolSource`, `McpToolAdapter`) | MCP server discovery, OAuth 2.1 authorization, and tool invocation wired into the registry and the UI auth dialog | `rig-agent` `tool::rmcp` (feature `rmcp`, on the `rmcp` crate) with its own transports | Two MCP stacks and dependency trees; OAuth/token-store flows and the UI dialog would need re-homing or a bridge |
| Per-group error state for UI badges (`tools/registry/errors.rs`, `ToolExecutor::record_tool_errors`) | Records the most recent failure kind per tool group; the UI "needs attention" badge reads it | No equivalent | Must keep recording into the registry at the bridge boundary |
| Tool-result cache (`tools/registry/cache.rs`) | Memoizes repeated tool results within a session | No equivalent | Cache lookup/populate must stay in the bridge/executor |
| `LLMClient` retry/backoff + timeout parity (`backon`: factor 2, 1s→8s, 10s total; 5xx/429/timeout; 60s/1800s timeouts) | Exact, tested retry windows and error classification | `rig` relies on reqwest-retry middleware with different knobs — [VERIFY] whether the windows are configurable to match | Either configure middleware to match, or keep `LLMClient` behind a `CompletionModel` impl (netting no HTTP-layer win) |
| OpenAI-shaped history + resume (`Vec<serde_json::Value>`, `SessionFinished` history, `ctx.history`) | History is persisted/resumed and passed to the model verbatim; debug and tests assert on the exact bodies | Typed, provider-neutral `Message` converted per provider | Converters at the boundary plus a re-format decision for persisted history; large test churn |
| Sync driver + `run_agent(ctx)` entry point (`session.rs` spawns a std-thread driver; 18 callers in `app/batch/executor.rs`; `integrations/discord/bot.rs` uses `LLMClient`) | Everything is synchronous and easy to drive from a plain thread | Async/tokio end-to-end | Session-owned runtime + `Handle` threading, plus a `block_on` wrapper to keep the 18 batch callers unchanged; deadlock audit if `block_on` is ever nested |
| Tool-call policy gate + cancellation (`tools/policy.rs` `ToolCallPolicy`, `ctx.cancel_flag`) | A policy object decides whether each tool call is allowed; an atomic cancel flag is polled between turns | Policy must be re-expressed in the driver or a hook; cancellation is driver-side — [VERIFY] hook abort semantics | Enforce policy in the driver; poll the flag between `AgentRun::next_step` steps |

### Test surface that would need rework if adopted

The following suites assert on the local loop, dispatch, envelope, and
registry behaviour and would have to be rewritten or re-homed:
`agent_impl_tests.rs`, `events_tests.rs`, `session_tests.rs`,
`tool_context_tests.rs`, `datamark_proptests.rs`, the `tools/`
suites (`tool_call_dispatch_proptests.rs`, `dtos_proptests.rs`,
`descriptor_tests.rs`, `dispatcher_tests.rs`, `provider_tests.rs`,
`specs_tests.rs`, `registry/tests.rs`, `registry/group_tests.rs`,
`mcp/adapter_tests.rs`, `csv_db/` and `jmap/` suites), and the
`llm_client` HTTP/retry tests. `app/batch` integration tests exercise
`run_agent` and would need an async host.

### Alternatives considered

| Option | Outcome |
|--------|---------|
| **Adopt `rig` end-to-end (`rig-agent` runner + typed `Tool` rewrites) (chosen NOT to pursue now)** | Removes the custom loop, `LLMClient`, and the hand-written tool layer, but loses the safety-based parallel/sequential policy, descriptor metadata, per-config enablement, datamark envelope, raw debug JSON, group badges, and the custom MCP client — all must be re-built on a foreign async model. Largest test rewrite. |
| Adopt `rig-core` providers only; keep the custom loop and tool layer | Minimal risk, but replaces only the HTTP client (which is already tested and works), netting little while importing `rig`'s async runtime and dependency graph. |
| Hand-drive `rig`'s `AgentRun` state machine behind a `CompletionModel` | Preserves parallel/sequential dispatch, cancellation, and observer events, but still requires the tool bridge, datamark re-wiring, debug-JSON capture, async runtime migration, and MCP consolidation — the costliest options remain. |
| Vendor/patch `rig` | Not viable; fast-moving breaking-change cadence would make a fork fragile. |

## Consequences

- The local agent loop, `LLMClient`, tool registry/executor, datamark
  envelope, and MCP client remain the source of truth for
  `fastmd-agent`.
- `rig` is not added to `Cargo.toml`; no async-runtime or dependency-graph
  change, and no sync→async migration risk.
- The spec'd behaviours (TOOL-001/010/014-024, AGENT-001-023, MCP
  OAuth 2.1, Spotlighting envelope) and their tests are retained.
- The gap analysis above is recorded so that a future re-evaluation is a
  delta review against this list rather than a fresh investigation.
- If `rig` later covers the critical gaps (per-tool parallel/sequential
  policy, raw request/response exposure, reasoning surfacing, MCP
  transport parity, or a config-driven tool set), or if a genuine
  multi-provider / RAG requirement appears, the decision should be
  revisited.
