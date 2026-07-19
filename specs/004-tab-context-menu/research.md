# Research: Tab Context Menu

## Decision 1: Operating System Integrations
**Decision**: Re-use `std::process::Command` calls existing in `tree.rs` for "Show in File Explorer" and "Open in Editor".
**Rationale**: `tree.rs` already contains tested, cross-platform code for these operations using `explorer` and `cmd` on Windows.
**Alternatives considered**: Introducing external crates like `open`, but that would add unnecessary dependencies and duplication.

## Decision 2: Clipboard Access
**Decision**: Use `egui`'s built-in clipboard support (`ui.output_mut(|o| o.copied_text = ...)`).
**Rationale**: It is already used across the app (e.g. `tree.rs`) and works natively across platforms without extra dependencies.
**Alternatives considered**: Using `arboard` or `copypasta`, but not needed.

## Decision 3: Triggering Tasks
**Decision**: Emit standard task execution structures, reusing logic from `tree.rs` for triggering the "Format Markdown" task.
**Rationale**: Keeps behavior perfectly consistent between the tab and tree context menus.
**Alternatives considered**: N/A.
