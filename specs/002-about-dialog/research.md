# Research: About Dialog

**Feature**: 002-about-dialog | **Date**: 2026-09-04

## Unknowns Resolved

All Technical Context unknowns were known or resolved during spec drafting; no `NEEDS CLARIFICATION` items remained. Research below validates the choices embedded in the implementation plan.

### 1. Build Metadata at Compile Time (branch, full hash, short hash, build date)

**Decision**: Use root `build.rs` to emit `cargo:rustc-env=BUILD_BRANCH`, `BUILD_COMMIT_HASH`, `BUILD_COMMIT_SHORT_HASH`, `BUILD_DATE`; read at runtime via `env!("BUILD_BRANCH")` etc.; embed license via `include_str!("../../../LICENSE")`.

**Rationale**: Pattern already proven in this repo — current `build.rs` (verified on disk) implements exactly this: `get_git_output(["rev-parse", "--abbrev-ref", "HEAD"])` with `GIT_BRANCH` env fallback, `GIT_COMMIT` fallback for `rev-parse HEAD`, slice to 8 chars for short hash, `SOURCE_DATE_EPOCH` → `YYYY-MM-DD` via Howard Hinnant algorithm, and `cargo:rerun-if-changed=.git/HEAD` / `.git/index`. Zero runtime cost, offline-safe fallback to `"unknown"`, deterministic and replayable. Alternatives considered: runtime `git` invocation (violates offline builds), `vergen` crate (adds dependency for trivial logic), `git2` crate (heavyweight for build script).

**Alternatives considered**: `vergen` / `git2` crates — rejected as unnecessary extra dependencies when `Command::new("git")` with env fallbacks suffices. Runtime git — rejected (requires `.git` at runtime, not reproducible builds).

### 2. Unified Command Routing for Dialog Open

**Decision**: Add `UserCommand::OpenAboutDialog` variant in `src/app/bus/events/user_command.rs` and handle in `CommandExecutor::apply_user_command` by setting `self.dialogs.about_dialog_open = true`; hamburger menu publishes via `UserCommandProducer::publish(apply_about_button_click())` and fires `on_click(ABOUT_EVENT)` for harness capture.

**Rationale**: Follows FMD-068 and AGENTS.md event-driven architecture; keeps UI panels free of `&mut AppOrchestrator` borrows (SC-002). Already implemented in repo (verified: `user_command.rs:68`, `command_executor.rs:72`, `panels/top.rs:apply_about_button_click`). No new bus plumbing needed.

**Alternatives considered**: Direct `app.dialogs.about_dialog_open = true` in `top.rs` — rejected (violates FMD-069 and test isolation). Separate bus type — rejected (unifies all user intent on `Bus<UserCommand>`).

### 3. UI Strings Centralization

**Decision**: Define all About literals in `src/app/ui/strings.rs` as `pub const &str` with `///` doc comments: `MENU_ABOUT`, `ABOUT_DIALOG_TITLE`, `ABOUT_APP_NAME`, `ABOUT_COPYRIGHT`, `ABOUT_BRANCH_LABEL`, `ABOUT_COMMIT_LABEL`, `ABOUT_DATE_LABEL`, `ABOUT_LICENSE_HEADER`, `ABOUT_ATTRIBUTIONS_HEADER`, `ABOUT_COPY_COMMIT_TOOLTIP`, `ABOUT_COPIED_NOTIFICATION`, `ABOUT_UNKNOWN_AUTHOR`, `ABOUT_COL_CRATE`, `ABOUT_COL_AUTHORS`, `ABOUT_COL_REPO`, `ABOUT_EVENT`.

**Rationale**: Enforced by `src/app/AGENTS.md §4` and RUST-021; enables doc lint and centralized i18n later. Verified that `strings.rs` already contains all `ABOUT_*` constants (lines 25–72).

**Alternatives considered**: Inline literals — rejected per lint gate.

### 4. Attribution Catalog Scope and Shape

**Decision**: `pub struct Attribution { name: &'static str, authors: &'static str, github_url: &'static str }` + `pub const DIRECT_DEPENDENCIES: &[Attribution]` with 58 curated entries sorted alphabetically, sourced from workspace `Cargo.toml` direct dependencies excluding workspace members (`fastmd`, `fastmd-agent`, `fastmd-pdf`, `fastmd-tool-macros`).

**Rationale**: Curated static list avoids runtime network / crate-registry parsing, keeps binary self-contained, and satisfies license attribution. Sorted alphabetical order with duplicate detection gives deterministic diff review. Completeness verified by an automated test parsing `Cargo.toml` members.

**Alternatives considered**: Generate at build time via `cargo_metadata` / `cargo-license` tooling — rejected: adds build dependency, non-deterministic output across registries, and still requires author/URL curation. Runtime fetch — rejected (offline use).

### 5. About Dialog Window and Interaction

**Decision**: `egui::Window::new(ABOUT_DIALOG_TITLE).id("about_dialog").open(&mut open).resizable(true).default_size([620.0, 580.0]).min_size([480.0, 400.0])`; header with `RichText::strong` for app name; metadata row with `Label` for short hash carrying `.on_hover_text(format!("Full commit: {BUILD_COMMIT_HASH}\n{ABOUT_COPY_COMMIT_TOOLTIP}"))` and click→`ctx.copy_text(BUILD_COMMIT_HASH.to_owned())`; License via `ScrollArea::vertical().id_salt("about_license_scroll").max_height(140.0)` inside `Frame`; Attributions via `ScrollArea::vertical().id_salt("about_attributions_scroll").max_height(240.0)` with `ui.hyperlink_to(authors, github_url)` rows; close via Window's `X` which toggles `open` and writes back to `app.dialogs_mut().about_dialog_open`.

**Rationale**: Standard `egui` patterns; `Window::open(&mut bool)` already used for `batch_dialog`/`tools_dialog` in this codebase; `ScrollArea` with `id_salt` gives stable IDs; `hyperlink_to` opens via `webbrowser`/`opener`; clipboard via `ctx.copy_text` mirrors existing `CopyPath` flow but uses egui clipboard (no `arboard` fallback needed inside egui context). Respects `egui::Id` stability rules (AGENTS.md §5) and conditional rendering rules (§6 — always allocate overlay, toggle visibility via `if dialogs.about_dialog_open`).

**Alternatives considered**: Modal `egui::Modal` — not in this `egui` version; `Window` is correct. Using `arboard::Clipboard` directly — unnecessary inside egui; `Context::copy_text` is toolkit-canonical.

### 6. Attribution Completeness Test Strategy

**Decision**: Unit test in `attributions_tests.rs` parses workspace `Cargo.toml` and member manifests to collect direct dependency names, filters workspace members, and asserts that `DIRECT_DEPENDENCIES` sorted names equals expected set (and covers no-transitive, no-duplicate, https-prefix invariants).

**Rationale**: Guards SC-003; fails on drift when a new direct dependency is added to any manifest. Sorted assertion gives minimal diff on failure.

**Alternatives considered**: Manual review only — rejected (human misses deps). Snapshot file — rejected (extra artifact).

### 7. Overlay Wiring (render.rs)

**Decision**: In `render_overlays` (or `ui::app::render` path), call `crate::ui::about_dialog::show_about_dialog(ctx, app)` when `app.orchestrator.dialogs.about_dialog_open` is true; `show_about_dialog` takes `&mut FastMdApp` or `&mut Dialogs` to write close state.

**Rationale**: Mirrors existing `tools_dialog`/`batch_dialog` overlay wiring; keeps `FastMdApp::update` side-effect structure consistent (AGENTS.md §3).

### 8. First-Run Auto-Show Persistence (FR-016 / US5)

**Decision**: Record the app version the About dialog was last shown for in a new `about_shown_for_version: Option<String>` field on `PersistedUiState` (`src/app/ui/persisted.rs`, persisted via `eframe::Storage` under `PERSISTED_UI_STATE_KEY`), evaluated at startup in `FastMdApp::new` (`src/app/ui/app/init.rs`) by a pure helper `should_auto_show_about(recorded: Option<&str>, current: &str) -> bool` against `env!("CARGO_PKG_VERSION")`. `None` (fresh state or pre-existing state written before the field existed, via `#[serde(default)]`) or a version mismatch auto-opens the dialog exactly once and records the current version; a match does nothing. No `CURRENT_SCHEMA_VERSION` bump: the shape change is backwards-compatible.

**Rationale**: Spec clarification (Session 2026-09-04) mandates the existing UI state persistence and forbids the config file — `ConfigStorageHandler` owns user configuration, while first-run-seen is UI session state alongside window sizes and expanded dirs. `Option<String>` (not `bool`) implements the per-version re-show policy with a single field. `#[serde(default)]` gives the upgrade path for free: users updating from a build without the field get one auto-show, then silence. The pure predicate keeps the decision unit-testable without constructing `FastMdApp` (fresh / same-version / upgraded / corrupt-JSON-falls-back-to-default matrix). Fail-open when storage is unavailable follows from the same logic (unreadable state ≡ `None` ≡ show).

**Alternatives considered**: Flag in `AppConfig` via `ConfigStorageHandler` — rejected by explicit spec clarification (config file must not carry it; also pollutes user-editable config with machine state). `bool` seen-flag — rejected (cannot express per-version re-show without a second field). `CURRENT_SCHEMA_VERSION` bump + migration — unnecessary (new field defaults cleanly; migration path reserved for incompatible changes). Recording on dialog close instead of on display — equivalent observable behavior with more state plumbing; display-time recording matches spec wording ("recorded once the dialog has been displayed").

## Open Questions

None. All unknowns resolved; no `NEEDS CLARIFICATION` remains.

## References

- Current `build.rs` on disk (see Technical Context)
- `src/app/ui/strings.rs:25-72` — ABOUT_* constants already present
- `src/app/ui/dialogs.rs:55,90` — about_dialog_open already present
- `src/app/bus/events/user_command.rs:68,102-103` — OpenAboutDialog already present
- `src/app/command_executor.rs:72` — handler already present
- `src/app/ui/panels/top.rs:apply_about_button_click` — helper already present
- `LICENSE` at repo root (MIT)
- `Cargo.toml` workspace members and direct dependencies enumeration (58 entries listed in spec FR-009)
- `src/app/ui/persisted.rs` — `PersistedUiState` shape, `#[serde(default)]` fields, `CURRENT_SCHEMA_VERSION` migration precedent
- `src/app/ui/app/init.rs:186-212` — `FastMdApp::new` restore + migration seam for the first-run check
- `src/app/ui/app/mod.rs:256-273` — `save()` serializes `persisted_ui_state`; the recorded version rides along with no new logic
