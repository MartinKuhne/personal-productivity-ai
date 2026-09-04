# Data Model: About Dialog

**Feature**: 002-about-dialog | **Date**: 2026-09-04 | **Spec**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md)

## Entities

### 1. Dialogs.about_dialog_open

- **Location**: `src/app/ui/dialogs.rs` — field on `pub struct Dialogs`
- **Type**: `bool`
- **Semantics**: Whether the About dialog window is currently visible. Defaults to `false` in `Dialogs::new()`. Mutated only by `CommandExecutor::apply_user_command(UserCommand::OpenAboutDialog)` (open) and by the dialog's own close affordance (`egui::Window::open(&mut bool)` write-back to `dialogs.about_dialog_open = false`).
- **Validation**: No invariants beyond boolean. Test: `dialogs_new_is_closed` asserts `!about_dialog_open`.
- **Relationships**: Lives alongside `tools_dialog_open`, `batch_dialog_open`, etc. in `Dialogs`. Displayed via conditional overlay when true.
- **State transitions**: `closed --OpenAboutDialog--> open --close(X)--> closed`; rapid toggles remain consistent (single bool; no queued windows).

### 2. BuildMetadata

- **Location**: `build.rs` → compile-time env vars; `src/app/ui/about_dialog.rs` reads via `env!`
- **Type**: Tuple of four `&'static str` constants
  - `BUILD_BRANCH: &str` — git branch name or `"unknown"`
  - `BUILD_COMMIT_HASH: &str` — full 40-char SHA or `"unknown"`
  - `BUILD_COMMIT_SHORT_HASH: &str` — 7–8 char prefix (first 8 chars of full hash when available)
  - `BUILD_DATE: &str` — `YYYY-MM-DD` computed from `SOURCE_DATE_EPOCH` or `SystemTime::now()`
- **Validation**:
  - `BUILD_BRANCH`, `BUILD_COMMIT_HASH`, `BUILD_DATE` non-empty (fallback `"unknown"` satisfies)
  - `BUILD_COMMIT_HASH` is 40 hex chars or `"unknown"`; `BUILD_COMMIT_SHORT_HASH` is 7–8 chars or `"unknown"`; short hash is prefix of full hash when full is known
  - `BUILD_DATE` matches `^\d{4}-\d{2}-\d{2}$` (enforced indirectly by `format_current_date` test)
- **Lifecycle**: Immutable after compile; no mutation at runtime. Build script reruns only when `.git/HEAD` or `.git/index` changes.
- **Display labels**: `ABOUT_BRANCH_LABEL`, `ABOUT_COMMIT_LABEL`, `ABOUT_DATE_LABEL` from `strings.rs`.

### 3. LicenseText

- **Location**: `src/app/ui/about_dialog.rs` — `const LICENSE_TEXT: &str = include_str!("../../../LICENSE")`
- **Type**: `&'static str` — verbatim contents of repo root `LICENSE`
- **Semantics**: Full MIT license text shown in a capped vertical scroll area (`max_height 140.0`, `id_salt "about_license_scroll"`) inside a framed region.
- **Validation**: Non-empty, contains expected substrings (`"MIT License"`, `"Copyright (c) 2026 Martin Kuhne"`, `"Permission is hereby granted"`). Build fails fast if file missing (compile-time `include_str!` error) — satisfies edge case in spec.
- **Relationships**: Rendered under `ABOUT_LICENSE_HEADER` heading.

### 4. Attribution

- **Location**: `src/app/ui/attributions.rs`
- **Type**:
  ```rust
  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct Attribution {
      pub name: &'static str,        // crate package name
      pub authors: &'static str,     // author/maintainer display string
      pub github_url: &'static str,  // https://github.com/...
  }
  pub const DIRECT_DEPENDENCIES: &[Attribution] = &[...58...];
  ```
- **Validation rules** (from FR-010/FR-012 and tests):
  - `name`, `authors`, `github_url` non-empty
  - `github_url` starts with `https://github.com/`
  - Slice sorted alphabetically by `name` (`windows` < `walkdir` etc. — strictly increasing)
  - No duplicate `name` values
  - Length == 58 and exactly matches direct third-party dependencies across workspace `Cargo.toml` members (excluding workspace members `fastmd`, `fastmd-agent`, `fastmd-pdf`, `fastmd-tool-macros`)
- **Display**: Rendered under `ABOUT_ATTRIBUTIONS_HEADER` inside `ScrollArea::vertical().id_salt("about_attributions_scroll").max_height(240.0)`. Each row: `name` in strong font, `authors` plain, clickable `github_url` via `ui.hyperlink_to(...)` which opens in external browser. Row `id_salt` uses `(name, "attribution_row")` for stable `egui::Id`.
- **Relationships**: Static catalog; no references to runtime state.

### 5. FirstRunVersionStamp

- **Location**: `src/app/ui/persisted.rs` — field `about_shown_for_version: Option<String>` on `pub struct PersistedUiState` (with `#[serde(default)]`)
- **Type**: `Option<String>` — the `fastmd` crate version (`env!("CARGO_PKG_VERSION")`) the About dialog was last shown for; `None` when never shown (including state written by builds pre-dating the field).
- **Semantics**: Evaluated once at startup in `FastMdApp::new` (after restore + schema migration) by the pure helper `should_auto_show_about(recorded: Option<&str>, current: &str) -> bool`: `None` or version mismatch → set `Dialogs.about_dialog_open = true` and record `current`; match → leave closed. The recorded version persists via the existing `save()` path with no new logic. Unreadable/corrupt storage falls back to `PersistedUiState::default()` (fail open: dialog shows once).
- **Validation**:
  - `None` → auto-show; `Some(v)` with `v == current` → no auto-show; `Some(v)` with `v != current` → auto-show once and overwrite with `current`
  - Round-trips through serde JSON; old state files without the field deserialize to `None`
  - No `CURRENT_SCHEMA_VERSION` bump required (backwards-compatible shape change)
- **Relationships**: Read once at startup, written once per auto-show; never touched by the hamburger-menu path (menu opens set the dialog flag without recording).

## Relationships Overview

```
Dialogs.about_dialog_open --controls visibility of--> AboutDialog
AboutDialog --reads--> BuildMetadata (env! constants)
AboutDialog --reads--> LicenseText (include_str!)
AboutDialog --iterates--> Attribution[58] via DIRECT_DEPENDENCIES
AboutDialog --writes on X--> Dialogs.about_dialog_open = false
UserCommand::OpenAboutDialog --sets--> Dialogs.about_dialog_open = true
FastMdApp::new --reads--> PersistedUiState.about_shown_for_version
FastMdApp::new --sets on first-run--> Dialogs.about_dialog_open = true + records version
```

## Persistence & Serialization

No database. `PersistedUiState` (including `about_shown_for_version`) serializes to JSON via `eframe::Storage` under `PERSISTED_UI_STATE_KEY` on exit (`FastMdApp::save`) and restores on launch (`FastMdApp::new`); corrupt payloads fall back to defaults (fail open). All other entities are compile-time or in-memory UI state. Build metadata regeneration is handled by Cargo build script rerun logic, not by application code.

## Change Propagation

- Adding a new direct dependency to any workspace `Cargo.toml` requires appending one `Attribution` entry to `DIRECT_DEPENDENCIES` and keeping the slice sorted; the completeness test will fail until updated.
- License text updates automatically via `include_str!` — no code change needed beyond updating the root `LICENSE` file.
- Branch/hash/date refresh automatically on next build when `.git/HEAD` changes or `SOURCE_DATE_EPOCH` differs.
