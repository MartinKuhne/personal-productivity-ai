# AI Agent Instructions

When working on this codebase, all AI agents must adhere strictly to the following guidelines.

These are the **repo-wide** principles that apply to every component. Each
component also has its own `AGENTS.md` with rules specific to that directory;
always read the nearest one before editing. See
[Component-specific rules](#component-specific-rules) below for the index.

## 1. Small Iterations
- Work in small, manageable iterations.
- Do not attempt sweeping, massive refactors in a single pass.
- Break tasks into logical, contained and testable steps, implement a change, and verify it before moving to the next step.

## 2. Test-Driven Changes
- Create unit and functional tests for any new features or changes.
- Ensure that you prove your code works through tests, rather than assumptions.

## 3. Keep Tests Updated
- Always update relevant tests after making code changes or bug fixes.
- If a bug is fixed, a test must be added or updated to cover the regression and prevent it from recurring.

## 4. Warnings and Compilation
- **Fix all warnings:** You must fix all new and existing compile or test warnings before considering a task "done."
- Never leave dangling unused variables, imports, or future-incompatibilities if they are within your control to fix.
- Ensure the component's build and test commands run perfectly clean (e.g. `cargo check` / `cargo nextest` for Rust crates — see the component's own `AGENTS.md`).

## 5. Clarification
- Always refuse a task or ask for clarification if the requirements or context are unclear.

## 6. Code Quality
- Write modular code with minimal side effects. Small, pure/honest functions are the goal.

## 7. String Constants and Reuse
- Use string constants (e.g. `const` items) for repeat strings or user-facing literals rather than duplicating them inline.
- Centralize such constants near the module or type they pertain to so they can be updated in one place.

## 8. Prefer Existing Libraries
- Prefer using available, well-maintained libraries (from `crates.io` or the existing dependency tree) over hand-coding equivalent functions.
- Before implementing a utility from scratch, check whether an existing dependency already provides it.

## 9. Cyclomatic Complexity
- Refactor code as required to limit cyclomatic complexity before adding new features on top of it.
- Prefer splitting large functions, extracting helpers, and reducing nesting over introducing additional branches into already-complex code.

## 10. Issue-Fixing Workflow (Test-First)
- When asked to fix an issue (bug, defect, or regression), **develop a failing test first** that reproduces the issue before writing the fix.
- The test must capture the bug's symptom: write it, run it, and confirm it fails for the same reason the issue describes. Only then implement the fix.
- Use the newly added test to validate the fix: it should now pass. Keep the test in the suite as regression coverage — do not delete or weaken it after the fix lands.
- If the issue cannot be reproduced at the unit or integration level (e.g. timing-, UI-, or environment-dependent), document why in the task/PR and add the closest possible deterministic test (snapshot, contract, or integration harness) rather than skipping coverage.
- Sequence: reproduce (failing test) → fix → verify (test passes) → run the component's full quality gate (see the component's own `AGENTS.md`).

## Component-specific rules

Each subdirectory owns its own `AGENTS.md`. Resolve rules by working directory:
when editing files under one of these directories, that directory's `AGENTS.md`
takes precedence for tooling, conventions, and quality gates.

| Directory                    | Scope                                                                                  |
|------------------------------|----------------------------------------------------------------------------------------|
| [`src/desktop/AGENTS.md`](src/desktop/AGENTS.md)               | Rust `fastmd` crate: documentation, egui, quality gate, tool/UI contracts. |
| [`src/android/AGENTS.md`](src/android/AGENTS.md)               | Android Kotlin/Gradle companion app.                                                  |
| [`doc/technical-context/AGENTS.md`](doc/technical-context/AGENTS.md) | Maintenance of architecture documentation.                |
| [`doc/planning/AGENTS.md`](doc/planning/AGENTS.md)             | Planning / design-record documents.                                                   |
| [`src/test/wiki/AGENTS.md`](src/test/wiki/AGENTS.md)           | Test wiki fixtures.                                                                   |


