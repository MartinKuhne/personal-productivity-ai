---
description: "Task list for About Dialog feature"
---

# Tasks: About Dialog

**Input**: Design documents from `/specs/002-about-dialog/`
**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/about-dialog.md, quickstart.md

**Tests**: Included per SC-007 (nextest + egui_kittest). Write tests first and ensure they FAIL before implementation where noted.

**Organization**: Tasks grouped by user story to enable independent implementation and testing. Foundational tasks already partially implemented on disk — marked as Verify where no code change is expected.

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Verify build and repository prerequisites that About Dialog depends on

- [X] T001 Verify compile-time build metadata in `build.rs` emits `BUILD_BRANCH`, `BUILD_COMMIT_HASH`, `BUILD_COMMIT_SHORT_HASH`, `BUILD_DATE` with `GIT_BRANCH`/`GIT_COMMIT` and `SOURCE_DATE_EPOCH` fallbacks and `cargo:rerun-if-changed` for `.git/HEAD` and `.git/index`
- [X] T002 Verify MIT license source exists at `LICENSE` at repository root and is readable by `include_str!("../../../LICENSE")` from `src/app/ui/about_dialog.rs`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Bus, dialog state, and strings that all user stories depend on — verify existing implementations before building new modules

**⚠️ CRITICAL**: No user story work can begin until this phase is confirmed

- [X] T003 Verify `UserCommand::OpenAboutDialog` variant exists in `src/app/bus/events/user_command.rs` with doc comment
- [X] T004 Verify `CommandExecutor::apply_user_command` handles `OpenAboutDialog` in `src/app/command_executor.rs` by setting `self.dialogs.about_dialog_open = true`
- [X] T005 Verify `Dialogs.about_dialog_open: bool` field and `Dialogs::new()` initialization to `false` in `src/app/ui/dialogs.rs` and test `test_new_dialogs_is_empty` covers it
- [X] T006 Verify all About string constants (`MENU_ABOUT`, `ABOUT_DIALOG_TITLE`, `ABOUT_APP_NAME`, `ABOUT_COPYRIGHT`, `ABOUT_BRANCH_LABEL`, `ABOUT_COMMIT_LABEL`, `ABOUT_DATE_LABEL`, `ABOUT_LICENSE_HEADER`, `ABOUT_ATTRIBUTIONS_HEADER`, `ABOUT_COPY_COMMIT_TOOLTIP`, `ABOUT_COPIED_NOTIFICATION`, `ABOUT_UNKNOWN_AUTHOR`, `ABOUT_COL_CRATE`, `ABOUT_COL_AUTHORS`, `ABOUT_COL_REPO`, `ABOUT_EVENT`) exist with `///` doc comments in `src/app/ui/strings.rs`
- [X] T007 Verify hamburger helper `apply_about_button_click() -> UserCommand` and menu wiring `ui.button(MENU_ABOUT) -> publish(OpenAboutDialog) + on_click(ABOUT_EVENT) + ui.close()` exist in `src/app/ui/panels/top.rs`

**Checkpoint**: Foundation ready — `cargo check --quiet` passes, existing `strings`/`dialogs`/`top_tests::test_about_button_click_opens_dialog` pass

---

## Phase 3: User Story 1 - Open About Dialog from Hamburger Menu (Priority: P1) 🎯 MVP

**Goal**: User can open the About dialog from the hamburger menu (☰) and close it via title-bar X

**Independent Test**: Open hamburger menu → verify `About FastMD...` visible → click it → dialog titled `About FastMD` appears and hamburger closes → click title-bar X → dialog closes and does not reappear until menu clicked again. Covered by `src/app/ui/panels/top_tests.rs::test_about_button_click_opens_dialog` and new dialog smoke test.

### Tests for User Story 1

- [X] T008 [P] [US1] Verify Tier-4 harness test `test_about_button_click_opens_dialog` in `src/app/ui/panels/top_tests.rs` asserts `captured.contains(ABOUT_EVENT)` and `assert_bus_contains(OpenAboutDialog)` — ensure test passes with `cargo nextest run -p fastmd`

### Implementation for User Story 1

- [X] T009 [US1] Declare new UI modules `pub mod about_dialog;` and `pub mod attributions;` in `src/app/ui/mod.rs` (facade-only per RUST-054)
- [X] T010 [US1] Implement overlay wiring in `src/app/ui/app/render.rs` to call `crate::ui::about_dialog::show_about_dialog(ctx, self)` when `self.orchestrator.dialogs.about_dialog_open` is true (mirror `batch_dialog`/`tools_dialog` pattern)

**Checkpoint**: US1 independently testable — `cargo nextest run -p fastmd` US1 tests pass; manual: `cargo run` → ☰ → About → dialog appears → X closes

---

## Phase 4: User Story 2 - View Application Identity and Build Metadata (Priority: P1)

**Goal**: About dialog header shows app name, copyright, branch, short commit hash (with full-hash hover tooltip and click-to-copy), and build date

**Independent Test**: Open About dialog → verify header `FastMD Viewer` and `Copyright (c) 2026 Martin Kuhne` → verify `Branch:` + `BUILD_BRANCH`, `Commit:` + 7–8 char hash, `Built:` + `YYYY-MM-DD` → hover commit hash shows `Full commit: <40 chars>` tooltip → click commit hash copies full 40-char hash to clipboard

### Tests for User Story 2

- [X] T011 [P] [US2] Create kittest smoke test `dialog_renders_without_panic` in `src/app/ui/about_dialog_tests.rs` that opens dialog and renders header without panic
- [X] T012 [P] [US2] Create content test `header_shows_app_name_copyright_and_labels` in `src/app/ui/about_dialog_tests.rs` asserting `ABOUT_APP_NAME`, `ABOUT_COPYRIGHT`, `ABOUT_BRANCH_LABEL`, `ABOUT_COMMIT_LABEL`, `ABOUT_DATE_LABEL` present via `env!("BUILD_*")` values
- [X] T013 [P] [US2] Create interaction test `commit_hover_shows_full_hash_and_click_copies` in `src/app/ui/about_dialog_tests.rs` that hovers short hash label and asserts tooltip contains `BUILD_COMMIT_HASH`, then clicks and asserts `ctx` clipboard equals `BUILD_COMMIT_HASH`

### Implementation for User Story 2

- [X] T014 [US2] Create `src/app/ui/about_dialog.rs` with `//!` module doc, `const LICENSE_TEXT: &str = include_str!("../../../LICENSE")`, `const BUILD_BRANCH/COMMIT_HASH/COMMIT_SHORT_HASH/DATE: &str = env!("BUILD_*")`, and `pub fn show_about_dialog(ctx: &egui::Context, app: &mut FastMdApp)` using `egui::Window::new(ABOUT_DIALOG_TITLE).id("about_dialog").open(&mut open).resizable(true).default_size([620.0, 580.0]).min_size([480.0, 400.0])`
- [X] T015 [US2] Implement header layout in `src/app/ui/about_dialog.rs` — bold heading for `ABOUT_APP_NAME`, copyright line, horizontal metadata row with `ABOUT_BRANCH_LABEL` + `BUILD_BRANCH`, `ABOUT_COMMIT_LABEL` + clickable `Label` for `BUILD_COMMIT_SHORT_HASH` with `.on_hover_text(format!("Full commit: {}\n{}", BUILD_COMMIT_HASH, ABOUT_COPY_COMMIT_TOOLTIP))` and `on click -> ctx.copy_text(BUILD_COMMIT_HASH.to_owned())`, `ABOUT_DATE_LABEL` + `BUILD_DATE`
- [X] T016 [US2] Implement commit copy edge case handling in `src/app/ui/about_dialog.rs` — fallback display `ABOUT_UNKNOWN_AUTHOR`/`"unknown"` when `BUILD_COMMIT_HASH == "unknown"`, repeated clicks remain idempotent, no panic in headless clipboard

**Checkpoint**: US2 independently testable — all about_dialog header tests pass; manual hover and click-to-copy verified

---

## Phase 5: User Story 3 - Read Full Application License (Priority: P2)

**Goal**: About dialog License section shows complete MIT license text in a vertically scrollable capped region

**Independent Test**: Open About dialog → locate `License` heading → verify full `LICENSE` text inside `ScrollArea::vertical().id_salt("about_license_scroll").max_height(140.0)` → scroll within area reveals entire text without resizing dialog or scrolling main window

### Tests for User Story 3

- [X] T017 [P] [US3] Create test `license_scroll_contains_mit_text` in `src/app/ui/about_dialog_tests.rs` asserting scroll area contains substrings `"MIT License"`, `"Copyright (c) 2026 Martin Kuhne"`, and `"Permission is hereby granted"`
- [X] T018 [P] [US3] Create test `license_is_scrollable_with_capped_height` in `src/app/ui/about_dialog_tests.rs` verifying `ScrollArea` with `id_salt "about_license_scroll"` and `max_height 140.0` inside a `Frame`

### Implementation for User Story 3

- [X] T019 [US3] Implement License section in `src/app/ui/about_dialog.rs` — `ui.label(ABOUT_LICENSE_HEADER)` heading, `egui::ScrollArea::vertical().id_salt("about_license_scroll").max_height(140.0).show(ui, |ui| Frame::group(ui.style()).show(ui, |ui| ui.label(LICENSE_TEXT)))`

**Checkpoint**: US3 independently testable — license tests pass; manual scroll verified at minimum window size

---

## Phase 6: User Story 4 - Browse Third-Party Attributions (Priority: P2)

**Goal**: Structured scrollable list of all 58 direct third-party dependencies with name (strong), authors, and clickable GitHub URL opening in external browser

**Independent Test**: Open About dialog → scroll to `Third-Party Attributions` → verify 58 rows sorted alphabetically, each with crate name, authors, `https://github.com/...` hyperlink → scroll within attributions area reveals all rows → click any link opens browser → no duplicates or empty fields

### Tests for User Story 4

- [X] T020 [P] [US4] Create test `attribution_all_entries_have_valid_fields` in `src/app/ui/attributions_tests.rs` asserting each of 58 `Attribution` entries has non-empty `name`, `authors`, `github_url` starting with `https://github.com/`
- [X] T021 [P] [US4] Create test `attribution_slice_is_sorted_and_unique` in `src/app/ui/attributions_tests.rs` asserting `DIRECT_DEPENDENCIES` is sorted ascending by `name` and contains no duplicate names
- [X] T022 [P] [US4] Create completeness test `attribution_completeness_against_cargo_manifests` in `src/app/ui/attributions_tests.rs` that parses workspace `Cargo.toml` members (`fastmd`, `fastmd-agent`, `fastmd-pdf`, `fastmd-tool-macros`), collects direct third-party deps minus workspace members, and asserts set equality with `DIRECT_DEPENDENCIES` names
- [X] T023 [P] [US4] Create kittest test `attributions_all_58_rendered_and_scrollable` in `src/app/ui/about_dialog_tests.rs` verifying scroll area `id_salt "about_attributions_scroll" max_height 240.0` renders 58 rows with strong name, authors label, and hyperlink

### Implementation for User Story 4

- [X] T024 [P] [US4] Create `src/app/ui/attributions.rs` with `//!` doc (`//! Attribution catalog ... Unit tests live in the sibling ...`), `pub struct Attribution { pub name: &'static str, pub authors: &'static str, pub github_url: &'static str }` with `///` on each field, and `pub const DIRECT_DEPENDENCIES: &[Attribution] = &[...58 curated entries alphabetically sorted...]` covering all workspace direct deps (anyhow through windows as listed in plan)
- [X] T025 [US4] Implement Attributions section in `src/app/ui/about_dialog.rs` — `ui.label(ABOUT_ATTRIBUTIONS_HEADER)`, `egui::ScrollArea::vertical().id_salt("about_attributions_scroll").max_height(240.0).show(ui, |ui| for attr in DIRECT_DEPENDENCIES { ui.push_id((attr.name, "attribution_row"), |ui| { ui.strong(attr.name); ui.label(attr.authors); ui.hyperlink_to(attr.github_url, attr.github_url); }) })` with fallback `ABOUT_UNKNOWN_AUTHOR` if needed and error-tolerant `open_url` handling
- [X] T026 [US4] Implement close affordance wiring in `src/app/ui/about_dialog.rs` — `Window::open(&mut open)` write-back sets `app.dialogs_mut().about_dialog_open = false` when user clicks title-bar X, and add kittest `close_button_clears_dialog_flag` in `src/app/ui/about_dialog_tests.rs`

**Checkpoint**: US4 independently testable — all attribution tests pass; manual: 58 rows visible, links open, sorted, scrollable

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: Documentation, style, and quality gate

- [X] T027 [P] Add `//!` module docs and `///` on every `pub` item in `src/app/ui/attributions.rs` and `src/app/ui/about_dialog.rs` per RUST-010/RUST-011, and ensure sidecar header pointer line when `#[cfg(test)] mod tests;` is used
- [X] T028 Verify `src/app/ui/mod.rs` remains facade-only (no logic) and `src/app` contains no `eframe::egui` imports outside `ui/` per RUST-058 / `app/` is egui-free
- [X] T029 Run quality gate from repository root: `cargo check --quiet` — no errors/warnings, `cargo nextest run --workspace --status-level fail --show-progress none` — all tests pass, `cargo clippy -- -D warnings` — no lints, `cargo fmt --check` — formatted, `cargo doc --no-deps --quiet` — no warnings
- [X] T030 Run `specs/002-about-dialog/quickstart.md` manual validation end-to-end (hamburger → About → hover → click-to-copy → license scroll → 58 attributions → link open → close → offline fallback with `GIT_BRANCH=unknown GIT_COMMIT=unknown`)

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: No dependencies — can start immediately
- **Foundational (Phase 2)**: Depends on Setup — BLOCKS all user stories (verify bus/dialogs/strings)
- **User Stories (Phase 3+)**: All depend on Foundational completion
  - US1 and US2 both P1 — US1 (overlay wiring) must land before US2/US3/US4 can render dialog content, so US1 precedes others in code integration even though conceptually parallel
  - US3 and US4 are P2 — can proceed in parallel after US1/US2 foundation (different file regions but both in `about_dialog.rs`; coordinate on that file)
- **Polish (Phase 7)**: Depends on all desired user stories being complete

### User Story Dependencies

- **US1 (P1)**: Can start after Foundational — no dependencies on other stories; delivers MVP skeleton (dialog opens/closes)
- **US2 (P1)**: Depends on US1 overlay wiring (`about_dialog.rs` + `render.rs`) — extends dialog with header/build metadata
- **US3 (P2)**: Depends on US1/US2 — adds License scroll area inside same dialog
- **US4 (P2)**: Depends on US1/US2 — adds Attributions catalog + scroll area; depends on `attributions.rs` catalog existing
- **US5 (P1)**: Depends on Foundational + US1 overlay wiring (`about_dialog.rs` + `render.rs`) — sets the same `about_dialog_open` flag at startup; extends `PersistedUiState` (`persisted.rs`) with version stamp

### Within Each User Story

- Tests MUST be written and FAIL before implementation (TDD per constitution SDLC)
- Catalog/struct (`attributions.rs`) before dialog section that iterates it
- Dialog shell (`Window`) before header, before scroll sections
- Core rendering before edge-case handling (offline `unknown`, headless clipboard)

### Parallel Opportunities

- T001 and T002 (Setup) can run in parallel — different checks, no file overlap
- T003–T007 (Foundational verifies) are [P]-eligible but touch different files — can run in parallel
- T011, T012, T013 (US2 tests) marked [P] — different test functions in same file but logically parallel
- T017, T018 (US3 tests) can run in parallel
- T020, T021, T022, T023 (US4 tests) marked [P] — can run in parallel
- T024 (attributions catalog) can be built in parallel with T014 (dialog shell) — different files
- T027 (docs) and T028 (lint checks) marked [P] — different concerns

---

## Parallel Example: User Story 4

```bash
# Launch US4 attribution invariant tests in parallel (same file, independent test functions):
cargo nextest run -p fastmd attributions_tests -- --test-threads 4

# Tasks T020–T023 can be developed in parallel — all touch src/app/ui/attributions_tests.rs
# but different test functions; T024 touches src/app/ui/attributions.rs (different file) so also parallel with test drafting.
```

---

## Parallel Example: User Story 2

```bash
# US2 tests T011–T013 are independent and can be written in parallel:
Task: "Create kittest smoke test dialog_renders_without_panic in src/app/ui/about_dialog_tests.rs"
Task: "Create content test header_shows_app_name_copyright_and_labels in src/app/ui/about_dialog_tests.rs"
Task: "Create interaction test commit_hover_shows_full_hash_and_click_copies in src/app/ui/about_dialog_tests.rs"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (T001–T002)
2. Complete Phase 2: Foundational (T003–T007) — verify bus/dialogs/strings
3. Complete Phase 3: US1 (T008–T010) — minimal `about_dialog.rs` stub + overlay wiring so dialog opens/closes
4. **STOP and VALIDATE**: `cargo nextest run -p fastmd` US1 tests pass; `cargo run` → ☰ → About → X
5. Deploy/demo if ready — MVP skeleton proves routing and window chrome

### Incremental Delivery

1. Setup + Foundational → Foundation ready
2. Add US1 → Test independently → Demo (MVP!)
3. Add US2 (build metadata header + clipboard) → Test independently → Demo — provenance visible
4. Add US3 (license scroll) → Test independently → Demo — legal text complete
5. Add US4 (58 attributions catalog + scroll + links) → Test independently → Demo — full feature per SC-003
6. Polish (docs + quality gate + quickstart) → `cargo nextest --workspace` + manual QA passes, ready for PR

### Parallel Team Strategy

With multiple developers after Foundational:

- Developer A: US1 + US2 (dialog shell + header) — owns `src/app/ui/about_dialog.rs` header region + `src/app/ui/app/render.rs`
- Developer B: US3 (license) — owns License scroll region in same file (coordinate merges)
- Developer C: US4 catalog — owns `src/app/ui/attributions.rs` + `src/app/ui/attributions_tests.rs` in parallel with A/B, then integrates scroll rendering into `about_dialog.rs` after A lands

Stories complete and integrate independently; final integration is a single merge into `about_dialog.rs`.

---

## Notes

- [P] tasks = different files, no dependencies — safe to run in parallel
- [Story] label maps task to spec user story for traceability (US1→FR-001/002/003/014, US2→FR-004/005/006/007, US3→FR-008, US4→FR-009/010/011/012)
- Each user story independently completable and testable per spec Independent Test criteria
- Verify tests fail before implementing (e.g., `attribution_completeness_against_cargo_manifests` should fail with incomplete catalog)
- Commit after each task or logical group; stop at any checkpoint to validate story independently
- `build.rs` and `LICENSE` are at repository root — `include_str!("../../../LICENSE")` is relative to `src/app/ui/about_dialog.rs`
- About dialog Window contract: `egui::Window::new(ABOUT_DIALOG_TITLE).id("about_dialog").open(&mut open).resizable(true).default_size([620.0, 580.0]).min_size([480.0, 400.0])`
- Scroll contracts: `id_salt "about_license_scroll" max_height 140.0`, `id_salt "about_attributions_scroll" max_height 240.0`

---

## Phase 8: Convergence

**Purpose**: Close remaining gaps found by converge assessment of code vs spec/plan/tasks (no rewrites of prior phases)

- [X] T031 Show user-visible confirmation when commit hash is copied per FR-006 (missing)
- [X] T032 Centralize commit hover prefix string into strings.rs with doc comment per FR-015 (partial)
- [X] T033 Strengthen commit hover/copy interaction test to assert tooltip and clipboard per SC-002 (partial)
- [X] T034 Add explicit unknown-fallback display for missing build metadata per FR-007 (partial)
- [X] T035 Review and justify or remove unused attribution column string constants per plan: strings decision (unrequested)

---

## Phase 9: User Story 5 - First-Run Auto-Show (Priority: P1)

**Goal**: About dialog opens automatically on first start (fresh UI state) and on each upgrade's first start, exactly once; same-version restarts stay quiet

**Depends on**: Foundational (Phase 2) + US1 overlay wiring (Phase 3) — auto-show sets the same `about_dialog_open` flag US1 renders; extends `PersistedUiState` with a version stamp

**Independent Test**: Launch with fresh `PersistedUiState` → dialog open on startup → close → restart same version → no auto-show → simulate recorded older version → auto-shows exactly once. Covered by new unit tests below plus `specs/002-about-dialog/quickstart.md` step 12.

### Tests for User Story 5 (write first; verify FAIL before implementation per constitution SDLC)

- [X] T036 [P] [US5] Create decision-matrix test `should_auto_show_about_*` in `src/app/ui/about_dialog_tests.rs` asserting `None` → true, same version → false, older version → true
- [X] T037 [P] [US5] Create round-trip test for `about_shown_for_version` in `src/app/ui/persisted.rs` (`mod tests`) asserting default is `None`, JSON without the field deserializes to `None`, and a recorded version round-trips

### Implementation for User Story 5

- [X] T038 [US5] Add `about_shown_for_version: Option<String>` with `#[serde(default)]` to `PersistedUiState` in `src/app/ui/persisted.rs` and pure helper `should_auto_show_about(recorded: Option<&str>, current: &str) -> bool` in `src/app/ui/about_dialog.rs` with `///` docs (no `CURRENT_SCHEMA_VERSION` bump — compatible shape change)
- [X] T039 [US5] Wire first-run check in `FastMdApp::new` in `src/app/ui/app/init.rs` — after `PersistedUiState` restore + migration, when `should_auto_show_about()` is true set `dialogs.about_dialog_open = true` and record `env!("CARGO_PKG_VERSION")` (persisted by existing `save()` in `src/app/ui/app/mod.rs`; corrupt storage falls back to defaults = fail open)
- [X] T040 [US5] Create startup wiring test in `src/app/ui/app/tests.rs` asserting fresh state auto-opens, recorded current version stays closed, and older recorded version re-opens once and records current
- [X] T041 [US5] Run manual validation end-to-end per `specs/002-about-dialog/quickstart.md` step 12 (fresh state → auto-show → restart quiet → simulated upgrade → exactly once)

**Checkpoint**: US5 independently testable — `cargo nextest run -p fastmd` US5 tests pass; manual quickstart step 12 verified

