# Quickstart & Validation Guide: Chrome DevTools Integration for Web Fetch

**Feature**: Chrome DevTools Integration for Web Fetch  
**Feature Directory**: `specs/003-chrome-devtools-web-fetch/`  
**Status**: Draft  
**Date**: 2026-09-07  

---

## 1. Prerequisites

- Rust toolchain (Rust 2024 edition) installed.
- Nextest installed: `cargo install cargo-nextest`.
- Optional: Google Chrome, Chromium, or Microsoft Edge installed on the host machine to test live browser rendering.

---

## 2. Test Execution

### 2.1 Run Unit Tests for Web Fetch & Browser Detection
Run the dedicated test sidecar for the web fetch tool family:
```bash
cargo nextest run -p fastmd-agent --test-threads 1 -E 'test(test_tool_web_fetch)'
```
**Expected Outcome**: All tests pass, including:
- Standard mock web fetch.
- Browser detection on supported OS platforms.
- Fallback to HTTP when browser is unavailable or errors.
- Cache hit and cursor pagination consistency across both fetch types.

### 2.2 Run Full Workspace Quality Gate
Run the workspace test suite and lint checks:
```bash
cargo check --quiet
cargo nextest run --workspace
cargo clippy -- -D warnings
cargo fmt --check
```

---

## 3. End-to-End Validation Scenarios

### Scenario 1: Fetching Client-Side JavaScript Rendered Page (Happy Path)
- **Target URL**: A web page requiring client-side JavaScript (e.g. an SPA documentation page with dynamic rendering).
- **Execution**:
  1. Ensure Google Chrome is installed on the host machine.
  2. Invoke `web_fetch` with `url: "<target-spa-url>"`.
- **Expected Outcome**:
  - The response returns Markdown populated with the dynamic text rendered by client-side JavaScript.
  - No graphical window or desktop flash appears.
  - The response contains `total_lines`, up to 64 lines of Markdown, and a valid `cursor` if the document exceeds 64 lines.

### Scenario 2: Automatic Fallback When Browser Is Absent
- **Execution**:
  1. Simulate absence of Chrome (e.g. by setting `AgentConfig.browser.chrome_path` to a nonexistent path or in a headless CI container without Chrome).
  2. Invoke `web_fetch` on any valid web URL.
- **Expected Outcome**:
  - The request succeeds via standard HTTP GET.
  - No error is returned to the user or agent.
  - A structured info/debug log is emitted noting that standard HTTP fallback was used.

### Scenario 3: Safety Timeout Recovery
- **Execution**:
  1. Invoke `web_fetch` targeting a simulated slow URL that delays responses or stalls script execution beyond 15 seconds.
- **Expected Outcome**:
  - After 15 seconds, the browser process is killed.
  - The fetch automatically falls back to standard HTTP GET.
  - A warning log with code `TOOL-W001` is emitted.

### Scenario 4: Ephemeral Isolation Verification
- **Execution**:
  1. Log into a website in regular Google Chrome (e.g. creating personal session cookies).
  2. Fetch the same website using `web_fetch`.
- **Expected Outcome**:
  - `web_fetch` sees only the unauthenticated public page.
  - The user's personal browser profile, history, and cookies remain completely untouched.
