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
