# Contract: About Dialog

**Feature**: 002-about-dialog | **Date**: 2026-09-04 | **Type**: UI Contract (desktop egui)

This contract documents the externally observable behavior of the About Dialog for harness tests and manual QA. It is implementation-agnostic except where egui widget contracts are the observable surface.

## 1. Hamburger Menu Entry

- **Location**: Top toolbar → hamburger menu button (`HAMBURGER_MENU_BUTTON` = Phosphor `LIST`) → `egui::containers::menu_button` with `id_salt HAMBURGER_MENU_ID_SALT`.
- **Entry**: After existing items and a `ui.separator()`, a button `ui.button(strings::MENU_ABOUT)` with label `"About FastMD..."`.
- **Helper**: `pub fn apply_about_button_click() -> UserCommand { UserCommand::OpenAboutDialog }` (pure, testable per FMD-068).
- **On click**:
  1. `producer.publish(apply_about_button_click())` → `Bus<UserCommand>` receives `OpenAboutDialog`.
  2. `on_click(strings::ABOUT_EVENT)` where `ABOUT_EVENT = "about_button"` for Tier-4 harness capture.
  3. `ui.close()` closes the hamburger menu.

**Harness assertions** (`panels/top_tests.rs`):
```rust
captured.contains(&strings::ABOUT_EVENT)
assert_bus_contains(&app, UserCommand::OpenAboutDialog)
```

## 2. Command Routing

- **Enum**: `UserCommand::OpenAboutDialog` variant (no payload).
- **Executor**: `CommandExecutor::apply_user_command(UserCommand::OpenAboutDialog) => self.dialogs.about_dialog_open = true`.
- **Bus**: `Bus<UserCommand>` broadcast; no lag handling needed (single-frame drain in orchestrator).
- **Test**: publish → drain → assert `dialogs.about_dialog_open == true`.

## 3. Dialog Window

- **Constructor**: `egui::Window::new(strings::ABOUT_DIALOG_TITLE)` where `ABOUT_DIALOG_TITLE = "About FastMD"`.
- **Params**: `.id(egui::Id::new("about_dialog"))`, `.open(&mut app.dialogs.about_dialog_open)`, `.resizable(true)`, `.default_size([620.0, 580.0])`, `.min_size([480.0, 400.0])`, `.collapsible(false)` (or default).
- **Overlay**: Called from `render_overlays` (or `ui::app::render`) as:
  ```rust
  if app.orchestrator.dialogs.about_dialog_open {
      crate::ui::about_dialog::show_about_dialog(ctx, app);
  }
  ```
- **Close**: Title-bar `X` toggles `open` to `false`; next frame `about_dialog_open == false`; dialog not rendered.
- **Invariants**: Window is modal in user sense (takes focus) but does not block other dialogs via code; closing restores previous state; rapid open/close toggles are idempotent.

## 4. Header Section

| Element | Content | Style |
|---------|---------|-------|
| App name | `strings::ABOUT_APP_NAME = "FastMD Viewer"` | `RichText::new(...).strong().heading()` or `ui.heading` |
| Copyright | `strings::ABOUT_COPYRIGHT = "Copyright (c) 2026 Martin Kuhne"` | plain small text |
| Branch row | `strings::ABOUT_BRANCH_LABEL` + `BUILD_BRANCH` | horizontal `Label` |
| Commit row | `strings::ABOUT_COMMIT_LABEL` + short hash label (see §5) | clickable label with tooltip |
| Date row | `strings::ABOUT_DATE_LABEL` + `BUILD_DATE` | plain label |

`BUILD_*` are compile-time env constants via `env!("BUILD_BRANCH")` etc.; fallback `"unknown"` when git unavailable.

## 5. Commit Hash Hover & Click-to-Copy

- **Display**: Short hash `BUILD_COMMIT_SHORT_HASH` (7–8 chars) rendered as a selectable/clickable `Label`.
- **Hover**: `.on_hover_text(format!("Full commit: {BUILD_COMMIT_HASH}\n{ABOUT_COPY_COMMIT_TOOLTIP}"))` where `ABOUT_COPY_COMMIT_TOOLTIP = "Click to copy full commit hash"`. Full hash is 40-char hex or `"unknown"`.
- **Click**: On `response.clicked()`:
  ```rust
  ctx.copy_text(BUILD_COMMIT_HASH.to_owned());
  // optional: transient notification via ctx data / toasts that ABOUT_COPIED_NOTIFICATION was shown
  ```
  Copies full hash (not short) to egui clipboard. In headless/CI tests clipboard may be noop but call must not panic.
- **Harness**: kittest click on Commit label → assert `ctx.output` clipboard text equals `BUILD_COMMIT_HASH` (or inspect via test helper).

## 6. License Section

- **Header**: `Label` with `strings::ABOUT_LICENSE_HEADER = "License"`, styled as section heading.
- **Scroll area**: `egui::ScrollArea::vertical().id_salt("about_license_scroll").max_height(140.0).show(ui, |ui| { Frame::group(ui.style()).show(ui, |ui| ui.label(LICENSE_TEXT) ) })`
- **Text**: `const LICENSE_TEXT: &str = include_str!("../../../LICENSE")` — full MIT text. Must contain `"MIT License"` substring.
- **Invariant**: Text is selectable but scroll is internal; main window does not scroll when License area scrolls.

## 7. Attributions Section

- **Header**: `strings::ABOUT_ATTRIBUTIONS_HEADER = "Third-Party Attributions"`.
- **Scroll area**: `egui::ScrollArea::vertical().id_salt("about_attributions_scroll").max_height(240.0)`.
- **Rows**: For each `attr in DIRECT_DEPENDENCIES` (58 entries, sorted by `name`):
  - Crate name: `ui.strong(attr.name)` or `RichText::strong`.
  - Authors: `ui.label(attr.authors)` (fallback `ABOUT_UNKNOWN_AUTHOR` if needed, though dataset has no unknowns).
  - Link: `ui.hyperlink_to(attr.github_url, attr.github_url)` or `ui.hyperlink_to(attr.name, attr.github_url)` — whichever is chosen must consistently open the GitHub URL in external browser via `open_url`/`webbrowser`. Must have `on_hover_text` showing URL.
  - Row id: `ui.push_id((attr.name, "attribution_row"), |ui| { ... })` for stable IDs.
- **Invariants** (covered by `attributions_tests.rs`):
  - `DIRECT_DEPENDENCIES.len() == 58`
  - Every `name`, `authors`, `github_url` non-empty
  - `github_url.starts_with("https://github.com/")`
  - Sorted ascending by `name` and no duplicates
  - Set equals parsed Cargo.toml direct deps minus workspace members

## 8. Wiring & Module Declarations

- **`src/app/ui/mod.rs`**: `pub mod about_dialog; pub mod attributions;`
- **`src/app/ui/attributions.rs`**: `pub struct Attribution` + `pub const DIRECT_DEPENDENCIES`
- **`src/app/ui/about_dialog.rs`**: `pub fn show_about_dialog(ctx: &egui::Context, app: &mut FastMdApp)` (+ optional `show_about_dialog_for_test` helper returning `egui::Response`s).

## 9. String Constants Contract

All literals in §1–§7 must be sourced from `strings.rs` — no inline strings — and each constant carries a `///` doc comment (RUST-021, crate-level doc lint). Current values:

```
MENU_ABOUT = "About FastMD..."
ABOUT_DIALOG_TITLE = "About FastMD"
ABOUT_APP_NAME = "FastMD Viewer"
ABOUT_COPYRIGHT = "Copyright (c) 2026 Martin Kuhne"
ABOUT_BRANCH_LABEL = "Branch:"
ABOUT_COMMIT_LABEL = "Commit:"
ABOUT_DATE_LABEL = "Built:"
ABOUT_LICENSE_HEADER = "License"
ABOUT_ATTRIBUTIONS_HEADER = "Third-Party Attributions"
ABOUT_COPY_COMMIT_TOOLTIP = "Click to copy full commit hash"
ABOUT_COPIED_NOTIFICATION = "Commit hash copied to clipboard"
ABOUT_UNKNOWN_AUTHOR = "Unknown"
ABOUT_COL_CRATE = "Crate"
ABOUT_COL_AUTHORS = "Authors"
ABOUT_COL_REPO = "Repository"
ABOUT_EVENT = "about_button"
```

## 10. First-Run Auto-Show

- **Trigger**: `FastMdApp::new` (`src/app/ui/app/init.rs`), after `PersistedUiState` restore + schema migration and before the first frame.
- **Decision**: pure helper `should_auto_show_about(recorded: Option<&str>, current: &str) -> bool` where `recorded` is `persisted_ui_state.about_shown_for_version` and `current` is `env!("CARGO_PKG_VERSION")`:
  - `None` (fresh state, or state written before the field existed) → auto-show.
  - `Some(v)` with `v != current` (upgraded) → auto-show once.
  - `Some(v)` with `v == current` → no auto-show.
- **Effect**: on auto-show, `dialogs.about_dialog_open = true` and the current version is recorded into `about_shown_for_version` (persisted by the existing `save()` path; no config-file involvement). The hamburger-menu path opens the dialog without touching the flag.
- **Failure mode**: unreadable/corrupt storage falls back to `PersistedUiState::default()` → dialog shows once (fail open), never crashes or blocks startup.
- **Harness assertions** (unit, no window needed):
  ```rust
  assert!(should_auto_show_about(None, "0.2.0"));
  assert!(!should_auto_show_about(Some("0.2.0"), "0.2.0"));
  assert!(should_auto_show_about(Some("0.1.0"), "0.2.0"));
  ```

## 11. Non-Goals

- No persistence, no config, no background work, no network fetch.
- Not localized (English only for v1).
- Not exposed as a CLI or HTTP API — purely UI.
