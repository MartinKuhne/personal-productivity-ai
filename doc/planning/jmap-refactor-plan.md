# JMAP Tools Refactor: Adopt jmap-client Crate

Status: proposal
Date: 2026-07-26

## Context

The JMAP tools (`src/desktop/src/tools/jmap/`) currently hand-code the entire JMAP protocol: HTTP session fetches, request/response JSON construction, error parsing, and account/capability resolution. The project already depends on two crates that implement these:

- **`jmap-client` 0.4.2** — Full JMAP Core + JMAP for Mail implementation with typed types (`Email`, `Filter`, `Property`, `EmailAddress`, etc.), builder patterns, and methods like `email_query()`, `email_get()`, `email_import()`, `mailbox_*()`.
- **`jmap-calendars-client` 0.1.2** — JMAP Calendars method implementations (19 methods) as an extension trait on the base client.

### Current Architecture

```
client.rs:
  get_jmap_session()   → ureq::get(url) → parse session JSON manually
  jmap_call()          → ureq::post(url) → build {"using":[...], "methodCalls":...} manually
  jmap_check_errors()  → parse methodResponses array for "error" entries
  get_account_id()     → index into primaryAccounts JSON

email.rs:
  tool_search_email()  → build Email/query + Email/get JSON, parse responses manually
  tool_get_email_by_id() → build Email/get JSON, parse responses manually
  tool_send_email()    → build Email/set + EmailSubmission/set JSON

contacts.rs:
  tool_search_contact() → build Contact/query + Contact/get JSON
  tool_get_contact()    → build Contact/get JSON
  tool_add_contact()    → build Contact/set JSON

calendar.rs:
  tool_search_calendar()  → build CalendarEvent/query + CalendarEvent/get JSON
  tool_get_calendar()     → build CalendarEvent/query + CalendarEvent/get JSON
  tool_get_calendar_item()→ build CalendarEvent/get JSON
  tool_add_calendar_item()→ build CalendarEvent/set JSON
  tool_update_calendar_item()→ build CalendarEvent/set JSON
  tool_delete_calendar_item()→ build CalendarEvent/set JSON
```

Each tool manually iterates over `config.jmap_clients`, calls `get_jmap_session` then `jmap_call`, then parses `methodResponses` JSON.

### Target Architecture

```
client.rs (refactored):
  JmapSession — thin wrapper around jmap_client::Client + session state
  get_session() → Client::new().credentials("token").connect(url)
  account_id(client, capability) → client.session().primary_account_id(cap)

email.rs (refactored):
  tool_search_email() → client.email_query(Filter, Comparator[]) → QueryResponse
                       → client.email_get(id, Property[]) → Vec<Email>
  tool_get_email_by_id() → client.email_get(id, Property[]) → Option<Email>
  tool_send_email() → client.email_import(raw_bytes, [...], ["$draft"], None)

contacts.rs (refactored):
  Uses jmap_client typed methods if available, otherwise stays as-is
  (jmap-client does not have Contact-specific typed methods)

calendar.rs (refactored):
  Uses jmap_calendars_client extension trait methods
```

## Decision

Replace the hand-coded JMAP transport layer with the `jmap-client` crate's `Client`, using its `blocking` feature for synchronous operations. For calendar operations, use `jmap-calendars-client`'s extension trait.

### Why the blocking feature?

The current `tool_*` functions are synchronous (they return `Result<T, String>` directly). Using `jmap-client` with the `blocking` feature provides synchronous `reqwest::blocking` HTTP calls while keeping the same `Result<T, String>` return signature. This minimizes changes to the tool interface layer.

### HTTP client change

`jmap-client` uses `reqwest` internally, not `ureq`. We must:

1. Add `jmap-client = { version = "0.4.2", features = ["blocking", "ring"] }` to `Cargo.toml`
2. Add `reqwest = { version = "0.13", features = ["rustls", "json", "blocking"] }` as a direct dependency
3. Remove `ureq` if no other code depends on it

### What the crates provide vs. what we currently hand-code

| Current hand-coded | Replaced by crate |
|---|---|
| `ureq::get` session fetch + JSON parse | `Client::new().credentials().connect()` |
| `ureq::post` request + JSON body build | `Client.send(request)` |
| `{"using":[...], "methodCalls":...}` | `core::request::Request` |
| `methodResponses` error detection | `core::response::Response` with typed errors |
| `primaryAccounts` JSON indexing | `client.session().primary_account_id(cap)` |
| Capability checking | `client.session().has_capability(cap)` |
| `Filter::text`, `Filter::from`, etc. | `email::query::Filter::text()`, etc. |
| `Property::Subject`, etc. | `email::Property::Subject` enum |
| `Email` struct with all fields | `jmap_client::email::Email` |
| `EmailAddress` struct | `jmap_client::email::EmailAddress` |

### What stays hand-coded (no crate equivalent)

- `convert_html_in_jmap()` — HTML-to-Markdown conversion via `fast_h2m` (domain logic)
- `simplify_jmap_emails()` — Response simplification for LLM consumption (domain logic)
- Contact operations — `jmap-client` has no typed Contact methods (no JMAP Contacts spec exists)
- Pagination logic — client-side pagination across multiple JMAP accounts (domain logic)

### Calendar operations approach

Two options:

**Option A: Use `jmap-calendars-client` (Recommended)**

```rust
use jmap_calendars_client::JmapCalendarsExt;

let session = client.session();
let cal_client = client.with_calendars_session(session);
cal_client.calendar_event_query(...).await?
```

- Pros: Typed `QueryResponse`, proper error types, follows RFC draft
- Cons: The blocking feature propagates through `jmap-base-client`; needs careful feature flag alignment

**Option B: Keep hand-coding calendar JSON**

The calendar code is already well-structured and simple. The crate provides marginal value here since we don't use typed calendar objects. However, to satisfy "full use of the JMAP crate", we should at minimum use the shared session/auth layer.

We recommend **Option A** but will accept Option B if blocking feature conflicts arise.

## Impact Assessment

### Dependencies

```diff
 [dependencies]
+ureq = { version = "2.9", features = ["json"] }  # kept: used by vision_processor, llm_client, web, weather
+jmap-client = { version = "0.4.2", features = ["blocking", "ring"] }
+reqwest = { version = "0.13", features = ["rustls", "json", "blocking", "http2"] }
```

`ureq` remains (used by `vision_processor.rs`, `llm_client.rs`, `web.rs`, `weather.rs`).

### Files changed

| File | Change |
|---|---|
| `src/desktop/Cargo.toml` | Swap `ureq` for `jmap-client` + `reqwest` |
| `src/desktop/src/tools/jmap/client.rs` | Rewrite: wrap `jmap_client::Client`, provide `JmapSession` |
| `src/desktop/src/tools/jmap/email.rs` | Rewrite: use typed `email_query`, `email_get`, `email_import` |
| `src/desktop/src/tools/jmap/contacts.rs` | Rewrite: use shared session layer, typed contact methods if available |
| `src/desktop/src/tools/jmap/calendar.rs` | Rewrite: use `jmap_calendars_client` extension trait |
| `src/desktop/src/tools/jmap/tests.rs` | Update: tests for new `JmapSession` methods |
| Inline tests in email/contacts/calendar | Update to use new types |
| `src/desktop/src/config.rs` | May need minor changes if `JmapClient` struct changes |

### Risks

1. **`jmap-client` uses `reqwest` not `ureq`** — If `reqwest` is already pulled in transitively by other deps, no new bloat. If not, this adds a larger HTTP client.
2. **Blocking feature feature conflicts** — `jmap-client` `blocking` feature requires `reqwest/blocking`. `jmap-calendars-client` depends on `jmap-base-client` which also uses `maybe-async`. Need to verify feature alignment.
3. **Async vs sync** — `jmap-calendars-client` methods are async. If blocking feature doesn't propagate, we need `tokio::task::spawn_blocking` wrappers.
4. **Type compatibility** — `jmap_client::email::Email` fields differ slightly from our hand-parsed JSON. `simplify_jmap_emails()` will need to consume typed objects instead of raw `Value`.
5. **Test infrastructure** — Current tests use TCP mock servers with `ureq`. `reqwest` mocks will need different setup (likely `wiremock` or in-process mock).

## Implementation Plan

### Task 1: Add dependencies

Add `jmap-client` with `blocking` and `ring` features. Add `reqwest` with `blocking` and `rustls` features. Remove `ureq` if unused.

### Task 2: Refactor `client.rs`

Replace hand-coded session/request layer with `jmap_client::Client` wrapper:

```rust
pub struct JmapSession {
    client: jmap_client::Client,
    account_cache: HashMap<&'static str, String>,
}

impl JmapSession {
    pub fn connect(url: &str, token: &str) -> Result<Self, String>
    pub fn account_id(&mut self, capability: &str) -> String
    pub fn has_capability(&self, capability: &str) -> bool
    pub fn inner(&self) -> &jmap_client::Client
}
```

Delete: `get_jmap_session()`, `jmap_call()`, `jmap_check_errors()` (kept only in tests).

### Task 3: Refactor `email.rs`

Replace hand-coded JSON with typed crate methods:

```rust
// Search: use Filter enum
let filter = Filter::text(keyword)
    .and(Filter::in_mailbox(&mailbox_id))
    .and(Filter::has_keyword("$seen"));
let query_resp = session.inner().email_query(filter.into(), comparators.into())?;
let email_ids: Vec<&str> = query_resp.take_ids();

// Get: use Property enum
let emails = session.inner().email_get(&email_ids, 
    [Property::Id, Property::Subject, Property::From, Property::To, 
     Property::HtmlBody, Property::TextBody, Property::BodyValues].into())?;

// Send: use email_import (single call, creates draft + submits)
let raw = format!("To: {}\r\nSubject: {}\r\n\r\n{}", to, subject, body);
session.inner().email_import(raw.into_bytes(), &[mailbox_id], 
    ["$draft".into()].into(), None)?;
```

Update `convert_html_in_jmap()` and `simplify_jmap_emails()` to consume `Vec<Email>` instead of `serde_json::Value`.

### Task 4: Refactor `contacts.rs`

`jmap-client` has no typed Contact methods. Use the shared session layer from Task 2 for connection/auth, but keep the Contact JSON construction for now (no better alternative exists). This reduces duplication: remove `get_jmap_session()` calls and use `JmapSession`.

### Task 5: Refactor `calendar.rs`

Use `jmap-calendars-client` extension trait:

```rust
use jmap_calendars_client::JmapCalendarsExt;

let cal_client = session.inner().with_calendars_session(session.inner().session());
let query_resp = cal_client.calendar_event_query(filter, None).await?;
```

If blocking feature doesn't propagate to calendars, wrap in `tokio::task::spawn_blocking`.

### Task 6: Update tests

- Replace TCP mock servers with `jmap_client` mockable setup or `wiremock`
- Update `tests.rs` for new `JmapSession` methods
- Update inline tests in each module to consume typed response objects
- Verify `cargo test` passes

### Task 7: Quality gate

Run from `src/desktop/`:
- `cargo check`
- `cargo test`
- `cargo clippy -- -D warnings`
- `cargo fmt --check`
- `cargo doc --no-deps --quiet`

## Verification Criteria

1. All `tool_*` functions maintain the same public API (same signatures, same return types)
2. `cargo clippy -- -D warnings` passes with zero warnings
3. All existing tests pass (no test behavior changes, only implementation changes)
4. The `jmap_check_errors()` function is removed from production code (handled by crate)
5. Session/auth is handled by `jmap_client::Client` (no manual HTTP session fetch)
6. Email filter building uses `jmap_client::email::query::Filter` enum
7. Email property requests use `jmap_client::email::Property` enum
8. Calendar operations use `jmap_calendars_client` extension trait
9. No `serde_json::json!()` macros used for JMAP request/response construction (except for domain logic like `simplify_jmap_emails`)
10. `ureq` dependency removed (if unused elsewhere)
