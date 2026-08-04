# AI Agent Instructions — `.agents/` (Speckit skills)

This directory hosts the **speckit** skill workflow (`skills/speckit-*`).
Repo-root `AGENTS.md` provides shared principles; this file pins when to use
the skills and how they relate to the rest of the repo.

## 1. Skills are the canonical workflow
- For any feature/spec/plan/tasks work, prefer the speckit skills over ad-hoc
  edits: `speckit-specify` → `speckit-clarify` → `speckit-plan` →
  `speckit-tasks` → `speckit-implement` → `speckit-converge` →
  `speckit-analyze`.
- Each skill's `SKILL.md` is the source of truth for its own steps; do not
  reimplement the workflow inline in code or docs.

## 2. Where artifacts live
- `spec.md`, `plan.md`, `tasks.md` land in `.specify/` per speckit convention.
- Requirements that graduate into the product must be mirrored into
  `doc/technical-context/SPEC.md` (the authoritative REQ-xxx list); speckit
  artifacts are working state, not the published spec.

## 3. Quality gate
- Skills' `SKILL.md` files render cleanly on GitHub.
- After running a skill, follow the component `AGENTS.md` for the touched
  directory (e.g. `src/desktop/AGENTS.md` quality gate).
