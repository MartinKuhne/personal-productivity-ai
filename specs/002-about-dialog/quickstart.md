# Quickstart: About Dialog Validation

**Feature**: 002-about-dialog | **Spec**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md) | **Contract**: [contracts/about-dialog.md](contracts/about-dialog.md)

## Prerequisites

- Rust toolchain with `cargo nextest` and `egui_kittest` support
- Linux or Windows desktop with display server (for `egui` window tests)
- Clean working tree on branch `002-about-dialog`

## Setup

```powershell
cargo check --quiet
cargo fmt --check
cargo clippy -- -D warnings
cargo doc --no-deps --quiet
```

## Automated Validation (no display needed)

```powershell
# Unit + attribution invariants
cargo nextest run -p fastmd --status-level fail --show-progress none

# Full workspace
cargo nextest run --workspace --status-level fail --show-progress none
```

Expected: all tests pass including new suites:
- `strings::tests::test_hamburger_menu_strings` (covers `ABOUT_*` constants)
- `dialogs::tests::test_new_dialogs_is_empty` (covers `about_dialog_open == false`)
- `panels::top_tests::test_about_button_click_opens_dialog` (Tier-4 harness: hamburger → About → bus `OpenAboutDialog` + `ABOUT_EVENT`)
- `attributions::tests::*`:
  - `all_entries_have_valid_fields` — 58 entries non-empty https URLs
  - `slice_is_sorted_and_unique`
  - `completeness_against_cargo_manifests` — parses `Cargo.toml` workspace members
- `about_dialog::tests::*`:
  - `dialog_renders_without_panic` (kittest smoke)
  - `header_shows_app_name_and_copyright`
  - `build_labels_and_values_present`
  - `commit_hover_shows_full_hash` (tooltip)
  - `commit_click_copies_full_hash` (clipboard)
  - `license_scroll_contains_mit_text` + `license_is_scrollable`
  - `attributions_all_58_present_and_scrollable`
  - `close_button_clears_dialog_flag`
- First-run auto-show (FR-016 / SC-008):
  - `should_auto_show_about_*` — pure matrix: `None` → true, same version → false, older version → true
  - `persisted::tests::*` — `about_shown_for_version` round-trips; state without the field deserializes to `None`
  - Startup wiring test — fresh `PersistedUiState::default()` auto-opens; recorded current version does not
```

## Manual Validation (interactive)

```powershell
cargo run
```

1. **Hamburger entry**: Click ☰ (top-right toolbar) → verify `About FastMD...` entry appears after a separator near bottom of menu.
2. **Open dialog**: Click `About FastMD...` → About dialog appears titled `About FastMD`; hamburger menu closes.
3. **Header**: Verify `FastMD Viewer` bold heading and `Copyright (c) 2026 Martin Kuhne`.
4. **Build metadata**: Row shows `Branch: <name>`, `Commit: <7-8 chars>`, `Built: YYYY-MM-DD`. On git-enabled build each field is not `unknown`.
5. **Hover full hash**: Hover over commit short hash → tooltip shows `Full commit: <40 chars>\nClick to copy full commit hash`.
6. **Click-to-copy**: Click short hash → full 40-char hash is on clipboard (paste into editor to confirm). A transient notification/badge may show `Commit hash copied to clipboard`.
7. **License**: Scroll in `License` section (capped ~140px) → full MIT text visible, selectable, main window does not scroll.
8. **Attributions**: Scroll in `Third-Party Attributions` section (capped ~240px) → 58 rows alphabetically sorted, each row has crate name (strong), authors, and `https://github.com/...` link.
9. **Link open**: Click any attribution GitHub link (e.g., `anyhow`) → external browser opens repository.
10. **Close**: Click title-bar `X` → dialog closes; reopen via hamburger → works again. With window at minimum size, both scroll areas remain usable.
11. **Offline fallback**: Build with `GIT_BRANCH=unknown GIT_COMMIT=unknown cargo build` → dialog shows `unknown` for branch/commit without panic.
12. **First-run auto-show**: Launch with fresh UI state (clear the app's persisted state) → About dialog opens automatically on startup. Close it, restart → no auto-show. Simulate an upgrade (recorded version older than `CARGO_PKG_VERSION`) → auto-shows exactly once, then stays quiet on subsequent starts.

## Quality Gate (before marking complete)

```powershell
cargo check --quiet
cargo nextest run --status-level fail --show-progress none
cargo clippy -- -D warnings
cargo fmt --check
cargo doc --no-deps --quiet
```

All must pass cleanly per AGENTS.md Quality Gate.

## Troubleshooting

- `test_about_button_click_opens_dialog` fails → ensure `panels/top.rs` publishes `UserCommand::OpenAboutDialog` and fires `ABOUT_EVENT`.
- `completeness_against_cargo_manifests` fails → a new direct dependency was added without updating `DIRECT_DEPENDENCIES`; add the missing `Attribution` and re-sort.
- Clipboard test fails headless → gate on `ctx.output.copied_text` or skip with `#[ignore]` in CI headless; main assertion is no panic.
- `license_scroll_contains_mit_text` fails → verify `LICENSE` exists at repo root and `include_str!` path is `../../../LICENSE` from `src/app/ui/about_dialog.rs`.
