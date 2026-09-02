# Quickstart & Validation Guide

This guide describes how to validate the `UserCommand` bus refactoring end-to-end to ensure the UI successfully routes intent through the bus and the orchestrator applies it.

## Prerequisites

- Cargo toolchain with `nextest` installed (`cargo nextest`)
- Access to the `fastmd` codebase.

## Validation Scenarios

### 1. Compile & Lint Check
Ensure the refactoring hasn't broken any module boundaries, and that `UserCommand` derivations are sound.

```bash
cargo check --quiet
cargo clippy -- -D warnings
cargo fmt --check
```
*Expected Outcome*: All checks pass without errors.

### 2. Tier-4 Click-Capture Test Validation
Run the existing UI interaction tests to ensure that `apply_*` helpers (which are now refactored to return `UserCommand` instead of mutating state inline) successfully emit the expected commands.

```bash
cargo nextest run --status-level fail --show-progress none
```
*Expected Outcome*: The test suite successfully passes. Specifically, any click interaction in a test will result in a `UserCommand` being evaluated in isolation.

### 3. Run the Application
Launch the FastMD desktop application to manually verify that command routing functions smoothly across different UI panels without lost intents (especially around file tree snapshots).

```bash
cargo run --bin fastmd
```
*Expected Outcome*: 
- Clicking an item in the file tree opens it (validates `SelectFile` via `TreeNodeContext`).
- Typing a prompt and pressing "Send" launches the AI agent (validates `RunAgent` command).
- The top-level toolbar's tool/batch dialogs open as expected (validates toolbar `OpenToolsDialog`).
