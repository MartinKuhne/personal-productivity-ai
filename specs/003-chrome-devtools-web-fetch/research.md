# Technical Research: Chrome DevTools Integration for Web Fetch

**Feature**: Chrome DevTools Integration for Web Fetch  
**Feature Directory**: `specs/003-chrome-devtools-web-fetch/`  
**Status**: Completed  
**Date**: 2026-09-07  

---

## 1. Overview & Problem Statement

The `web_fetch` tool currently retrieves web pages via standard blocking HTTP GET requests (`reqwest::blocking::Client`) and converts the returned HTML body to Markdown via `fast_h2m::convert`. While fast and lightweight for static documents, this approach fails on modern client-side rendered websites (Single-Page Applications built with React, Vue, Angular, etc.), documentation platforms that require client hydration, and pages behind client-side challenge scripts. In such cases, `web_fetch` yields empty root divs (`<div id="root"></div>`), loading indicators, or "JavaScript is required" warnings.

This feature enables `web_fetch` to detect whether Google Chrome (or a compatible Chromium-based browser) is installed on the host system and, if present, leverage headless Chrome rendering to execute client-side JavaScript, wait for the DOM to populate, and extract the rendered DOM before conversion to Markdown. When Chrome is not installed, or if browser execution encounters an error or timeout, the system seamlessly and transparently falls back to standard HTTP GET retrieval.

---

## 2. Research Decisions & Rationale

### D-001: Browser Discovery & Path Resolution

- **Decision**: Implement a multi-platform `BrowserLocator` that checks for installed Chromium binaries in priority order:
  1. Custom executable path if configured in `AgentConfig` / `AppConfig` (`browser.chrome_path`).
  2. Well-known operating system installation directories:
     - **Windows**:
       - `C:\Program Files\Google\Chrome\Application\chrome.exe`
       - `C:\Program Files (x86)\Google\Chrome\Application\chrome.exe`
       - `%LOCALAPPDATA%\Google\Chrome\Application\chrome.exe`
       - Fallback to Microsoft Edge: `C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe`, `C:\Program Files\Microsoft\Edge\Application\msedge.exe`
     - **macOS**:
       - `/Applications/Google Chrome.app/Contents/MacOS/Google Chrome`
       - `/Applications/Chromium.app/Contents/MacOS/Chromium`
       - `/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge`
     - **Linux**:
       - `/usr/bin/google-chrome`
       - `/usr/bin/google-chrome-stable`
       - `/usr/bin/chromium`
       - `/usr/bin/chromium-browser`
  3. System `PATH` search via `which` / `where.exe` lookup.
- **Rationale**: Prioritizing Google Chrome while accepting Edge/Chromium ensures that Windows and Linux users with default system configurations obtain rich web fetching immediately without manual path configuration.
- **Alternatives Considered**:
  - *Hardcoding `chrome` on PATH*: Fails on Windows where `chrome.exe` is installed under `Program Files` but rarely added to the system `PATH` by default.
  - *Requiring Playwright/Node installation*: Unacceptable burden for non-developer end users; requires multi-hundred megabyte downloads and Node.js runtime.

---

### D-002: Browser Invocation & DevTools Rendering Engine

- **Decision**: Utilize Chromium's headless rendering pipeline with `--headless=new` and `--dump-dom`, combined with an adaptive virtual time budget (`--virtual-time-budget=3000`). For requests requiring HTTP response headers (`headers: true`), combine rendered DOM extraction with HTTP header retrieval via `reqwest`.
- **Rationale**:
  - `--headless=new --dump-dom` executes the full Chromium V8 and Blink rendering engine, evaluates scripts, mutates the DOM, and outputs the final serialized HTML to stdout upon page stabilization.
  - Unlike long-lived background browser daemons, `--dump-dom` is completely self-terminating, avoids opening unauthenticated TCP/WebSocket listening ports on localhost (preventing Windows Defender firewall alerts and port collision risks), and eliminates orphan browser processes.
  - Process execution is completely decoupled from UI rendering threads and can be executed synchronously or via `tokio::process::Command` within the agent tool execution pool.
- **Alternatives Considered**:
  - *CDP WebSocket Daemon (`--remote-debugging-port=0` + `tokio-tungstenite`)*: Adds significant protocol state-machine complexity, requires WebSocket framing dependencies in `fastmd-agent`, and introduces orphan process hazards if the WebSocket connection drops unexpectedly.
  - *Playwright / `playwright-rs`*: Already exists as an optional feature in the repo, but is currently gated behind `feature = "browser"`, only supports Firefox in `fastmd`, and requires downloading Playwright driver binaries. `web_fetch` must work in the core `fastmd-agent` crate without optional features.

---

### D-003: Graceful Fallback & Resilience Pipeline

- **Decision**: Implement a two-tier `WebFetchPipeline`:
  ```text
  [web_fetch request]
          │
     Cache hit? ──YES──> Return cached Markdown (0ms)
          │ NO
  Browser installed?
     ├─ YES ──> Attempt Headless Chrome Render
     │              │
     │          Success? ──YES──> Convert HTML -> Markdown -> Cache & Return
     │              │ NO (crash / timeout / non-zero exit)
     │              ▼
     │          Log Warning (TOOL-W001)
     │              ▼
     └─ NO ───> Fallback HTTP GET (reqwest)
                    │
                Success? ──YES──> Convert HTML -> Markdown -> Cache & Return
                    │ NO
                    ▼
                Return user-facing error
  ```
- **Rationale**: Guarantees zero user-facing regressions on machines without Chrome or when navigating problematic sites (e.g. sites triggering browser sandbox crashes or infinite loops).
- **Alternatives Considered**:
  - *Failing immediately if Chrome is configured but fails*: Degrades user experience; users prefer receiving static text over a hard error.

---

### D-004: Execution Isolation & Security Boundary

- **Decision**: Enforce strict command-line flags on every headless browser invocation:
  ```text
  --headless=new
  --disable-gpu
  --no-first-run
  --no-default-browser-check
  --incognito
  --user-data-dir=<tempfile::TempDir>
  --disable-background-networking
  --disable-sync
  --disable-default-apps
  --disable-extensions
  --mute-audio
  --virtual-time-budget=3000
  --dump-dom
  <URL>
  ```
- **Rationale**:
  - `--incognito` + an isolated ephemeral `--user-data-dir` ensures zero leakage of the user's cookies, passwords, history, or active session state.
  - Disabling extensions, sync, and background networking prevents malicious extension interference and improves process launch latency by ~300ms.
  - Passing `--virtual-time-budget=3000` enables deterministic script evaluation without wall-clock sleep delays.
- **Alternatives Considered**:
  - *Reusing the user's default Chrome profile*: High security risk; could expose authenticated user sessions to untrusted web content or alter user browser history.

---

### D-005: Timeout, Concurrency & Process Lifecycle

- **Decision**: Enforce a strict 15-second total timeout per browser invocation. On timeout, the child process is immediately terminated (`kill()`), temporary directories are cleaned up, and the request falls back to standard HTTP GET.
- **Rationale**: Heavy web pages with open WebSockets or infinite animation loops could otherwise block the agent tool execution loop indefinitely. 15 seconds provides sufficient headroom for heavy client-side SPAs while bounding worst-case latency.
- **Alternatives Considered**:
  - *Indefinite wait*: High risk of freezing agent turns.
  - *5-second timeout*: Too aggressive for complex SPAs on slower hardware or high-latency network connections.

---

### D-006: Testability & Mocking Architecture (RUST-006 Compliance)

- **Decision**: Define a `BrowserRunner` trait:
  ```rust
  pub trait BrowserRunner: Send + Sync {
      fn is_available(&self) -> bool;
      fn render_dom(&self, url: &str, timeout: Duration) -> Result<String, BrowserError>;
  }
  ```
  Implement `SystemChromeRunner` for production, and `MockBrowserRunner` for unit and integration testing.
- **Rationale**:
  - RUST-006 mandates that tests MUST NEVER mutate real user filesystem paths or depend on specific host software installations.
  - With `BrowserRunner`, tests can exercise:
    1. Successful Chrome rendering and Markdown conversion.
    2. Missing Chrome detection triggering HTTP fallback.
    3. Process crash / error triggering HTTP fallback.
    4. Timeout triggering HTTP fallback.
    5. Caching and cursor pagination invariants.
    All without spawning real Chrome processes or touching real user profiles.
