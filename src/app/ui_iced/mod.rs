//! Iced-based UI layer (The Elm Architecture). Lives beside `ui/` until Phase 5 removes the egui UI.
//!
//! The runtime bootstrap is exercised by the `fastmd_iced` pilot binary
//! (`src/app/bin/fastmd_iced.rs`) while the egui UI remains untouched. The
//! subsystem tree grows module by module in Phases 2..4 (`doc/planning/
//! iced.md`):
//!
//! - [`fonts`] — `iced::font::load` bootstrap for the FastMD font
//!   collections ([TASK-007]),
//! - [`theme`] — FastMD palette tokens mapped into a custom `iced::Theme`
//!   ([TASK-008], [GUD-003]),
//! - [`persisted`] — `PersistedUiState` save/load behind an injected
//!   storage handler, plus DEC-008 scale composition and window-field
//!   mapping ([TASK-009], DEC-006/[RUST-024]),
//! - [`layout`] — the three-band pane-grid geometry with pure
//!   `crate::ui::PanelLayout`-bound resize reductions ([TASK-010], UI-001),
//! - [`message`] — the top-level `Message` enum, one variant per logical
//!   interaction channel, with `UserCommand` as the user-intent channel
//!   ([TASK-021], [FMD-068]).
//!
//! `update`, `view`, `panels`, `tree`, `render` and `agent` follow in
//! Phase 3/4.

pub mod fonts;
pub mod layout;
pub mod message;
pub mod persisted;
pub mod theme;
