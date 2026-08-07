## Objective

Fixes the issue where the application window size and custom UI states (such as panel widths and expanded directories) were not being remembered across restarts.

## Changes

### Bug Fix
- **src/desktop/src/main.rs:** Added `.with_app_id("fastmd")` to the `egui::ViewportBuilder` inside `NativeOptions`. This enables eframe's built-in persistence by instructing it to use the "fastmd" key to save both native window properties and the custom application storage context.

## How

The issue occurred because eframe 0.35 requires an explicit `app_id` configured on the viewport builder to activate its persistent `Storage` backend. Without it, the application receives a temporary in-memory storage dictionary that vanishes when the app is closed. By passing `"fastmd"` to `with_app_id()`, eframe seamlessly persists the native window state (like size and position) directly, and it also persists our custom `ppai_ui_state` object which stores the side-panel widths and directory expanded statuses.

## Why

The window size, position, and layout were failing to persist because the application storage wasn't being saved to the OS. Setting the application ID explicitly resolves this by enabling eframe's persistence layer.

## Quality Gates

- [x] Build: `cargo check` — succeeded cleanly with 0 warnings
- [x] Lint: `cargo clippy -- -D warnings` — succeeded cleanly with 0 issues
- [x] Tests: `cargo nextest run` — all 1038 tests passed successfully (14 skipped)
- [ ] Static Analysis: not configured
- [ ] Secrets Scan: not configured
