# Data Model & Architecture: Chrome DevTools Integration for Web Fetch

**Feature**: Chrome DevTools Integration for Web Fetch  
**Feature Directory**: `specs/003-chrome-devtools-web-fetch/`  
**Status**: Draft  
**Date**: 2026-09-07  

---

## 1. Entities & Data Structures

### 1.1 `BrowserKind` (Enum)

Represents the flavor/branding of the detected Chromium-based executable.

| Variant | Binary Names | Description |
| :--- | :--- | :--- |
| `GoogleChrome` | `chrome.exe`, `google-chrome`, `google-chrome-stable` | Standard Google Chrome installation. |
| `Chromium` | `chromium.exe`, `chromium`, `chromium-browser` | Open-source Chromium binary. |
| `MicrosoftEdge` | `msedge.exe`, `msedge` | Microsoft Edge (Chromium-based). |
| `Brave` | `brave.exe`, `brave` | Brave browser. |
| `Custom` | Arbitrary executable path | User-configured executable via settings. |

### 1.2 `BrowserInstallation` (Struct)

Represents a verified browser binary available on the local operating system.

| Field | Type | Description |
| :--- | :--- | :--- |
| `kind` | `BrowserKind` | The detected browser kind. |
| `executable_path` | `PathBuf` | Absolute path to the validated executable. |
| `version_string` | `Option<String>` | Optional version banner (e.g. `Chrome 126.0.0.0`). |

### 1.3 `BrowserLocator` (Trait)

Abstracts discovery of browser binaries on the host system to allow mocking in test suites.

```rust
pub trait BrowserLocator: Send + Sync {
    /// Detects the highest priority installed Chromium browser, or None if none found.
    fn locate(&self) -> Option<BrowserInstallation>;
}
```

### 1.4 `ChromeFetchConfig` (Struct)

Configurable parameters governing headless browser execution.

| Field | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `custom_path` | `Option<PathBuf>` | `None` | User override path for the browser binary. |
| `timeout` | `Duration` | `15s` | Maximum time allowed for browser launch and DOM render. |
| `virtual_time_budget` | `u32` | `3000ms` | Virtual execution time granted for client-side JavaScript timers. |
| `enabled` | `bool` | `true` | Master toggle to enable or disable browser-backed fetching. |

### 1.5 `FetchProvenance` (Enum)

Records how a specific web fetch response was fulfilled for observability and testing.

| Variant | Description |
| :--- | :--- |
| `Cache` | Served from `ToolCache.web_documents` without network or process activity. |
| `BrowserRender` | Successfully rendered via headless Chrome / Chromium DevTools. |
| `HttpFallback` | Fetched via standard HTTP GET (`reqwest`), either by design or after browser failure. |

### 1.6 `RenderedDocument` (Struct)

Internal intermediary containing the raw HTML output and captured HTTP response headers before conversion to Markdown.

| Field | Type | Description |
| :--- | :--- | :--- |
| `html_body` | `String` | Fully rendered DOM / HTML content. |
| `headers` | `HashMap<String, String>` | HTTP response headers captured from the request. |
| `provenance` | `FetchProvenance` | Origin method of the document. |

---

## 2. State Machine & Execution Flow

```text
               ┌──────────────────────┐
               │ WebFetchInput (url)  │
               └──────────┬───────────┘
                          │
                   [Cursor supplied?]
                   ├── YES ──> Return next page from ToolCache.web_lines
                   │
                   └── NO ───> [force_refetch == false && In Cache?]
                               ├── YES ──> Return cached Markdown (Provenance: Cache)
                               │
                               └── NO ───> [BrowserLocator::locate()]
                                           │
                        ┌──────────────────┴──────────────────┐
                        │ Found                               │ None
                        ▼                                     ▼
            ┌──────────────────────┐              ┌──────────────────────┐
            │  Headless Chrome     │              │  Standard HTTP GET   │
            │  Render Execution    │              │  (reqwest::blocking) │
            └──────────┬───────────┘              └──────────┬───────────┘
                       │                                     │
                 [Render OK?]                                │
                 ├── YES ──> (HTML + Headers)                │
                 │                 │                         │
                 └── NO (error) ───┼─────────────────────────┘
                     (Fallback)    │
                                   ▼
                       ┌──────────────────────┐
                       │  fast_h2m::convert   │
                       └──────────┬───────────┘
                                  │
                                  ▼
                       ┌──────────────────────┐
                       │ Cache in ToolCache   │
                       │ Slice First Page(64) │
                       └──────────┬───────────┘
                                  │
                                  ▼
                       ┌──────────────────────┐
                       │  WebFetchResponse    │
                       └──────────────────────┘
```

---

## 3. Invariants & Business Rules

1. **R-001 (Zero Regression)**: Under no circumstances shall an unavailable, corrupted, or crashing Chrome binary cause `tool_web_fetch` to return an `Err` to the caller if standard HTTP GET would succeed.
2. **R-002 (Privacy & Isolation)**: Every Chrome process MUST be passed `--incognito`, `--no-first-run`, `--no-default-browser-check`, and an isolated `--user-data-dir` so that local cookies and session files are never read or persisted.
3. **R-003 (Bounded Execution Time)**: Any browser render operation exceeding `ChromeFetchConfig.timeout` (default 15 seconds) MUST be killed immediately, freeing resources and triggering fallback.
4. **R-004 (Cache Equivalence)**: Cached documents produced by browser rendering MUST adhere to the exact same cache expiration rules (30-minute TTL) and cursor pagination contract (64 lines per page) as standard HTTP fetched documents.
5. **R-005 (Test Isolation)**: The production `BrowserLocator` and `BrowserRunner` MUST be injectable via `ToolContext` extensions or function parameters, allowing unit tests to run in hermetic environments without real Chrome installations.
