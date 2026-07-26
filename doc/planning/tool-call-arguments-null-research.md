# Research: LLM "claims" tool arguments but `function.arguments` arrives as `null`/empty

Research compiled 2026-07-25. Goal: catalog the failure modes where an LLM's
assistant text says "I'll call tool X with {a:1, b:2}" but the structured
`tool_calls[].function.arguments` field in the same message is `null`, `""`, or
malformed, and the orchestrator passes the broken payload straight into the
tool. Related opencode/AI-SDK bug reports are listed at the bottom.

This file is **research / reference only** — no production code changes yet.
The relevant call sites in this repo are:

- `src/desktop/src/agent/agent_impl.rs` — `process_turn` reads `message["tool_calls"]`
  and passes the array straight to the executor.
- `src/desktop/src/agent/tool_executor.rs` — `extract_str(tc, &["function", "arguments"])`
  in `execute_parallel` / `execute_sequential` pulls the arguments string. When
  the field is `null`, `current.as_str()` returns `None` and `unwrap_or("")`
  yields `""`. The empty string is then passed to `execute_tool`.
- `src/desktop/src/tools/registry.rs:126` — `execute_tool` parses `args_str`
  as JSON; `serde_json::from_str("")` errors with "EOF while parsing".

So in this codebase, an LLM that emits `arguments: null` is silently turned
into `args_str = ""` and surfaced to the tool as an unparseable payload.

---

## 1. The four concrete failure modes

| # | Wire form of `function.arguments` | What the LLM text says | What the tool sees |
|---|-----------------------------------|------------------------|--------------------|
| A | `""` (empty string)               | "I'll call `unlock()` with no params" | `args_str = ""` → JSON parse error |
| B | `null`                            | "Calling with `{id:42}`" | `args_str = ""` (extract_str coalesces) → JSON parse error |
| C | `"{\"id\":"` (truncated mid-JSON) | Half-written tool call (max_tokens hit) | `args_str = "{\"id\":"` → JSON parse error |
| D | `<function=...>` XML in `content` instead of `tool_calls` | Normal prose, no structured call | No tool invocation at all |

Modes A and B are the user's "claims but null" pattern. Mode C is the
streaming/max_tokens cousin. Mode D is the opencode/Qwen/Ollama XML family.

---

## 2. Why it happens (root causes, ranked)

### 2.1 Provider / SDK serializes empty arg lists as `""` (mode A)

The OpenAI Chat Completions spec says `arguments` is a **stringified JSON
object**, and for a zero-arg tool the canonical value is `"{}"`. In practice:

- **Anthropic on OpenAI-compatible providers** sends `""` instead of `"{}"`
  for parameterless tools. Confirmed in
  [vercel/ai #6687](https://github.com/vercel/ai/issues/6687) and
  [BerriAI/litellm #5063](https://github.com/BerriAI/litellm/issues/5063).
- **vLLM with certain parsers** (e.g. GLM-5 via NIM) emits
  truncated/malformed JSON, see
  [anomalyco/opencode #13900](https://github.com/anomalyco/opencode/issues/13900).

### 2.2 LLM decides after-the-fact that no args are needed (mode A/B)

Some reasoning models emit a `<tool_call>` block with the args they
*intend* to use, then re-emit a second call where the args field is empty
because the plan changed but the call wasn't cleaned up. Cohere Command-A
exhibited this — see
[HuggingFace discussion](https://huggingface.co/CohereLabs/command-a-plus-05-2026-w4a4/discussions/3)
where `tool=, error=Model tried to call unavailable tool ''. … invalid [tool=]`
was the symptom of an empty tool-call record.

### 2.3 Max-tokens truncation (mode C)

When the response is cut off mid-`arguments` string, the SDK still closes the
chunk and forwards `"{\"path\":\"/us"` to the orchestrator. Common with
small context windows (qwen3-30B-A3B, GLM-5, Mistral) and reported on
[llama.cpp #14697](https://github.com/ggml-org/llama.cpp/issues/14697) as
"intermittent JSON in content".

### 2.4 Hallucinated XML / non-JSON tool format (mode D)

Qwen2.5/3-Coder, GLM-4, granite3.3, Hermes, and similar local models emit
`<tool_call>{...}</tool_call>` or `<function=...><parameter=...>value`
inside `content` rather than as a structured `tool_calls` entry. See
[anomalyco/opencode #4428](https://github.com/anomalyco/opencode/issues/4428),
[#1406](https://github.com/sst/opencode/issues/1406),
[#1809](https://github.com/sst/opencode/issues/1809).

### 2.5 Stream parser drops the call when JSON is unparseable (Vercel AI SDK)

This is the closest cousin of the user's bug and the one most often cited as
"opencode bug" — opencode's runtime uses the Vercel AI SDK.

In `packages/openai-compatible/src/openai-compatible-chat-language-model.ts`
the stream parser does:

```ts
if (isParsableJson(toolCall.function.arguments)) {
  controller.enqueue({ type: 'tool-call', /* ... */ });
}
```

`isParsableJson("")` is `false`, so an Anthropic-style empty-args call is
**silently dropped**. The user sees a `finishReason: "tool-calls"` but no
execution happens. Fixed by PR that landed in `merged-main` for
[vercel/ai #6687](https://github.com/vercel/ai/issues/6687). The fix
accepts both `""` and `"{}"` as "no args".

---

## 3. Hallucinated vs. lost arguments — both look like "null to tool"

Two distinct bugs share the same downstream symptom (the tool gets nothing
usable):

1. **Lost-in-stream** (mode A): the args are *valid* on the model's side but
   serialized as `""` and dropped by the stream parser. The LLM "claimed"
   in text and structured the call correctly in the model's output tokens,
   but the JSON envelope in transit was empty.
2. **Hallucinated-by-LLM** (mode B/D): the LLM *itself* emits `null` /
   `""` for arguments, often as a side-effect of the same reasoning trace
   where it tells the user in prose what it intends to do. qwen2.5-coder
   is a repeat offender; see
   [r/LocalLLaMA: qwen2.5-coder emits tool calls opencode can't process](https://www.reddit.com/r/LocalLLM/comments/1uiixjz/).

Distinguishing them at runtime: log the raw `message.tool_calls` JSON the
moment it lands. If the string is `""` but `finish_reason` is
`tool_calls` and the content text describes a full call → mode A (SDK bug).
If the string is `null` and the content text describes a full call → mode B
(model bug). For mode D the tool_calls array is empty and the JSON is
inside `content`.

---

## 4. Opencode / SST OpenCode bug reports (relevant)

All from `github.com/sst/opencode` and `github.com/anomalyco/opencode`:

- **[#1406 — tool_call was output as text, not executed](https://github.com/sst/opencode/issues/1406)**
  Qwen emits `<tool_call>{...}</tool_call>` in content; opencode shows it as
  text. Mode D.
- **[#1034 — Local Ollama tool calling either not calling or failing outright](https://github.com/anomalyco/opencode/issues/1034)**
  Multiple Ollama-served models hallucinate tool calls, output raw JSON, or
  describe what they would do without invoking anything.
- **[#234 — Tool Calling Issues with Open Source Models in OpenCode](https://github.com/anomalyco/opencode/issues/234)**
  Aggregated: case-sensitivity (`Write` vs `write`), complete tool-call
  failure, model compatibility matrix. Workaround: provider-level
  `toolParser` to recover text-only tool intents into structured calls.
- **[#1809 — qwen3-coder-30B-A3B is not able to call any tool](https://github.com/sst/opencode/issues/1809)**
  Args get double-stringified (`"todos":"[…]"` instead of `"todos": […]`).
  Mode C-ish / model-bug hybrid.
- **[#800 — DigitalOcean Inference fails to use tool-calls (openai-compatible provider)](https://github.com/sst/opencode/issues/800)**
  Streaming chunk arrives as `{"function":{"arguments":"{\""}` — the
  JSON in the chunk is itself truncated. Mode C.
- **[#1622 — OpenAI OSS tool calling](https://github.com/sst/opencode/issues/1622)**
  gpt-oss via LM Studio uses the OpenAI Harmony format; opencode's
  OpenAI-compatible provider doesn't transcribe it, so tool calls
  sometimes never parse.
- **[#4428 — Why is opencode not working with local LLMs via Ollama?](https://github.com/anomalyco/opencode/issues/4428)**
  Symptom catalog: XML tags, hallucinated tool names, sometimes right args,
  sometimes prose-only. Tagged `model-problem`.
- **[#3077 — Expected thinking or redacted_thinking, but found tool_use](https://github.com/sst/opencode/issues/3077)**
  Provider emits `tool_use` interleaved with a reasoning block that the
  orchestrator can't reassemble — analogous to mode A in that the call is
  detected but the args payload is unusable.
- **[#13900 — GLM-5 via NVIDIA NIM emits malformed MCP tool JSON](https://github.com/anomalyco/opencode/issues/13900)**
  Missing `}` in the args string. Mode C.

Adjacent-but-related Vercel AI SDK issue (this is what opencode uses under
the hood for the OpenAI-compatible provider):

- **[vercel/ai #6687 — Tool Calls Fail with Empty Arguments](https://github.com/vercel/ai/issues/6687)**
  The exact "arguments are `""` not `"{}`" → silently dropped tool call.
  **Fixed in main.** The fix path is the recommended pattern for any
  in-house orchestrator.

Other ecosystems with the same shape:

- **[BerriAI/litellm #5063](https://github.com/BerriAI/litellm/issues/5063)**
  Anthropic stream emits `""`; litellm must coerce to `"{}"` at
  `content_block_stop`. Fixed.
- **[microsoft/semantic-kernel #9212](https://github.com/microsoft/semantic-kernel/issues/9212)**
  Empty `FunctionArgumentsUpdate` was null-derefing; fix added explicit
  IsEmpty validation.
- **[langchain4j #2711](https://github.com/langchain4j/langchain4j/issues/2711)**
  Streamed tool args can't be coerced to the strongly-typed `ToolType`
  enum; same root cause.
- **[OpenAI community: function call returns no arguments](https://community.openai.com/t/chat-api-and-function-calling-returns-no-arguments/628005)**
  Streaming `toolCallDone` event has `arguments: ""` even after the full
  call should be known — confirmation that this is a *protocol-level*
  edge case, not specific to one SDK.
- **[OpenAI community: malformed function-calling arguments](https://community.openai.com/t/malformed-function-calling-arguments/272803)**
  Wider taxonomy of malformed args: backticks instead of quotes, doubled
  JSON, missing fields. Worth skimming for the "formatting" patterns.

---

## 5. Mitigations the ecosystem uses (catalog, in priority order)

1. **Coerce `null` / `""` → `"{}"` before parsing.** Verbatim from
   litellm #5063 and the vercel/ai #6687 fix. One-line defense against the
   "LLM claims it provided args, tool gets nothing" symptom in modes A and B.
2. **Reject-and-repair loop.** When `args_str` doesn't parse, append a
   `{"role":"tool","tool_call_id":cid,"content":"Error: …"}` so the model
   sees the failure and retries. The OpenAI cookbook and the
   [vercel/ai docs tool-call-repair](https://sdk.vercel.ai/docs/ai-sdk-core/tools-and-tool-calling#tool-call-repair)
   document this pattern.
3. **Detect mode D (XML-in-content) and salvage.** Anomalyco/opencode #234
   added an opt-in `provider.options.toolParser` to recover
   `<tool_call>` / `<function=…>` blocks out of `content` and re-emit them
   as structured `tool_calls`. Not free, but the only path for Qwen /
   Hermes / GLM-4 in production.
4. **Constrained decoding / grammar-constrained generation.** Removes
   formatting errors at the source. Requires an inference stack that
   supports it (vLLM, guidance, outlines, lm-format-enforcer). Eliminates
   mode C entirely.
5. **Forensic logging.** Persist the raw `message` JSON before the
   executor touches it. This single line is what makes the rest of the
   diagnosis possible — without it, "tool got null" is a mystery and
   with it, you can tell mode A from B from C in a single grep.
6. **Schema-level defense.** For each tool, validate parsed args against
   the declared JSON schema; if validation fails, return a structured
   error to the model rather than a 500. The Anthropic/OpenAI docs
   recommend this; LiteLLM's `modify_params=True` does the equivalent at
   the message-sanitization layer.

---

## 6. What this means for the ppai codebase (concrete code path)

The "null arguments" symptom in this repo lands here:

```
process_turn (agent_impl.rs)
  └─ tool_calls: &[serde_json::Value]   // from message.tool_calls
       └─ executor.execute_all(tc, …)
            └─ extract_str(tc, &["function", "arguments"])
                 └─ if .as_str() is None → returns ""
            └─ execute_tool(&ctx, &func_name, &func_args)   // func_args = ""
                 └─ TOOL_REGISTRY.execute(ctx, name, "")    // registry.rs:126
                      └─ serde_json::from_str("") → error
```

So a `null` from the provider becomes `""` in the tool, which then JSON-parse
errors. The error text becomes the `tool` message back to the LLM on the
next turn. That's why the user-visible symptom in this codebase is usually
"the LLM says it called the tool, but nothing happened and the next turn
the LLM apologizes" — the LLM never actually saw a success result, only a
parse error string.

**Three small, well-scoped changes worth considering (no code yet, this is
just research):**

1. In `extract_str`, when the target value is `Value::Null`, return
   `"{}"` instead of `""`. Removes mode A and mode B at the boundary.
2. In `execute_tool` (registry.rs:126), catch the
   `serde_json::from_str` failure and return a structured error like
   `{"status":"error","message":"arguments could not be parsed as JSON"}`
   instead of letting it propagate as a panic. The model already gets
   this as a `tool` message — making it a *clean* error makes repair
   loops effective.
3. Log the raw `tool_calls` payload (in `agent_impl.rs::process_turn`,
   before passing to the executor) at `tracing::info!` with a redacted
   args preview. One line, but it makes future "null args" debugging
   trivial — same idea as point 5 in §5.

None of these are big refactors. Each is a self-contained one-or-two-liner
that closes a specific class of failure without changing the orchestrator
shape.

---

## 7. TL;DR

The "LLM claims it provided args, tool gets null" pattern is a real,
well-documented bug class that hits every agentic framework that wraps
OpenAI/Anthropic Chat Completions. The two highest-leverage causes are:

- **Provider/SDK serializes empty arg lists as `""`** instead of `"{}"`
  (Anthropic via OpenAI-compatible, some Qwen, GLM-5, gpt-oss/Harmony).
  Fix: coerce on the way in.
- **Stream parser drops the call when `isParsableJson("")` is false**.
  Fix: treat `""` and `"{}"` equivalently. Already done in
  vercel/ai main (issue #6687).

Opencode carries a long tail of related reports (#1406, #1034, #1809,
#800, #1622, #4428, #3077, #13900) but most are model-format (mode D) or
provider-truncation (mode C) — different symptoms, same family. The
cleanest defensive posture is a four-line fix at the orchestrator
boundary (this repo: `tool_executor.rs` + `registry.rs` + one log line in
`agent_impl.rs`).
