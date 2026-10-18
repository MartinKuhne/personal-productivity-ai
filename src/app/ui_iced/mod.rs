//! Iced-based UI layer (The Elm Architecture). Lives beside `ui/` until Phase 5 removes the egui UI.
//!
//! Currently a facade shell only (`TASK-004`, `doc/planning/iced.md`): the
//! runtime bootstrap is exercised by the `fastmd_iced` pilot binary
//! (`src/app/bin/fastmd_iced.rs`) while the egui UI remains untouched. The
//! subsystem tree (`theme`, `layout`, `message`, `update`, `view`, `panels`,
//! `tree`, `render`, `agent`) grows module by module in Phases 2..4.

// No public items yet — the pilot keeps its state and view contained in
// `src/app/bin/fastmd_iced.rs` so this facade stays minimal.
