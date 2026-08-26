---
description: "Task list for System Library Skills (VFS-120..123)"
---

# Tasks: System Library Skills (VFS-120..123)

**Input**: Feature specification from `/specs/005-system-skills/spec.md`

**Tests**: Test-driven development with unit tests in sidecars and quality gate checks between steps.

**Organization**: Tasks are ordered sequentially by user story (US1 -> US2 -> US3).

## Path Conventions
- Workspace root: `C:\Users\mkuhn\src\ppai`
- Quality gate command:
  ```powershell
  cargo check --quiet
  cargo nextest run --status-level fail --show-progress none
  cargo clippy -- -D warnings
  cargo fmt --check
  cargo doc --no-deps --quiet
  ```

---

## Phase 1: User Story 1 — System Skills Structure & Note Skills Context Menu (Priority: P1)

- [x] T001 [US1] Add `SkillFile` struct and helper methods (`ensure_skills_dirs()`, `ensure_skills_dirs_at()`, `get_skills_note_path()`, `get_skills_folder_path()`, `get_skills_batch_path()`, `list_skill_files()`, `list_note_skills()`, `list_folder_skills()`, `list_batch_skills()`) to `src/app/config/config.rs` and update `ensure_system_library_present()` to auto-create all skill directories (VFS-120).
- [x] T002 [US1] Add unit tests for skill directory creation, subpath resolution, and skill file enumeration in `src/app/config/config_tests.rs`.
- [x] T003 [US1] Render `Skills/Note` options in directory tree file context menu (`src/app/ui/tree/render.rs`) and open tab context menu (`src/app/ui/panels/center.rs`), executing agent prompt with skill content and right-clicked note as active file (VFS-121).
- [x] T004 [US1] Add unit tests for Note skill menu generation and prompt dispatch in `src/app/ui/tree/render_tests.rs`.

---

## Phase 2: User Story 2 — Folder Skills in Directory Tree Context Menu (Priority: P2)

- [x] T005 [US2] Render `Skills/Folder` options in directory tree folder context menu (`src/app/ui/tree/render.rs`), executing agent prompt with skill content and right-clicked folder as active directory (VFS-122).
- [x] T006 [US2] Add unit tests for Folder skill menu generation and prompt dispatch in `src/app/ui/tree/render_tests.rs`.

---

## Phase 3: User Story 3 — Batch Skills in Batch Dialog (Priority: P3)

- [x] T007 [US3] Include `Skills/Batch` files in prompt discovery/resolution (`src/app/agent/batch/prompts.rs` and `src/app/ui/batch_dialog.rs`) so batch skills are available as options in the Batch prompt processing dialog (VFS-123).
- [x] T008 [US3] Add unit tests for batch skill prompt discovery in `src/app/agent/batch/prompts.rs` and batch dialog tests in `src/app/ui/batch_dialog_tests.rs`.

---

## Phase 4: Quality Gate & Validation

- [x] T009 Run full quality gate (`cargo check`, `cargo nextest run`, `cargo clippy -- -D warnings`, `cargo fmt --check`, `cargo doc --no-deps --quiet`) and verify all tests pass.
