# AI Agent Instructions — `doc/technical-context/`

This directory holds the **authoritative** technical reference docs:
`ARCHITECTURE_C4.md`, `RUST.md`, and `SPEC.md`. Repo-root `AGENTS.md` provides
shared principles; this file adds doc-maintenance rules.

## 1. Docs reflect reality, not aspiration
- `ARCHITECTURE_C4.md` must describe the **current** code. No "Target", "Planned",
  or "Key Improvements" sections — those belong in `doc/planning/`.
- When module boundaries, public API, or container shape change in
  `src/desktop/`, update `ARCHITECTURE_C4.md` in the same PR.
- Every container/component in the C4 diagrams must map to real files/modules
  under `src/desktop/src/`. Cite the file path in the diagram node text where
  feasible (e.g. `agent/manager.rs (278)`).

## 2. SPEC.md
- EARS-formatted requirements (`REQ-xxx`). Each requirement has a stable ID;
  never renumber. Supersede with a new REQ and a "supersedes REQ-yyy" note.
- Add a REQ *before* implementing the behaviour it covers, so PRs can cite it.
- Keep the tool table in sync with `src/desktop/Tools.md` and the actual
  `tools::registry` registrations.

## 3. RUST.md
- Captures Rust-specific design notes (memory model, async runtime, error
- strategy, etc.). Update when a foundational crate or runtime choice changes
  (e.g. `tokio` vs `std::thread`, allocator, TLS provider).

## 4. Mermaid style
- Use `C4Context`, `C4Container`, `C4Component` from the C4-PlantUML subset
  supported by GitHub's mermaid renderer.
- One Level-1 Context diagram, one Level-2 Container diagram, then Level-3
  Component diagrams per subsystem — do not collapse everything into one
  giant diagram.
- Keep node labels short; move detail into the surrounding prose.

## 5. Quality gate
- Markdown renders cleanly on GitHub (no broken tables, no dangling links).
- `REQ-xxx` citations resolve to entries in `SPEC.md`.
- `ARCHITECTURE_C4.md`'s "Folder Layout" block matches the output of
  `Get-ChildItem -Recurse src/desktop/src`.
