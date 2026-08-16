# Code Review: Vector Search Feature Branch

**Review protocol**: `review-code` (systems-engineer persona)  
**Language**: Rust (edition 2024)  
**Scope**: `git diff main...HEAD` — vector-search feature + extensions plumbing  
**Reviewer**: opencode (automated)  
**Date**: 2026-08-14

---

## Anti-hallucination protocol

All findings below are grounded in specific file paths and line numbers
from the source tree as it exists in the branch. No behavior is inferred
that is not directly visible in the code.

---

## Findings

### 1. [SEVERITY: Medium] — Unnecessary thread creation per search call

**Location**: `src/desktop/src/app/background/vector_search.rs:153-163` (`VectorSearchService::search`)

**Issue**: `search` spawns a new `std::thread::spawn`, moves the service
and query into it, then immediately calls `.join()` and blocks until it
completes. The thread provides no concurrency benefit — the caller is
still blocked waiting for the result. The only value is panic isolation
(if `search_inner` panics, `join()` catches it and returns an error).

**Evidence**:
```rust
fn search(&self, query: &str, limit: usize) -> Result<...> {
    let service = self.clone();
    let query = query.to_string();
    std::thread::spawn(move || service.search_inner(&query, limit))
        .join()
        .map_err(|_| "Vector search worker panicked.".to_string())?
}
```

**Why it matters**: The tool-execution path already runs on a
`tokio::task::JoinSet::spawn_blocking` worker thread inside
`ToolExecutor::execute_parallel` (`tool_executor.rs:211`). Spawning
another raw OS thread per search call adds ~50 KB of stack allocation,
scheduler context-switch latency, and a kernel syscall — all for zero
parallelism gain. Each vector-search tool call in an LLM conversation
pays this overhead.

**Suggestion**: Replace with `std::panic::catch_unwind` for lightweight
panic isolation:
```rust
fn search(&self, query: &str, limit: usize) -> Result<...> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        self.search_inner(query, limit)
    })).map_err(|_| "Vector search worker panicked.".to_string())?
}
```
Or, if panics are acceptable, call `self.search_inner` directly.

---

### 2. [SEVERITY: Medium] — Mutex held during slow embedding and disk I/O

**Location**: `src/desktop/src/app/background/vector_search.rs:167-184` (`search_inner`) and `vector_search.rs:116-141` (`index_path`)

**Issue**: The `Arc<Mutex<VectorState>>` lock is acquired and then held
for the entire duration of `model.embed()` — a potentially slow ONNX
Runtime inference — and for `database.save_collection()` — a disk write.
This serializes all concurrent search and indexing operations.

**Evidence** (`search_inner`, lines 173-188):
```rust
let state = self.state.lock()...?;
let model = state.model.as_ref()...;
let embedding = model.embed(vec![query], None)...?;  // slow — lock held
let results = state.collection.search(...)?;           // lock still held
```

**Evidence** (`index_path`, lines 121-141):
```rust
let Ok(mut state) = self.state.lock() else { return; };
let Some(model) = state.model.as_ref() else { return; };
let Ok(embeddings) = model.embed(texts, None) else { return; };  // slow — lock held
for (chunk, embedding) in chunks.into_iter().zip(embeddings) {
    // ... inserts ...
}
if let Some(database) = state.database.as_mut() {
    let _ = database.save_collection(...);  // disk I/O — lock held
}
```

**Why it matters**: While `search_inner` holds the lock during embedding,
the background indexing thread (in `start`, line 73) cannot make
progress on new file events. While `index_path` holds the lock during
embedding and `save_collection`, no search can execute. The vector
index effectively becomes single-threaded under load.

**Suggestion**: Restructure so the lock is held only for the actual
collection read/write, not for the embedding computation. Options:
- Store the `TextEmbedding` model behind an `Arc` so a clone can be
  taken, the lock released, embedding computed outside the lock, then
  re-acquired for the collection search.
- Split `VectorState` into two mutexes: one for the model (immutable
  after initialization) and one for the collection/database.

---

### 3. [SEVERITY: Medium] — `remove_path` is a no-op; deleted files leave stale index entries

**Location**: `src/desktop/src/app/background/vector_search.rs:144`

**Issue**: The `FileEventKind::Removed` handler calls
`service.remove_path(&path)`, but `remove_path` does nothing. Deleted
Markdown files retain their chunks in the SahomeDB collection, and
search results will return content from files that no longer exist on
disk.

**Evidence**:
```rust
fn remove_path(&self, _path: &Path) {}
```

**Why it matters**: User-facing correctness — the LLM tool will return
search hits pointing to deleted files. This is a data-integrity issue:
the index is out of sync with the filesystem.

**Suggestion**: Implement `remove_path` to query the collection's
records, filter those whose stored metadata path matches, and delete
them. Check the SahomeDB `Collection` API for filter-based deletion.

---

### 4. [SEVERITY: Medium] — No deduplication on re-index; `MarkdownChunk.hash` is unused

**Location**: `src/desktop/src/app/background/vector_search.rs:116-141` (`index_path`) and `vector_search.rs:229-248` (`markdown_chunks`)

**Issue**: When a file is updated and `index_path` is called again, new
records are inserted into the SahomeDB collection without removing the
old records for the same file. The `MarkdownChunk.hash` field is
computed via SHA-256 but is never used to detect or skip unchanged
chunks. This causes:
- Index bloat (duplicate chunk vectors per file update)
- Stale results (old versions of chunk content remain searchable)

**Evidence**:
```rust
// markdown_chunks computes hash:
MarkdownChunk { path: ..., hash: hash(&text), text }
// but index_path never checks it:
let _ = state.collection.insert(&record);  // unconditional insert
```

**Why it matters**: Over time, frequently-edited Markdown files will
accumulate multiple versions of their chunks in the index, degrading
search quality and increasing memory/disk usage. The hash was clearly
intended for deduplication but was never wired up.

**Suggestion**: Before inserting, query existing records whose metadata
starts with the file path, remove them, and insert fresh chunks. Use
the `hash` field to skip re-embedding unchanged chunks.

---

### 5. [SEVERITY: Low] — Inconsistent `with_extensions` semantics across builder types

**Location**:
- `src/desktop/src/agent/tools/context.rs:150-153` (`ToolContextBuilder`) — **extends** (merge)
- `src/desktop/src/agent/context.rs:187-189` (`AgentContextBuilder`) — **replaces**
- `src/desktop/src/agent/tool_executor.rs:83-86` (`ToolExecutorBuilder`) — **replaces**

**Issue**: `ToolContextBuilder::with_extensions` merges incoming
extensions into the existing map (`self.extensions.extend(&extensions)`),
while `AgentContextBuilder` and `ToolExecutorBuilder` overwrite entirely
(`self.extensions = extensions`). This doesn't cause a bug in the
current call flow (each builder is called with `with_extensions` exactly
once, after no prior `with_extension` calls), but it is a latent
footgun: a future caller who chains `.with_extension(X).with_extensions(Y)`
on `AgentContextBuilder` or `ToolExecutorBuilder` would silently lose `X`.

**Evidence**:
```rust
// ToolContextBuilder — merge:
pub fn with_extensions(mut self, extensions: ...) -> Self {
    self.extensions.extend(&extensions);  // merge
    self
}

// AgentContextBuilder — replace:
pub fn with_extensions(mut self, extensions: ...) -> Self {
    self.extensions = extensions;        // replace
    self
}
```

**Suggestion**: Make all three use `extend` (merge) semantics for
consistency, or rename the replace-variant to `set_extensions` to make
the distinction explicit at the call site.

---

### 6. [SEVERITY: Low] — Unsanitized error strings forwarded to the LLM

**Location**: `src/desktop/src/app/background/vector_search.rs:183-188` (`search_inner`)

**Issue**: Errors from `model.embed()` are forwarded via `e.to_string()`
and SahomeDB search errors via `e.message().to_string()`. These strings
may contain internal file paths, library internals, or model details
that become visible to the end user in the chat panel (since tool-result
errors are displayed as chat messages).

**Evidence**:
```rust
let embedding = model
    .embed(vec![query], None)
    .map_err(|e| e.to_string())?                       // raw error → LLM + UI
    .remove(0);
// ...
let results = state
    .collection
    .search(&Vector::from(embedding), limit)
    .map_err(|e| e.message().to_string())?;              // raw error → LLM + UI
```

**Suggestion**: Wrap these in a sanitized, generic message (e.g.,
`"Vector search failed. See background logs for details."`) and log
the detailed error via `tracing::error!` for operators.

---

### 7. [SEVERITY: Low] — Missing REQ-xxx requirement citations in new code

**Location**: `src/desktop/src/agent/tools/vector_search.rs` (new file), `src/desktop/src/app/background/vector_search.rs` (new file), `src/desktop/src/app/background_task.rs` (modified), `src/desktop/src/agent/agent_impl.rs` (modified)

**Issue**: Per `AGENTS.md [RUST-042]`, new or changed code SHOULD cite
`REQ-xxx` identifiers in `//!` / `///` doc comments. The new vector-search
modules have descriptive doc comments but no traceability links. The
`SPEC.md` update contains descriptive prose but no traceable
requirement IDs.

**Evidence**: No `REQ-` or `AGENT-0xx` tags appear in the new or modified
source files (the `log_prompt_context` test cites `AGENT-026`, but that
is pre-existing in `agent_impl.rs`).

**Suggestion**: Define requirement IDs in `SPEC.md` for the vector-search
feature (e.g., `REQ-031` for "semantic Markdown search") and reference
them in the `//!` module comments of both `vector_search.rs` files.

---

### 8. [SEVERITY: Low] — Newline-in-path parsing fragility

**Location**: `src/desktop/src/app/background/vector_search.rs:134` (write) and `vector_search.rs:195-200` (read)

**Issue**: Chunk metadata is stored as a raw newline-delimited string
`format!("{}\n{}", path.display(), chunk.text)` and parsed back with
`splitn(2, '\n')`. If a file path contains a newline character (legal
on Linux and macOS), the path extraction would be incorrect and the text
would include a truncated portion of the path.

**Evidence**:
```rust
// Write:
&Metadata::from(format!("{}\n{}", path.display(), chunk.text))

// Read:
let mut parts = value.splitn(2, '\n');
let path = parts.next().unwrap_or_default().to_string();
```

**Suggestion**: Use a structured format (e.g., JSON-encoded
`serde_json::json!({"path": ..., "text": ...})`) or base64-encode the
path so the delimiter is unambiguous.

---

### 9. [SEVERITY: Nit] — `BackgroundLog` empty struct used as a function namespace

**Location**: `src/desktop/src/app/background/vector_search.rs:272-286`

**Issue**: `BackgroundLog` is a unit struct (`struct BackgroundLog;`)
with two associated functions (`progress` and `failed`) and no fields.
This is a non-idiomatic pattern for namespacing functions in Rust.

**Evidence**:
```rust
struct BackgroundLog;
impl BackgroundLog {
    fn progress(processed: usize) -> crate::bus::events::BackgroundLogEntry { ... }
    fn failed(message: &str) -> crate::bus::events::BackgroundLogEntry { ... }
}
```

**Suggestion**: Replace with free functions (e.g., `fn bg_log_progress` /
`fn bg_log_failed`) or a private submodule. This would also comply with
`AGENTS.md [RUST-051]` (place code by concern, not by type).

---

### 10. [SEVERITY: Nit] — Inline test module instead of sidecar file

**Location**: `src/desktop/src/agent/tools/vector_search.rs:107-121`

**Issue**: The test module uses an inline `#[cfg(test)] mod tests` block.
Per `AGENTS.md [RUST-001]`, unit tests SHOULD be in a separate sidecar
file named `<file>_tests.rs`. The project consistently uses this pattern
elsewhere (e.g., `background_task_tests.rs`).

**Evidence**:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn input_limit_is_bounded_by_tool() {
        assert_eq!(default_limit(), 5);
        // ...
    }
}
```

**Suggestion**: Extract to `src/desktop/src/agent/tools/vector_search_tests.rs`
and declare from the source file with `#[cfg(test)] mod tests;`.

---

## Summary

| Severity | Count |
|----------|-------|
| Medium   | 4     |
| Low      | 4     |
| Nit      | 2     |
| **Total**| **10** |

**Overall assessment**: **Request changes**

The vector-search feature is well-structured: the `Extensions` plumbing
thread through `Extensions::extend`, `ToolContext`,
`ToolExecutor`, `AgentContext`, and `AgentSessionBuilder` is correct,
feature gating is consistent across `mod.rs` files, and the two test
files cover the chunk-hash stability and input-limit clamping.

However, 4 medium-severity issues must be addressed before merge:

1. **Unnecessary thread creation per search** — `spawn` + `join`
   provides no parallelism; `catch_unwind` is lighter.
2. **Mutex held during embedding and disk I/O** — serializes all
   concurrent search/index operations, degrading performance under load.
3. **`remove_path` is unimplemented** — deleted files leave stale
   chunks in the index; search returns content from non-existent files.
4. **No deduplication on re-index** — the computed `MarkdownChunk.hash`
   is never used; updated files accumulate duplicate records.

**Top 3 findings** (ranked by severity × user impact):
1. Mutex held during embedding (`search_inner` and `index_path`) —
   affects all users of the feature, degrades search latency under load.
2. `remove_path` no-op — causes stale results for all users with
   deleted files, a correctness issue.
3. Thread spawn-and-join per search — adds measurable latency to every
   vector-search tool call, compounded when the LLM calls the tool
   multiple times in a conversation.

### Quality checklist

- [x] Every finding cites a specific code location (file:line)
- [x] Every finding has a severity rating (Medium / Low / Nit)
- [x] Every finding includes a concrete fix suggestion
- [x] Findings are ordered by severity (Medium → Low → Nit)
- [x] All 10 findings re-verified against the source files read
- [x] Overall assessment stated: **Request changes**
- [x] Top 3 highest-severity items identified in summary

---

## Resolution (2026-08-14)

All findings were addressed on `feature/vector-search-onboarding`.

| Finding | Severity | Resolution |
|---------|----------|------------|
| 1 | Medium | `search` now uses `std::panic::catch_unwind` instead of `spawn`+`join` (`app/background/vector_search.rs`). |
| 2 | Medium | `VectorState.model` is now `Arc<TextEmbedding>`; both `search_inner` and `index_path` clone the model, drop the lock, and embed outside it, re-acquiring only for the collection read/write. |
| 3 | Medium | `remove_path` implemented: lists records, filters by structured path metadata, deletes the IDs, and persists the collection. |
| 4 | Medium | `index_path` deletes stale records whose `hash` is no longer in the file and skips re-embedding chunks whose `hash` is already indexed. |
| 5 | Low | `AgentContextBuilder` and `ToolExecutorBuilder` `with_extensions` now merge via `extend`, matching `ToolContextBuilder`. |
| 6 | Low | `search_inner` surfaces a sanitized generic error string and logs the detail via `tracing::error!`. |
| 7 | Low | Added `AGENT-031` (`agent/SPEC.md`) and `TOOL-043` (`tools/SPEC.md`) and cited them in both `vector_search.rs` module docs. |
| 8 | Low | Chunk metadata is now a structured `Metadata::Object` (`path`/`hash`/`text` keys) instead of a newline-delimited string. |
| 9 | Nit | `BackgroundLog` unit struct replaced with `bg_log_progress` / `bg_log_failed` free functions. |
| 10 | Nit | Tests extracted to `agent/tools/vector_search_tests.rs` sidecar. |

Quality gate (from `src/desktop/`): `cargo check`, `cargo clippy -- -D warnings`,
`cargo fmt --check`, and `cargo doc --no-deps` all pass for default features and
`--features vector-search`; `cargo nextest run` passes (978 tests). The
vector-search test binary does not link in this environment due to a pre-existing
`mimalloc` CRT (MT vs MD) mismatch unrelated to these changes.
