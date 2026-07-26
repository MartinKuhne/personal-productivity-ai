# AI Agent Instructions

When working on this codebase, all AI agents must adhere strictly to the following guidelines:

## 1. Small Iterations
- Work in small, manageable iterations. 
- Do not attempt sweeping, massive refactors in a single pass. 
- Break tasks into logical steps, implement a change, and verify it before moving to the next step.

## 2. Test-Driven Changes
- Create unit and functional tests for any new features or changes.
- Ensure that you prove your code works through tests, rather than assumptions.

## 3. Keep Tests Updated
- Always update relevant tests after making code changes or bug fixes.
- If a bug is fixed, a test must be added or updated to cover the regression and prevent it from recurring.

## 4. Warnings and Compilation
- **Fix all warnings:** You must fix all new and existing compile or test warnings before considering a task "done."
- Never leave dangling unused variables, imports, or future-incompatibilities if they are within your control to fix.
- Ensure `cargo check` and `cargo test` run perfectly clean.

## 5. Clarification
- Always refuse a task or ask for clarification if the requirements or context are unclear.

## 6. Code Quality
- Write modular code with minimal side effects.

## 7. Documentation
- Every module must have a `//!` module-level doc comment.
- Start with a concise one-sentence summary, then add detail if needed.
- The first line (before any blank line) is used in search results and overviews — keep it short and descriptive.
- Every `pub` item (struct, enum, function, trait, type alias, const) must have a `///` doc comment.
- Include examples in doc comments where they clarify usage.
- Run `cargo doc --no-deps` to verify documentation builds without warnings.

## 8. String Constants and Reuse
- Use string constants (e.g. `const` items) for repeat strings or user-facing literals rather than duplicating them inline.
- Centralize such constants near the module or type they pertain to so they can be updated in one place.

## 9. Prefer Existing Libraries
- Prefer using available, well-maintained libraries (from `crates.io` or the existing dependency tree) over hand-coding equivalent functions.
- Before implementing a utility from scratch, check whether an existing dependency already provides it.

## 10. egui Best Practices
- Follow egui's recommended patterns and idioms when building UI.
- Follow egui testing best practices (e.g. using `egui::__run_test_ctx` / `State::test_ctx` style harnesses as appropriate) for deterministic UI tests.
- Keep `update` methods side-effect free where possible and avoid storing unnecessary state in `egui::Id`s.

## 11. Cyclomatic Complexity
- Refactor code as required to limit cyclomatic complexity before adding new features on top of it.
- Prefer splitting large functions, extracting helpers, and reducing nesting over introducing additional branches into already-complex code.

## 12. Quality Gate (Rust)

Before marking any task as complete, run the following and ensure they all pass cleanly:
- `cargo check` — no errors or warnings
- `cargo test` — all tests pass
- `cargo clippy -- -D warnings` — no lint warnings (deny all)
- `cargo fmt --check` — code is properly formatted
- `cargo doc --no-deps --quiet` — documentation builds without warnings
