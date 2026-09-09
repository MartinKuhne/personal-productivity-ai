# Feature Specification: Chrome DevTools Integration for Web Fetch

**Feature Branch**: `feature/chrome-devtools-web-fetch`

**Created**: 2026-09-07

**Status**: Draft

**Input**: User description: "use chrome devtools if installed for a better web_fetch"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Client-Side Rendered Web Page Fetching via Headless Chrome (Priority: P1)

As an AI agent or end user researching dynamic web content, I want `web_fetch` to use headless Chrome to execute client-side JavaScript when Chrome is installed on the host machine, so that the retrieved text includes dynamically generated DOM content rather than an empty single-page application (SPA) shell or loading placeholder.

**Why this priority**: Core value of the feature. Static HTTP retrieval frequently fails to extract readable content from modern JavaScript-rendered websites and SPAs.

**Independent Test**: Can be fully tested by fetching a URL known to require client-side JavaScript rendering (such as a single-page documentation app or React dashboard), and confirming the converted Markdown contains the rendered body elements rather than an empty root container or noscript banner.

**Acceptance Scenarios**:

1. **Given** Google Chrome (or a compatible Chromium browser) is installed on the host system, **When** `web_fetch` is invoked on a URL that relies on client-side JavaScript execution, **Then** the page is loaded headlessly with JavaScript enabled and the fully rendered DOM content is extracted and converted into Markdown.
2. **Given** a web page is rendered via headless Chrome, **When** the HTML is converted to Markdown, **Then** the output adheres to existing `web_fetch` line capping (64 lines per page) and returns a valid cursor for subsequent pagination.
3. **Given** `web_fetch` triggers headless Chrome, **When** the browser process runs, **Then** no graphical browser window, taskbar icon, or system prompt appears to the user.

---

### User Story 2 - Seamless Fallback to Standard HTTP Retrieval (Priority: P1)

As a user running FastMD on a system without Chrome installed, or in an environment where browser execution fails or times out, I want `web_fetch` to seamlessly fall back to standard HTTP GET retrieval, so that web fetching operations remain completely functional without throwing errors or breaking agent workflows.

**Why this priority**: Guarantees zero regression and resilient operation across heterogeneous systems, minimal environments, and unexpected browser crashes.

**Independent Test**: Can be fully tested by disabling or removing Chrome from the execution path, calling `web_fetch`, and verifying the fetch succeeds normally via standard HTTP.

**Acceptance Scenarios**:

1. **Given** Chrome is not installed on the system, **When** `web_fetch` is invoked for any URL, **Then** the system automatically retrieves the content via standard HTTP GET without surfacing an error to the user or agent.
2. **Given** Chrome is installed but the browser launch or page navigation times out (exceeding 15 seconds) or crashes, **When** `web_fetch` executes, **Then** the system catches the failure, logs a diagnostic warning, and automatically completes the fetch using standard HTTP GET.

---

### User Story 3 - Execution Isolation and Privacy (Priority: P2)

As a security-conscious user, I want headless Chrome operations for `web_fetch` to run in an ephemeral, isolated profile, so that my personal browsing sessions, active logins, cookies, and search history in my primary browser are never accessed, altered, or leaked.

**Why this priority**: Protects user privacy and security while ensuring idempotent, uncontaminated web fetches.

**Independent Test**: Can be fully tested by verifying that browser invocations use a temporary isolated user-data directory or incognito flag, leaving the default user profile untouched.

**Acceptance Scenarios**:

1. **Given** headless Chrome is invoked for `web_fetch`, **When** the browser process launches, **Then** it uses an isolated temporary profile or incognito session and does not read from or write to the user's primary Chrome profile data.
2. **Given** a browser fetch finishes or is cancelled, **When** the process terminates, **Then** any temporary session artifacts created during the run are cleanly discarded.

---

### User Story 4 - Cache Transparency and Pagination Continuity (Priority: P2)

As a user or agent issuing repeat queries or paginating through large web documents, I want cached responses and cursor pagination to work identically regardless of whether the initial fetch was performed via Chrome or standard HTTP, so that subsequent operations remain fast and consistent.

**Why this priority**: Ensures seamless integration with existing caching policies (TOOL-006, TOOL-008, TOOL-009) without redundant browser launches.

**Independent Test**: Can be tested by fetching a URL, verifying the first page is returned, requesting subsequent pages using the returned cursor, and confirming no new browser process is launched.

**Acceptance Scenarios**:

1. **Given** a URL was previously fetched and rendered into Markdown, **When** `web_fetch` is called with the same URL and `force_refetch = false`, **Then** the cached Markdown is returned immediately without launching Chrome.
2. **Given** a multi-page Markdown document was generated from a Chrome render, **When** a follow-up call passes a valid pagination `cursor`, **Then** the next chunk of lines is served from the line cursor cache without re-fetching.
3. **Given** a cached document exists, **When** `web_fetch` is invoked with `force_refetch = true`, **Then** the cache is invalidated and a fresh fetch is executed using Chrome (or HTTP fallback).

---

### Edge Cases

- **Slow or Stalled Pages**: If a page contains long-polling network requests, endless animations, or unyielding scripts, the headless fetch aborts at 15 seconds and captures the DOM in its current state or falls back to standard HTTP.
- **Non-HTML Responses**: If a URL targets a raw asset (e.g. plain text, CSV, JSON, or image), the system falls back to direct HTTP retrieval or extracts the plain document body without hanging.
- **Missing or Corrupted Chrome Binary**: If a detected Chrome path points to an inoperable or incompatible binary, the failure is logged and the fetch gracefully falls back to standard HTTP.
- **Concurrent Fetches**: If multiple `web_fetch` tool calls execute concurrently, each uses independent browser sessions or connections without socket port collisions or lockfile contention.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST detect whether Google Chrome (or a compatible Chromium browser) is installed on the host system by inspecting standard platform installation paths and system PATH.
- **FR-002**: When a compatible browser is detected, `web_fetch` MUST utilize headless browser execution with Chrome DevTools Protocol (CDP) to load the page and capture the fully rendered DOM after JavaScript execution.
- **FR-003**: When no compatible browser is detected, `web_fetch` MUST automatically retrieve the page using standard HTTP GET without reporting an error.
- **FR-004**: If headless browser rendering fails, encounters an unhandled protocol error, or exceeds a 15-second timeout, `web_fetch` MUST automatically fall back to standard HTTP GET retrieval.
- **FR-005**: All browser-backed fetching operations MUST run headlessly with no visible graphical windows, popups, or dock/taskbar icons.
- **FR-006**: Browser-backed fetching operations MUST run in an isolated ephemeral session (e.g. incognito/temporary user profile) that does not read or mutate the host user's personal browser data or saved credentials.
- **FR-007**: Rendered DOM content MUST be converted into Markdown according to the application's established HTML-to-Markdown pipeline and must honor existing line-length and pagination constraints.
- **FR-008**: When the `headers` option is requested (`headers = true`), the response MUST include the HTTP response headers captured from the main document response.
- **FR-009**: Document caching (30-minute TTL) and cursor-based line pagination MUST function identically whether content was retrieved via headless Chrome or standard HTTP GET.
- **FR-010**: The system MUST emit structured diagnostic logs detailing the fetch method used (browser render vs. HTTP fallback), execution duration, and any fallback triggers.

### Key Entities

- **Web Fetch Input**: The tool invocation arguments including target URL, optional pagination cursor, optional `headers` flag, and optional `force_refetch` flag.
- **Web Fetch Response**: The output payload containing the Markdown content, line count, optional cursor for next page, cache status, and optional response headers.
- **Browser Capability State**: Runtime metadata tracking whether a compatible headless browser is available on the machine, the resolved executable path, and its readiness.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of tested client-rendered single-page applications (SPAs) that yield empty content under static HTTP retrieval yield populated, readable Markdown when a compatible browser is installed.
- **SC-002**: On systems without Chrome or Chromium installed, 100% of `web_fetch` operations continue to succeed via standard HTTP retrieval with zero user-facing regressions or error popups.
- **SC-003**: In the event of a browser freeze, unhandled script loop, or unresponsive server, the fallback mechanism recovers within 15 seconds, returning standard HTTP content or an actionable diagnostic error.
- **SC-004**: Requests served from cache return in under 50 milliseconds without initiating any browser process.
- **SC-005**: Browser-backed fetching introduces zero state contamination to the user's personal browser data (0% cookie or session sharing).

## Assumptions

- Compatible browsers include Google Chrome, Chromium, and Chromium-derived browsers installed in standard OS directories (e.g., Windows `%ProgramFiles%` / `%LocalAppData%`, macOS `/Applications`, Linux `/usr/bin`), or discoverable via the system `PATH`.
- The user's device has sufficient ephemeral resources (typically ~100 MB RAM and an available CPU core) to run headless Chrome during an active fetch.
- `web_fetch` remains a read-only content retrieval tool; user interactions, form fills, clicks, and multi-step workflows remain within the scope of the dedicated browser automation subsystem.
- A 15-second total timeout provides an optimal balance between allowing modern heavy pages to hydrate and keeping the agent's turn response interactive.
