# AI Agent Instructions — `doc/technical-context/`

This directory holds the **authoritative** technical reference 

## 1. Docs reflect reality, not aspiration
- `ARCHITECTURE_C4.md` MUST describe the **current** code.
- When module boundaries, public API, or container shape change in
  `src/desktop/`, update `ARCHITECTURE_C4.md` in the same PR.
- Every container/component in the C4 diagrams must map to real directories or modules under `src/desktop/src/`.

## 3. RUST.md
- Captures Rust-specific design notes (memory model, async runtime, error
- strategy, etc.). Update when a foundational crate or runtime choice changes
  (e.g. `tokio` vs `std::thread`, allocator, TLS provider).

## 4. Mermaid style
- Use `C4Context`, `C4Container`, `C4Component` from the C4-PlantUML subset
  supported by GitHub's mermaid renderer.
- You SHOULD maintain one Level-1 Context diagram, one Level-2 Container diagram, then Level-3
  Component diagrams per subsystem
- Keep node labels short; move detail into the surrounding prose.

## 5. Quality gate
- Markdown renders cleanly on GitHub (no broken tables, no dangling links).
- `ARCHITECTURE_C4.md`'s "Folder Layout" block matches the output of
  `Get-ChildItem -Recurse src/desktop/src`.
