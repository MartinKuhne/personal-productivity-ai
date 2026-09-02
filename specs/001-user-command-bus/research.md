# Phase 0: Research & Decisions

## Technical Context Decisions

No unknown technical context variables ("NEEDS CLARIFICATION") were found. The technical context is fully resolved based on the existing `fastmd` ecosystem:

- **Decision (Language)**: Rust 2024 Edition.
  - **Rationale**: The project is already written in Rust, and changing languages is out of scope.
- **Decision (Dependencies)**: `tokio` (for `tokio::sync::broadcast::channel`), `eframe`/`egui`.
  - **Rationale**: The `Bus<T>` pattern is already implemented using `tokio::sync::broadcast::channel` in `src/app/bus/core.rs` (e.g. `Bus<FileEvent>`). Reusing it maintains consistency.
- **Decision (Testing)**: `cargo nextest` and `egui_kittest` (for UI Tier-4 click-capture tests).
  - **Rationale**: Existing testing infrastructure; preserves the `apply_*` helper testability.

## Architectural Approach

The feature introduces a unified `UserCommand` event bus. 

- **Decision**: Define `UserCommand` as a `Clone + Send + 'static` enum.
  - **Rationale**: Required by `tokio::sync::broadcast`.
- **Decision**: Expose `UserCommandProducer` for UI components.
  - **Rationale**: Avoids `&mut AppOrchestrator` borrows in UI components.
- **Decision**: Centralize execution in `CommandExecutor`.
  - **Rationale**: Removes UI coupling to state mutation and eliminates the `submit_prompt` deferred action slot.
