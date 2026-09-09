# Feature Specification: About Dialog

**Feature Branch**: `feature/about-dialog`

**Created**: 2026-09-04

**Status**: Draft

**Input**: User description: "Design and implementation plan for adding an About Dialog to FastMD, reachable from the top toolbar's hamburger menu (☰). The dialog displays the application name, copyright notice, compile-time build metadata (git branch, 7–8 char commit hash with full hash on hover/click-to-copy, build date), a scrollable full copy of the application license, and a structured attribution list of all 58 direct crate dependencies across all workspace crates with their authors and GitHub repository URLs."

## Clarifications

### Session 2026-09-04

- Q: How should "the first time" be determined for auto-showing the About dialog? → A: Use the existing application state persistence (PersistedUiState via eframe::Storage); do not use the config file.
- Q: After the first automatic display, when may the About dialog auto-appear again on startup? → A: Each new version — the flag resets when the app version changes, so each upgrade's first start re-shows the dialog.
- Q: What priority should the first-run auto-show behavior (FR-016) have for implementation ordering? → A: P1 (MVP) — required before the feature counts as done.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Open About Dialog from Hamburger Menu (Priority: P1)

As a user, I want to open an About dialog from the top toolbar's hamburger menu (☰) so I can quickly access application identity, version, and legal information without leaving my current context.

**Why this priority**: This is the sole entry point for the feature; without it no other About content is discoverable. It establishes the user-visible affordance in the primary toolbar.

**Independent Test**: Can be fully tested by opening the hamburger menu, verifying an "About FastMD..." entry exists, clicking it, and confirming the About dialog appears with its title visible.

**Acceptance Scenarios**:

1. **Given** the application is running with any workspace open, **When** the user opens the hamburger menu (☰) in the top toolbar, **Then** an "About FastMD..." menu entry is visible near the bottom of the menu.
2. **Given** the hamburger menu is open, **When** the user clicks "About FastMD...", **Then** the About dialog opens as a modal/window titled "About FastMD" and the hamburger menu closes.
3. **Given** the About dialog is open, **When** the user clicks the dialog's close affordance (title-bar X), **Then** the dialog closes and does not reappear until the menu entry is clicked again.

---

### User Story 2 - View Application Identity and Build Metadata (Priority: P1)

As a user or support engineer, I want to see the application name, copyright notice, and build provenance (git branch, commit identifier, build date) in the About dialog so I can confirm exactly which build I am running.

**Why this priority**: Build provenance is essential for bug reports, support triage, and verifying deployments. Identity and copyright are standard for any shipped application.

**Independent Test**: Can be fully tested by opening the About dialog and inspecting that all four metadata fields are visible and correctly formatted, without exercising license or attribution areas.

**Acceptance Scenarios**:

1. **Given** the About dialog is open, **When** the user looks at the header area, **Then** the application name "FastMD Viewer" is displayed prominently and the copyright notice "Copyright (c) 2026 Martin Kuhne" is displayed beneath it.
2. **Given** the About dialog is open, **When** the user locates the build metadata row, **Then** three labeled fields are shown: Branch (git branch name), Commit (7–8 character short hash), and Built (build date in YYYY-MM-DD format).
3. **Given** the Commit field shows a short hash, **When** the user hovers over the short hash, **Then** a tooltip reveals the full 40-character commit hash.
4. **Given** the Commit field shows a short hash, **When** the user clicks the short hash, **Then** the full 40-character commit hash is copied to the system clipboard (with a brief confirmation that the copy succeeded).
5. **Given** the application was built in an environment where git information is unavailable, **When** the About dialog is opened, **Then** Branch and Commit fields show a graceful fallback value (e.g., "unknown") rather than being blank or causing an error.

---

### User Story 3 - Read Full Application License (Priority: P2)

As a user, I want to read the complete application license inside the About dialog so I can understand the terms under which the software is provided.

**Why this priority**: Displaying the full license satisfies legal/attribution obligations and builds user trust. It is independent of build metadata.

**Independent Test**: Can be fully tested by opening the About dialog, locating the License section, and verifying the full MIT license text is present in a vertically scrollable region.

**Acceptance Scenarios**:

1. **Given** the About dialog is open, **When** the user scrolls to the License section, **Then** a section headed "License" is visible and contains the complete application license text.
2. **Given** the license text exceeds the visible area, **When** the user scrolls within the License area, **Then** the full license content can be viewed without resizing the dialog or scrolling the main application.

---

### User Story 4 - Browse Third-Party Attributions (Priority: P2)

As a user, I want to browse a structured list of all direct third-party dependencies used by the application, with each entry showing the crate name, author(s), and a link to its repository, so I can acknowledge open-source contributions and locate upstream projects.

**Why this priority**: Attribution is a licensing and community requirement. The list must be complete, structured, and verifiable independently of other dialog content.

**Independent Test**: Can be fully tested by opening the About dialog, scrolling to the Third-Party Attributions section, and verifying that 58 entries are present, each with a name, author string, and clickable repository link.

**Acceptance Scenarios**:

1. **Given** the About dialog is open, **When** the user scrolls to the Third-Party Attributions section, **Then** a section headed "Third-Party Attributions" is visible and contains 58 entries covering all direct dependencies across all workspace members (fastmd, fastmd-agent, fastmd-pdf, fastmd-tool-macros), with no workspace-internal crates listed and no direct dependency missing.
2. **Given** an attribution entry is displayed, **When** the user reads it, **Then** it shows the crate package name, the author or maintainer organization, and a clickable hyperlink to the project's GitHub repository (URL starting with `https://github.com/`).
3. **Given** the attribution list exceeds the visible area, **When** the user scrolls within the Attributions area, **Then** all entries remain accessible without leaving the dialog.
4. **Given** the user clicks a GitHub repository link, **When** the click is processed, **Then** the system opens the repository URL in the default external browser.
5. **Given** the attribution list is rendered, **When** inspected, **Then** entries are sorted alphabetically by crate name and contain no duplicates or empty fields.

---

### User Story 5 - See About Dialog on First Start (Priority: P1)

As a first-time user, I want the About dialog to appear automatically when the application starts for the first time so I immediately see application identity, version, and legal information.

**Why this priority**: First-run visibility is MVP acceptance; without it the feature's discovery depends solely on the hamburger menu.

**Independent Test**: Can be fully tested by launching the application with fresh UI state persistence and verifying the About dialog is visible on startup, then restarting on the same version and verifying it does not reappear automatically.

**Acceptance Scenarios**:

1. **Given** the application starts with UI state persistence recording no shown version, **When** the main window appears, **Then** the About dialog is open automatically.
2. **Given** the About dialog was displayed and its version recorded, **When** the application restarts on the same version, **Then** the About dialog does not open automatically (it remains available via the hamburger menu).
3. **Given** the recorded version differs from the current app version, **When** the application starts, **Then** the About dialog opens automatically exactly once.

---

### Edge Cases

- What happens when the dialog is opened while another modal/dialog is already open? The About dialog should stack or take focus without corrupting the underlying dialog state, and closing it should restore the previous dialog.
- What happens when the application window is very small (minimum size)? The About dialog should remain fully usable with internal scroll areas for License and Attributions rather than overflowing off-screen; it should enforce a minimum size and be resizable.
- What happens when git metadata is unavailable at build time (offline build, shallow clone, missing .git)? Build fields should display "unknown" with no panic or empty label.
- What happens when the user repeatedly clicks the Commit hash to copy? Each click should reliably copy the full hash to the clipboard and not interfere with text selection or cause errors.
- What happens when a repository URL is malformed or unreachable? The link should still be rendered; clicking it should not crash the application even if the browser fails to open the URL.
- What happens when the license file is missing at build time? The build should fail fast with a clear error, or fall back to a bundled placeholder, rather than showing an empty license section at runtime.
- How does the system handle rapid open/close toggling of the About dialog? State should remain consistent (boolean open/closed) with no leaked window handles or duplicate dialogs.
- What happens when UI state persistence is unavailable or unwritable at startup? The About dialog should still open on that start (fail open) without crashing or blocking startup; recording the shown version is best-effort.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST provide an "About FastMD..." entry in the hamburger menu (☰) of the top toolbar.
- **FR-002**: System MUST open an "About FastMD" dialog when the hamburger menu's "About FastMD..." entry is activated, and MUST close the hamburger menu at that time.
- **FR-003**: System MUST allow the About dialog to be closed via its title-bar close affordance (X) and retain closed state until explicitly reopened.
- **FR-004**: System MUST display the application name "FastMD Viewer" and copyright notice "Copyright (c) 2026 Martin Kuhne" in the About dialog header.
- **FR-005**: System MUST display build metadata in the About dialog: git branch name, short commit hash (7–8 characters), and build date (YYYY-MM-DD), each with a descriptive label (Branch, Commit, Built).
- **FR-006**: System MUST reveal the full 40-character commit hash on hover over the short hash (tooltip) and MUST copy the full hash to the system clipboard when the short hash is clicked, with user-visible confirmation that the copy succeeded.
- **FR-007**: System MUST handle missing build metadata gracefully by displaying "unknown" (or equivalent fallback) for Branch/Commit/Date rather than blank or error state.
- **FR-008**: System MUST display the complete application license text inside the About dialog within a vertically scrollable region headed "License".
- **FR-009**: System MUST display a structured, vertically scrollable attribution list headed "Third-Party Attributions" containing exactly the set of direct third-party dependencies across all workspace crates (58 unique crates), excluding workspace-internal members.
- **FR-010**: Each attribution entry MUST show the crate package name, author/maintainer string, and a clickable hyperlink to the crate's GitHub repository (`https://github.com/...`).
- **FR-011**: System MUST open the GitHub repository URL in the default external browser when an attribution link is clicked.
- **FR-012**: Attribution entries MUST be sorted alphabetically by crate name, contain no duplicates, and have no empty name/author/URL fields.
- **FR-013**: The About dialog MUST be resizable, have a sensible default size large enough to show header plus portions of both scroll areas, and enforce a minimum size that keeps all sections usable; License and Attributions areas MUST each be independently vertically scrollable with capped heights.
- **FR-014**: Opening the About dialog MUST be routed through the application's unified user-command/event bus rather than direct UI-to-state mutation (decoupling requirement).
- **FR-015**: All user-facing strings for the About feature (menu label, dialog title, field labels, section headers, tooltip text, confirmation message) MUST be centralized as named constants with documentation, not inlined as literals in rendering code.
- **FR-016**: System MUST display the About dialog automatically when the application starts for the first time, where "first time" is determined by a first-run flag stored in the existing application UI state persistence (restored across restarts), NOT in the config file. The flag records the app version the dialog was last shown for (empty when never shown); the dialog auto-shows when no version is recorded or the recorded version differs from the current app version, and the current version is recorded once the dialog has been displayed — so each upgrade's first start re-shows the dialog exactly once.

### Key Entities

- **About Dialog State**: Boolean open/closed state controlling visibility of the About window; lives with other dialog states and defaults to closed.
- **Build Metadata**: Immutable compile-time provenance tuple — Branch (string), Full Commit Hash (40-char string), Short Commit Hash (7–8 char string), Build Date (YYYY-MM-DD string) — with fallback value "unknown" when source information is unavailable.
- **License Text**: Complete verbatim text of the application license (MIT), bundled at build time for offline viewing.
- **Attribution**: Structured record for a direct third-party dependency containing Name (crate package name), Authors (author/organization display string), and GitHub URL (repository URL starting with `https://github.com/`). The catalog contains 58 unique Attribution records spanning all workspace members.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of users can open the About dialog within 2 clicks from the main window (hamburger menu → About entry) and can close it with a single click on the title-bar affordance.
- **SC-002**: 100% of About dialog openings display all four header items (app name, copyright, branch, commit, date) without error, and hovering/clicking the commit hash reveals/copies the full 40-character hash on the first attempt for 100% of attempts.
- **SC-003**: 100% of the application's direct third-party dependencies (58 crates) appear in the Attributions list with correct name, non-empty author, and valid GitHub URL; automated completeness check passes by comparing workspace Cargo manifests against the rendered list.
- **SC-004**: All 58 attribution repository links open the correct GitHub URL in the external browser when clicked (manual spot-check of at least 10 diverse entries succeeds).
- **SC-005**: The License section displays the full license text in a scrollable region and users can scroll through the entire text without resizing the dialog or using the main window scroll.
- **SC-006**: Attribution and License scroll areas remain fully navigable when the application window is at its minimum size; no content is clipped without a scroll affordance.
- **SC-007**: Existing automated test suite passes with zero regressions; new tests covering menu entry, dialog open/close, build metadata presence, clipboard copy, and attribution completeness all pass.
- **SC-008**: 100% of starts with fresh UI state (no recorded shown version) display the About dialog automatically, and 0% of same-version restarts re-display it automatically; each upgrade's first start re-displays it exactly once.

## Assumptions

- The hamburger menu (☰) in the top toolbar already exists and can host an additional menu entry with a separator; no new toolbar placement is needed.
- Build metadata (branch, commit hashes, date) is captured at compile time; runtime generation is out of scope.
- When git is unavailable at build time, fallback values ("unknown") are acceptable and do not require a build failure.
- The application license is the MIT license stored at the repository root (`LICENSE` file) and its full verbatim text is appropriate to embed and display.
- Attribution scope is limited to direct third-party dependencies across all workspace crates (fastmd, fastmd-agent, fastmd-pdf, fastmd-tool-macros), totaling 58 unique crates; transitive dependencies and workspace-internal crates are out of scope.
- Author strings and GitHub repository URLs for each attribution are maintained as curated static data derived from crate registries, not fetched at runtime.
- Clipboard access is available via the windowing toolkit; failure to copy (e.g., headless environment) should not crash the dialog.
- Opening external browser links uses the platform's default browser mechanism; offline or restricted environments may not open links but should not produce errors.
- The feature does not require localization/internationalization for v1; all strings are English.
