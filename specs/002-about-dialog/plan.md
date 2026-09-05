# Implementation Plan: About Dialog

**Branch**: `002-about-dialog` | **Date**: 2026-09-04 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/002-about-dialog/spec.md`

## Summary

Add an About Dialog reachable from the top toolbar hamburger menu (☰) that displays application identity ("FastMD Viewer", "Copyright (c) 2026 Martin Kuhne"), compile-time build metadata (git branch, 7–8 char short commit hash with full 40-char hash on hover tooltip and click-to-copy to clipboard, build date YYYY-MM-DD), the full MIT LICENSE text in a capped scroll area, and a structured attribution list of all 58 direct third-party crate dependencies across the four workspace members with crate name, authors, and GitHub URL. The dialog also opens automatically on first start: a version-stamped flag in the existing `PersistedUiState` (via `eframe::Storage`, never the config file) records the app version the dialog was last shown for, so fresh installs and each upgrade's first start auto-open it exactly once. Approach: capture git metadata at compile time via `build.rs` (`cargo:rustc-env`), route dialog opening through the unified `Bus<UserCommand>` (`OpenAboutDialog`) per FMD-068, centralize UI literals in `strings.rs`, store open/closed state in `Dialogs.about_dialog_open`, render with `egui::Window` + two `ScrollArea`s, expose attributions as a curated static `Attribution` catalog with completeness tests, and evaluate a pure `should_auto_show_about(recorded, current)` helper at startup in `FastMdApp::new` after `PersistedUiState` restore.

## Technical Context

**Language/Version**: Rust 2024 edition, nightly/stable via `rust-toolchain` (MSRV aligned with `eframe` 0.36 / `egui` 0.36). Build script uses `winresource` conditionally on Windows.

**Primary Dependencies**: `eframe` 0.36 / `egui` 0.36 + `egui_extras`, `egui_phosphor` 0.13 (icons), `arboard` 3.6 (clipboard), `chrono` 0.4, `tokio` 1.53, `pulldown-cmark` 0.13, `tracing`/`tracing-subscriber`. Attribution data is static; no new runtime crates.

**Storage**: `PersistedUiState` (`src/app/ui/persisted.rs`, via `eframe::Storage` under `PERSISTED_UI_STATE_KEY`) gains a version-stamped first-run flag `about_shown_for_version: Option<String>` (`#[serde(default)]`, so pre-existing state deserializes to `None` and triggers exactly one auto-show after upgrade). The flag records `env!("CARGO_PKG_VERSION")` of the `fastmd` crate once the dialog has been displayed; `FastMdApp::new` (`src/app/ui/app/init.rs`, after restore + schema migration) evaluates the pure helper `should_auto_show_about(recorded, current)` and sets `dialogs.about_dialog_open = true` when no version is recorded or it differs. No config-file (`ConfigStorageHandler`) involvement per spec clarification. Build metadata remains compile-time `env!`; license via `include_str!`; attributions are `&'static` constants.

**Testing**: `cargo nextest run --workspace` (Tier-4 `egui_kittest` snapshot/click tests + unit tests). Harness uses `egui_kittest` for window rendering, `tempfile::TempDir` for isolated filesystem, `NoopConfigStorage`/`InMemoryConfigStorage` for config isolation per AGENTS.md.

**Target Platform**: Desktop application (Windows primary, Linux secondary), `eframe` + `wgpu`/`wayland`+`x11`; single binary `fastmd` crate (`src/app`).

**Project Type**: desktop-app

**Performance Goals**: Dialog open/close within 1 frame (no background work); attribution list render <16ms at 60fps; no allocations on hot path; startup overhead of build metadata is zero (env reads).

**Constraints**: UI MUST remain responsive per FMD-060/FMD-061; no blocking I/O on UI thread; attribution catalog is static (no network fetch at runtime); build must succeed offline with graceful fallbacks ("unknown"); license text must be embedded at compile time.

**Scale/Scope**: One modal dialog, two scroll areas, 58 attributions, 13 new string constants, 2 new modules (`about_dialog`, `attributions`), 1 enum variant, 1 dialog flag, 1 persisted `Option<String>` flag + 1 pure startup helper; no database or network.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Evidence / Notes |
|-----------|--------|------------------|
| I. Testability | PASS | Modular pure helpers (`apply_about_button_click`, `Attribution` catalog invariants, `should_auto_show_about(recorded, current)`, `commit_tooltip_text`) + unit tests for strings/dialogs state + first-run matrix (fresh/ same-version/ upgraded/ unwritable-storage) + attribution completeness + `egui_kittest` smoke/click tests for dialog and hamburger menu; no side effects in helpers |
| II. Security | PASS | No user input parsed; attribution URLs are static https strings validated by test (`starts_with https://github.com/`); clipboard write uses toolkit API; `open_url`/`webbrowser` guarded by not crashing on failure; no secrets or PII logged |
| III. Modularity | PASS | Two new bounded modules (`ui::attributions`, `ui::about_dialog`) each single concern; `Dialogs` and `UserCommand` are existing seams; no mass refactor — small additive change only |
| IV. Open Source Leverage | PASS | Uses existing crates only (`arboard`, `egui`, `chrono`); attribution catalog acknowledges 58 OSS dependencies rather than reimplementing |
| V. SDLC Best Practices | PASS | `cargo check`, `cargo clippy -- -D warnings`, `cargo fmt --check`, `cargo doc`, `cargo nextest` required; tests must be added for new branches; warnings fixed before merge; requirements traceable to spec FRs |

**Gate result**: PASS — no violations requiring justification.

## Project Structure

### Documentation (this feature)

```text
specs/002-about-dialog/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output (/speckit-plan command)
├── data-model.md        # Phase 1 output (/speckit-plan command)
├── quickstart.md        # Phase 1 output (/speckit-plan command)
├── contracts/           # Phase 1 output (/speckit-plan command)
│   └── about-dialog.md  # UI contract for About dialog
└── tasks.md             # Phase 2 output (/speckit-tasks command - NOT created by /speckit-plan)
```

### Source Code (repository root)

```text
build.rs                                    # [MODIFY] build metadata (branch, hashes, date) — already implemented
src/app/
├── bus/events/user_command.rs              # [MODIFY] add OpenAboutDialog variant — already implemented
├── command_executor.rs                     # [MODIFY] handle OpenAboutDialog -> dialogs.about_dialog_open — already implemented
├── ui/
│   ├── dialogs.rs                          # [MODIFY] about_dialog_open: bool — already implemented
│   ├── strings.rs                          # [MODIFY] ~13 ABOUT_* constants — already implemented
│   ├── persisted.rs                        # [MODIFY] add about_shown_for_version: Option<String> with #[serde(default)]
│   ├── app/init.rs                         # [MODIFY] FastMdApp::new: after PersistedUiState restore, auto-open About when should_auto_show_about()
│   ├── app/mod.rs                          # [MODIFY] save() persists the recorded version (no new logic — flag rides along)
│   ├── panels/top.rs                       # [MODIFY] hamburger menu About button + helper — verify remaining wiring
│   ├── panels/top_tests.rs                 # [MODIFY] Tier-4 click test for About
│   ├── attributions.rs                     # [NEW] Attribution struct + DIRECT_DEPENDENCIES[58]
│   ├── attributions_tests.rs               # [NEW] invariants + completeness test
│   ├── about_dialog.rs                     # [NEW] show_about_dialog() with include_str LICENSE + env! metadata
│   ├── about_dialog_tests.rs               # [NEW] smoke + content + clipboard + close tests
│   ├── render/                             # [MODIFY] overlay call when dialogs.about_dialog_open
│   └── mod.rs                              # [MODIFY] pub mod about_dialog; pub mod attributions;
src/agent/                                  # no changes (dependencies enumerated only)
src/md2pdf/                                 # no changes
src/fastmd-tool-macros/                     # no changes
```

**Structure Decision**: Single-project layout (Option 1) — this is a UI feature in the `fastmd` crate (`src/app` bounded subsystem). No new crates; new modules colocate by concern under `ui/` per RUST-050/051. `lib.rs` remains facade-only (RUST-054).

## Complexity Tracking

> **Fill ONLY if Constitution Check has violations that must be justified**

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| — | — | — |

No violations; no tracking required.

---

## Phase 0 — Research (summary; see research.md)

- Build metadata pattern: `build.rs` → `cargo:rustc-env` + `env!` read, with `GIT_BRANCH`/`GIT_COMMIT` env overrides and `cargo:rerun-if-changed` on `.git/HEAD` and `.git/index`.
- Attribution completeness: parsing workspace `Cargo.toml` members to compare direct deps against `DIRECT_DEPENDENCIES`.
- Clipboard hover+copy: `egui::Label::on_hover_text` for tooltip + `.clicked()` / `ctx.copy_text()` for full hash copy.
- Scroll areas: `ScrollArea::vertical().id_salt(...).max_height(140/240)` inside styled `Frame`.
- Dialog window: `egui::Window::new(...).id(...).open(&mut open).resizable(true).default_size([620,580]).min_size([480,400])`.
- First-run auto-show: version-stamped flag in `PersistedUiState` (never the config file); pure `should_auto_show_about(recorded, current)` predicate evaluated in `FastMdApp::new` after restore; `#[serde(default)]` makes pre-existing state (`None`) trigger exactly one post-upgrade auto-show with no schema bump.

## Phase 1 — Design (summary; see data-model.md, contracts/, quickstart.md)

- Entities: `Dialogs.about_dialog_open`, `BuildMetadata` tuple, `LicenseText`, `Attribution`, plus `PersistedUiState.about_shown_for_version` (first-run version stamp).
- Contract: `contracts/about-dialog.md` documents hamburger menu entry, dialog sections, copy behavior, attribution rows, first-run auto-show (§10), and overlay wiring.
- Quickstart: run app, open hamburger menu, click About, verify metadata/hover/copy/scroll/links/close.
