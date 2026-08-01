# Browser tools for the LLM agent

Status: proposal
Date: 2026-08-01

## Context

The `ppai` (fastmd) agent already ships `web_fetch`, `web_search`, and
`web_delegate` (SearXNG + url-to-markdown). These are *read-only* tools
that fetch and convert HTML to markdown — fine for static content, useless
for anything that needs JavaScript, login flows, or multi-step interaction.

In `src/agent/tools/browser.rs` we already have a Playwright-based
implementation:

- `browser_navigate(&Page, url)`
- `browser_get_page_state(&Page) -> String` (returns
  `{agent_id, tag, text, placeholder}`)
- `browser_click(&Page, selector)`
- `browser_fill_input(&Page, selector, text)`
- `browser_select_dropdown(&Page, selector, value)`
- `browser_press_key(&Page, key)`
- `browser_evaluate_js(&Page, script) -> String`

`playwright-rs = "0.14.1"` is already in `Cargo.toml`. The in-module
integration tests are commented out because they need Playwright browsers
installed locally.

**The gap**: none of these are registered as LLM-callable tools. There is
no `Browser` group in `InternalToolGroup`, no `tool_groups.browser` field
in `config.yaml`, no `Tool` trait implementations, no DTOs, no
registration in `register_all_builtins`, no `ToolContext` plumbing for
the long-lived page handle the Playwright functions need, and no
documentation entry. From the LLM's perspective, this code does not
exist.

This ADR closes that gap: turn the dead-code Playwright helpers into
first-class tools, registered under a new `Browser` group, so the agent
can drive a real headless Firefox to log in, click, fill, and read
JS-rendered content.

## Decision

### 1. New `Browser` group (not folded into `Web`)

The three existing web tools are stateless and read-only. Browser tools
are stateful (one persistent `Page` per session) and largely mutating
(clicking changes page state). Mixing them under one toggle makes it
impossible to keep `web_fetch`/`web_search` on while disabling
`browser_*`, which is a reasonable posture for users who don't want a
headless browser running. Independent toggle is also better for
context-budget accounting (parallel-safe differs per group).

Concretely:

```rust
// src/agent/tools/manager/groups.rs
pub enum InternalToolGroup {
    Filesystem, Web, Browser,   // <-- new
    Email, Contacts, Calendar, CsvDb, Weather,
}
```

### 2. Default `tool_groups.browser: false` (opt-in)

Consistent with the README's "no bash / no system access" philosophy.
Users turn the group on when they need it. Default `false` keeps the
default install surface minimal — no Chromium/Firefox subprocess until
the user explicitly opts in.

### 3. Browser engine: Firefox (Playwright's firefox channel)

`playwright-rs` exposes the same `BrowserType` enum as upstream
Playwright; we use `playwright.firefox()` instead of `playwright.chromium()`.
This matches the rest of the user's stack (their Fidalgo setup already
runs Playwright on Firefox per the user profile).

**Install requirement** (documented in `src/desktop/AGENTS.md`):
`playwright install firefox` must be run once by the user before the
first browser tool call. The startup path does not auto-install;
missing-browsers surface as a `ToolGroupError::Discovery` with a clear
hint.

### 4. Persistent `Page` with cookie persistence across app restarts

A new `app::browser::BrowserSession` resource owns a long-lived
`Playwright` + `Browser` + `Page`. Lazy launch on first use.

**Cookie / storage persistence** (user override of the initial lean):

- On first launch, load Playwright `storage_state` from
  `%APPDATA%\fastmd\browser-storage.json` if it exists.
- After every mutating tool call, the session saves the current
  `storage_state` to that same file (debounced — at most once every
  few seconds, never blocking the tool call).
- Login state therefore survives app restarts. The user does not
  have to re-authenticate after closing and reopening the app.
- A `Forget Browser Session` action in the Tools dialog deletes the
  file and closes the live browser, giving a clean logout.

**Idle timeout** (`browser.idle_timeout_seconds`, default 300):
after N seconds of no tool calls, the session closes the Firefox
process and clears the in-memory `Page`. The next call relaunches.
On relaunch the storage file is reloaded, so persistent cookies
survive idle timeouts too.

**Sync-to-async bridge**: reuse the existing
`tools::blocking::block_on` (already used by CalDAV/CardDAV) to drive
the Playwright futures from the sync `Tool::execute`. The runtime is
process-wide.

### 5. Tool list to expose

| Name | Args | Safety | Purpose |
|---|---|---|---|
| `browser_navigate` | `url: str` | Mutating | Navigate the persistent page to a URL |
| `browser_get_page_state` | — | ReadOnly | Return interactable elements (`a` / `button` / `input` / `select` / `textarea`) with `agent_id`, `tag`, `text`, `placeholder`, plus current `url` and `title` |
| `browser_click` | `selector: str` | Mutating | Click a single element by CSS selector |
| `browser_fill_input` | `selector: str, text: str` | Mutating | Fill a single input/textarea |
| `browser_select_dropdown` | `selector: str, value: str` | Mutating | Pick a `<select>` option |
| `browser_press_key` | `key: str` | Mutating | Press a key (Enter, Tab, Escape, ...) |
| `browser_evaluate_js` | `script: str` | Mutating | Escape hatch — evaluate arbitrary JS, return JSON |
| `browser_screenshot` | `filename: str`, `full_page: bool = false` | Mutating | Save a PNG; return the absolute path. Path is validated against `browser.screenshot_dir` (see §6). |

Notes:

- `browser_get_page_state` is the only `ReadOnly` tool — must be
  parallel-safe so the agent can use it to "look around" while
  another safe tool runs.
- All others are `Mutating`. `browser_evaluate_js` in particular
  could literally do anything, so it can never be classified as
  read-only reliably.
- `browser_close` is intentionally **not** exposed. The app owns
  the session lifecycle (idle timeout, storage save). The LLM
  does not get to close the browser; the system does on its own.
- DTOs go in `src/agent/tools/dtos.rs`, user-visible strings in
  `manager/builtin/strings/browser.rs` (new file), `Tool` impls
  in `manager/builtin/browser.rs` (new file), registered with
  `register_builtin(InternalToolGroup::Browser, Box::new(...))`
  in `register_all_builtins`.

### 6. Config surface

```yaml
tool_groups:
  browser: false          # default off; opt-in (BRWS-CONF-001)

browser:
  # Path policy
  screenshot_dir: ""      # absolute path; default "" → falls back to
                          # <first content library>/browser-screenshots/

  # Engine
  headless: true          # set false for visual debugging only
  browser_type: firefox   # only "firefox" supported today

  # Lifecycle
  idle_timeout_seconds: 300
  page_load_timeout_ms: 30000

  # Storage
  storage_state_path: ""  # default "" → %APPDATA%\fastmd\browser-storage.json
```

- `ToolGroupsConfig` gains `pub browser: bool` next to the existing
  fields (serde-default `false`).
- New `BrowserConfig` struct in `config.rs` with the fields above.

### 7. Restricted screenshot path

Per the user override: the LLM cannot write screenshots to arbitrary
filesystem paths.

- `browser.screenshot_dir` configures the **only** writable
  destination. If empty, the default is
  `<first content library>/browser-screenshots/` (created on first
  write).
- The `browser_screenshot` tool takes a `filename` (not a full
  path), sanitises it (no `..`, no absolute paths, no path
  separators, max 128 chars, restricted to `[A-Za-z0-9._-]`), and
  joins it with the configured directory. Anything else returns
  an error envelope — the LLM cannot escape the directory.
- The Tools dialog offers a click-to-open action on the resulting
  absolute path so the user can view screenshots.

### 8. UI surface

- The Tools dialog (`src/ui`) currently lists seven groups. Add
  "Browser" with the same checkbox pattern. When the user toggles
  it on, surface a small note: "Launches a headless Firefox on
  first use. Cookies persist across restarts. Idle timeout N
  seconds." Provide a "Forget Browser Session" action next to the
  toggle.
- A new "Browser" section in the config screen for the
  `BrowserConfig` fields.
- Screenshot responses render a clickable absolute path.

### 9. Documentation

- `src/desktop/Tools.md` — add a "Browser Automation" section with
  per-tool Request/Response blocks matching the existing style.
- `src/desktop/src/agent/tools/SPEC.md` — append a Browser section
  with EARS requirements (`BRWS-001..`). Update TOOL-014 to say
  "eight built-in groups" instead of seven.
- `src/desktop/src/agent/tools/mod.rs` — add `pub mod browser;`
  (the file exists but is not a module).
- `src/desktop/SPEC.md` — update the tool list in the high-level
  spec.

### 10. Testing

The commented-out tests in `browser.rs` need to come back, but
moved out so they don't get compiled in CI:

- Move to a separate `#[cfg(test)] mod tests` in the same file,
  gated with `#[ignore = "requires Playwright Firefox installed"]`.
- Document in `src/desktop/AGENTS.md` that the browser tests need
  `playwright install firefox` to be runnable, and that they are
  invoked with `cargo test -- --ignored` (or
  `cargo nextest run --run-ignored all`).
- Add unit tests for the DTO round-trip and `is_enabled` predicate
  (so a regression in the config wiring is caught without
  Playwright).
- Add a contract test that asserts the new `Browser` group shows
  up in `ToolManager::groups_snapshot` and that the eight tool
  names appear in `get_tools_schema` when enabled.

## Consequences

### Positive

- The LLM can drive interactive sites (login + multi-step flows,
  JS-rendered SPAs, anything behind authentication).
- Cookies persist across restarts, so a one-time login keeps
  working — the user does not have to re-authenticate after
  every app restart or idle timeout.
- Headless Firefox matches the rest of the user's stack.
- Default-off and restricted screenshot path preserve the
  README's "no system access" philosophy.
- The persistent `Page` model means the agent does not pay
  Firefox-launch latency on every tool call.

### Negative / risks

- A long-lived Firefox process is a heavier dependency than
  Chromium would be; on a system that does not have
  `playwright install firefox` run, the first tool call will
  fail with a `Discovery` error until the user installs it.
- Persisting cookies to disk in `%APPDATA%` is a privacy
  consideration. Mitigations: the `Forget Browser Session`
  action, clear documentation, and the `storage_state_path`
  config so users can point it at an encrypted volume if they
  want to.
- `browser_evaluate_js` is a true escape hatch — the LLM can run
  any JavaScript in the page context. This is by design (the
  user already accepts that the LLM can do anything via
  `web_delegate`), but worth flagging.
- The `Arc<BrowserSession>` adds a new field to `ToolContext`,
  which is a non-trivial change to a type that is currently
  borrowed-only. Every existing `ToolContext::new` call site
  needs to be updated.
- The `app::browser` module is a new home for a long-lived
  resource; we do not have a precedent in this codebase for
  owning a subprocess that survives across agent turns. The
  component-level `AGENTS.md` for the new module should
  document the lifecycle invariants.

## Plan of work (task order, no implementation yet)

1. **Wire the new group and config** — add `Browser` to
   `InternalToolGroup`, `browser: bool` to `ToolGroupsConfig`, the
   `BrowserConfig` struct, default-`false` semantics, the UI
   toggle hook, and the `AppHandle` plumbing for `BrowserSession`.
   No tools yet. `cargo check` must stay clean.
2. **Implement `BrowserSession`** — the `app::browser` module with
   lazy Firefox launch, persistent `Page`, idle-timeout close,
   storage-state save/load, `block_on` bridge. Add
   `Arc<BrowserSession>` to `ToolContext`. Tests for "session
   returns same page across calls" and "storage state round-trips".
3. **Implement the eight tools** — DTOs in `dtos.rs`, strings in
   `manager/builtin/strings/browser.rs`, `Tool` impls in
   `manager/builtin/browser.rs`, `register_all_builtins` calls.
   All `cargo check` / `cargo clippy` / `cargo test` clean.
4. **Documentation** — `Tools.md`, `SPEC.md`, the high-level
   `src/desktop/SPEC.md`, the inline `pub mod browser;` export.
5. **Browser integration tests** — re-enable the two Playwright
   tests as `#[ignore]` integration tests. Add the DTO and
   `is_enabled` unit tests that don't need Firefox.
6. **End-to-end manual smoke** — start the app, enable Browser in
   the Tools dialog, ask the agent to log into a test site,
   restart the app, confirm the login persists, screenshot a
   page. Capture a short log in the commit message.

## What this plan does NOT cover

- Headless detection / stealth / proxy support.
- Multi-tab / multi-page (one persistent page only).
- Network interception (the LLM cannot see / rewrite requests yet
  — a future `browser_route` or `browser_intercept` could go
  here).
- Visual / screenshot diffing.
- File download handling (a future `browser_handle_download` is a
  clean follow-up).
- Chromium or WebKit support (Firefox-only for v1; the
  `browser_type` config is a string in case we add others later).

These are deliberately deferred to keep the first PR reviewable.
