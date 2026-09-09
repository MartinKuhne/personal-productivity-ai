---
description: Implement comprehensive unit tests for newly added or modified code, focusing on testing all code paths, error conditions, and boundary conditions
---

## User Input

```text
$ARGUMENTS
```

You **MUST** consider the user input before proceeding (if not empty). Arguments may specify:
- Specific crate (e.g. `fastmd`, `fastmd-agent`, `fastmd-pdf`, `fastmd-tool-macros`)
- Specific source file or directory (e.g. `src/agent/tools/csv_db/query.rs`)
- If `$ARGUMENTS` is empty, target all source files touched in the active feature branch, `tasks.md`, or working tree.

## Pre-Execution Checks

**Check for extension hooks (before unit-tests)**:
- Check if `.specify/extensions.yml` exists in the project root.
- If it exists, read it and look for entries under the `hooks.before_unit_tests` key.
- If the YAML cannot be parsed or is invalid, skip hook checking silently and continue normally.
- Filter out hooks where `enabled` is explicitly `false`. Treat hooks without an `enabled` field as enabled by default.
- For each remaining hook, do **not** attempt to interpret or evaluate hook `condition` expressions:
  - If the hook has no `condition` field, or it is null/empty, treat the hook as executable
  - If the hook defines a non-empty `condition`, skip the hook and leave condition evaluation to the HookExecutor implementation
- For each executable hook, output the following based on its `optional` flag:
  - **Optional hook** (`optional: true`):
    ```text
    ## Extension Hooks

    **Optional Pre-Hook**: {extension}
    Command: `/{command}`
    Description: {description}

    Prompt: {prompt}
    To execute: `/{command}`
    ```
  - **Mandatory hook** (`optional: false`):
    ```text
    ## Extension Hooks

    **Automatic Pre-Hook**: {extension}
    Executing: `/{command}`
    EXECUTE_COMMAND: {command}

    Wait for the result of the hook command before proceeding to the Goal.
    ```
    After emitting the block above you MUST actually invoke the hook and wait for it to finish before continuing. Run it the same way you would run the command yourself in this agent/session. Emitting the block alone does not run the hook.
- If no hooks are registered or `.specify/extensions.yml` does not exist, skip silently.

## Goal

Implement exhaustive, high-coverage unit tests for all newly added, modified, or affected code in the active feature. The primary objective is to guarantee that **all code paths**, **error and failure conditions**, and **boundary/corner cases** are completely covered by unit tests, adhering strictly to the repository architecture, safety guidelines, and quality gates.

## Core Rules & Testing Guidelines

You **MUST** strictly follow the project's testing principles from `AGENTS.md` and `.specify/memory/constitution.md`:

1. **[RUST-001] Test File Location**:
   - Unit tests SHOULD be kept in a separate file named `<file>_tests.rs`.
   - Never clutter production source files with massive inline test blocks.

2. **[RUST-002] Test Runner**:
   - You **MUST** use `cargo nextest`. Do NOT use `cargo test` directly unless invoking doc-tests.
   - Use the appropriate crate commands:
     - `fastmd`: `cargo nextest run -p fastmd`
     - `fastmd-agent`: `cargo nextest run -p fastmd-agent`
     - `fastmd-pdf`: `cargo nextest run -p fastmd-pdf`
     - `fastmd-tool-macros`: `cargo nextest run -p fastmd-tool-macros`
     - workspace: `cargo nextest run --workspace`

3. **[RUST-003] Comprehensive Coverage**:
   - Every modified or created function, method, struct, enum, and branch **MUST** be covered by unit tests.
   - Happy paths, error paths, corner cases, and boundary conditions must all be tested.

4. **[RUST-006] Test Isolation & Safety Shield (CRITICAL)**:
   - Tests **MUST NEVER** read, write, or mutate real user filesystem paths, live configuration files (`%APPDATA%`, `~/.fastmd*`, `USERPROFILE`), or production databases.
   - All tests involving persistence MUST use mock/noop storage handlers (`NoopConfigStorage`, `InMemoryConfigStorage`) or isolated `tempfile::TempDir` paths.
   - Functions targeting platform-default user paths MUST contain runtime panic shields preventing execution in test environments.

5. **[RUST-020] Pure & Honest Functions**:
   - Write modular code with minimal side effects. Functions should be testable without hidden global state.

6. **[RUST-021] String Constants**:
   - Use string constants for repeat strings or user-facing literals.

7. **[RUST-024] Injected Storage**:
   - Persisting state or config must use injected storage handlers, never direct platform-default paths.

8. **[RUST-056] & [RUST-057] Test Sidecar Extraction**:
   - When a source file's `#[cfg(test)] mod tests { ... }` block exceeds ~150 lines or more than half the file, extract the test body into a sibling sidecar file `<file>_tests.rs`.
   - Declare it from the source file with `#[cfg(test)] mod tests;`.
   - The source file's `//!` module doc comment MUST end with the header pointer:
     `//! Unit tests live in the sibling \`<filename>_tests.rs\` sidecar.`

9. **[RUST-058] `app/` is Egui-Free**:
   - No `.rs` file under `app/` may import `eframe::egui`, `egui`, or any UI crate.

## Comprehensive Testing Matrix

Every target function and module MUST be tested against this 4-dimensional matrix:

### 1. Happy Path & Nominal Execution
- Verify nominal inputs produce correct return values and expected state transformations.
- Verify default configurations and typical usage patterns.
- Verify complete lifecycle transitions (initialization -> processing -> successful termination).

### 2. Error & Failure Paths
- **Every `Result::Err`**: Trigger every error variant declared in error enums.
- **Every `?` operator**: Ensure failure modes in callee functions propagate correctly.
- **Every `Option::None`**: Test cases where optional values are `None` or missing.
- **Validation Errors**: Pass invalid formats, negative counts, missing required parameters, unauthorized requests.
- **I/O & Protocol Failures**: Simulate network drops, socket timeouts, 4xx/5xx HTTP statuses, malformed JSON/YAML payloads, truncated streams.
- **Filesystem Failures**: Missing files, access denied (using mock/temp directory permissions), invalid path characters.

### 3. Boundary & Extreme Conditions
- **Zero & Empty**: Empty strings (`""`), zero numbers (`0`, `0.0`), empty slices/vectors (`&[]`, `vec![]`), empty maps, 0-byte files.
- **Maximum & Capacity Limits**: Maximum buffer size, max string length, integer boundary limits (`usize::MAX`, `i64::MAX`, `i64::MIN`), full caches/queues.
- **Off-by-One & Fenceposts**: Slicing boundaries (index 0, length - 1, length, length + 1), pagination boundaries (page 0, page 1, last page, past last page), table columns.
- **String & Path Encodings**: Trailing slashes, leading slashes, path traversal sequences (`../`, `..\`), special characters (`\0`, `\r\n`, tabs), multi-byte UTF-8 sequences, Unicode normalization, emojis.
- **Concurrency & Timing**: Cancellation token triggered before/during execution, zero-timeout expirations, re-entrant calls.

### 4. Corner Cases & State Invariants
- **Enum Exhaustiveness**: Test all enum variants, especially newly added variants, ensuring matching logic handles them.
- **State Invariants**: Attempt out-of-order method calls (e.g. calling read before open, or finalize twice).
- **Cleanup Invariants**: Ensure resources (temp files, background handles) are safely cleaned up even if execution aborts or errors.

## Execution Outline

1. **Identify Target Files and Changes**:
   - Check if an active feature is registered in `.specify/feature.json`. If present, inspect `tasks.md` and feature specs.
   - Run `git status` and `git diff --name-only` (or compare to the base branch) to identify all modified or created Rust source files (`src/**/*.rs`).
   - If `$ARGUMENTS` specifies target paths, filter to those targets.
   - Group target files by crate (`fastmd`, `fastmd-agent`, `fastmd-pdf`, `fastmd-tool-macros`).

2. **Extract Code Paths & Audit Existing Tests**:
   - For each target file:
     - Read the source code and identify all functions, structs, and impl blocks.
     - Enumerate each branch (`if`, `match`, `?`, `unwrap_or`, `while let`).
     - Check if a sibling `<file>_tests.rs` or inline `mod tests` exists.
     - Identify gaps where branches, error conditions, or boundary inputs lack test coverage.

3. **Implement Unit Tests**:
   - For each file with identified coverage gaps:
     - Follow the sidecar convention: create or update `<file>_tests.rs`.
     - In `<file>.rs`, ensure `#[cfg(test)] mod tests;` is present and add the sidecar pointer to `//!` doc comment if applicable.
     - Structure test functions with clear, descriptive names:
       `test_<function>_<scenario>_<expected_outcome>()`
     - Use isolated mocks or `tempfile::TempDir` where persistence or filesystem interaction is needed.
     - Cover:
       - Happy path tests
       - Error paths (asserting exact error variants using `assert!(matches!(res, Err(...)))`)
       - Boundary and corner cases (empty, max, edge values)

4. **Run Tests with Nextest**:
   - Execute the test suite for touched crates:
     - `cargo nextest run -p <crate_name> --filter <test_filter>`
   - Verify every new test passes cleanly.
   - If a test fails:
     - Determine if the test assertion is faulty or if the production code contains a genuine bug.
     - If it's a bug in the production code, fix it carefully while maintaining backward compatibility.
     - Re-run `cargo nextest run` until all tests pass with 0 failures.

5. **Run Quality Gate Checks**:
   - Run `cargo check --quiet`
   - Run `cargo clippy -- -D warnings`
   - Run `cargo fmt --check`
   - Verify zero warnings and clean passes.

6. **Check for Extension Hooks (after unit-tests)**:
   - Check `.specify/extensions.yml` for `hooks.after_unit_tests`.
   - Dispatch any registered hooks following the mandatory/optional protocol.

7. **Completion Report**:
   - Present a concise report containing:
     - Table of touched files and their test sidecars
     - Summary of test cases added (Happy paths, Error paths, Boundary conditions)
     - Nextest test execution results (number of tests passed)
     - Quality gate status (`check`, `clippy`, `fmt`)

## Done When

- [ ] All target source files audited for missing unit test coverage
- [ ] Comprehensive unit tests implemented in `<file>_tests.rs` covering:
  - [ ] All happy paths and nominal flows
  - [ ] All error paths, `Result::Err`, and `?` propagations
  - [ ] All boundary conditions (empty, zero, limits, string/path edge cases)
- [ ] Safe test isolation verified (no live config or real user paths mutated)
- [ ] `cargo nextest run` passes cleanly with 0 failures
- [ ] Quality gates (`cargo check`, `cargo clippy -- -D warnings`, `cargo fmt`) pass cleanly
- [ ] Completion report delivered to the user
