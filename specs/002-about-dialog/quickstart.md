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

> **Note (2026-10-31)**: the suite names listed below are the original egui-era
> ones. The shipped implementation is the iced rebuild; its tests live in
> `src/app/ui_iced/about_tests.rs` and `src/app/ui_iced/attributions_tests.rs`
> (`cargo nextest run -p fastmd --lib about`, `--lib attributions`).

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
  - `all_entries_have_valid_fields` — 61 entries non-empty https URLs
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
3. **Header**: Verify `FastMD AI` bold heading and `Copyright (c) 2026 Martin Kuhne`.
4. **Build metadata**: Row shows `Branch: <name>`, `Commit: <7-8 chars>`, `Built: YYYY-MM-DD`. On git-enabled build each field is not `unknown`.
5. **Click-to-copy**: Click the commit cell → full 40-char hash is on clipboard (paste into editor to confirm). There is **no hover tooltip** on iced; the reveal-on-hover step of the original plan is not implemented.
6. **Size**: The card fills 80 % of the window on both axes and stays centred; shrinking the window shrinks the card (a viewport gutter keeps it inside the screen).
7. **License**: Scroll in `License` section → full MIT text visible, selectable, main window does not scroll.
8. **Attributions (inline)**: Scroll in the `Third-Party Attributions` section **inside the same card** → 61 rows alphabetically sorted, each row has crate name, authors, and a `https://github.com/...` link. There is no "View third-party attributions…" button and no second window.
9. **Link open**: Click any attribution GitHub link (e.g., `anyhow`) → external browser opens repository.
10. **Close**: Click `Close` in the card footer, or press `Escape` → the card closes; reopen via hamburger → works again. With a small window, both scroll areas remain usable.
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
