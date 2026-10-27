//! Pure boundary solvers for the framework's floating surfaces
//! (`UIF-VER-002`).
//!
//! Every card this framework floats — context menus, fixed navbar menus,
//! submenu cascades, input dialogs — has to land inside a window that may
//! be anywhere down to the `640 × 480` minimum viewport of
//! `UIF-WIN-001`. Deciding where it lands is arithmetic, not rendering,
//! so it lives here as pure functions over plain values: no widget, no
//! renderer, no mutable state, no I/O.
//!
//! * [`anchor`] — the placement solver: an [`anchor::Anchor`] (pointer,
//!   control rectangle or cursor-proximal point), a card size and a
//!   viewport resolve to an [`anchor::Placement`] that carries the
//!   top-left origin *and* the reasons it landed there — quadrant flips,
//!   the roomier-side scrollable fallback and viewport clamping
//!   (`UIF-CMENU-001`, `UIF-FMENU-001`, `UIF-INP-001`).
//! * [`clamp`] — the movable-overlay reachability solver: a dragged or
//!   resized card is clamped so its **title bar**, not its body, can never
//!   be lost off-screen, plus the min/max size policy a resize is clamped
//!   against (`UIF-MOV-002`).
//! * [`pane_solver`] — pure boundary solvers for workspace pane dimensions:
//!   clamping and auto-fit to visible item labels (`UIF-PANE-002`).
//! * [`window_geometry`] — window dimensions schema, `UiScale` step math,
//!   window layout solving preserving central workspace floor, and the
//!   `WindowStorage` seam (`UIF-WIN-001`).
//!
//! All values are `f32` logical pixels in the single root viewport
//! coordinate space of `UIF-WIN-002`: origin `(0, 0)` at the top-left of
//! the Main Window content area, `+x` right, `+y` down. Every spacing
//! number the solver uses — the `2px` cursor offset, the `4px` trigger
//! gap and the `8px` viewport margin — is read from
//! [`crate::token::space`]; nothing in this directory invents a spacing
//! value.

pub mod anchor;
pub mod clamp;
pub mod pane_solver;
pub mod window_geometry;

pub use pane_solver::{
    LEFT_PANE_MIN_W, RIGHT_PANE_MIN_W, auto_fit_pane_width, solve_clamped_pane_width,
};
pub use window_geometry::{
    CENTER_PANE_MIN_W, MemoryWindowStorage, NoopWindowStorage, SolvedWindowLayout, UiScale,
    WINDOW_DEFAULT_HEIGHT, WINDOW_DEFAULT_WIDTH, WINDOW_MIN_HEIGHT, WINDOW_MIN_WIDTH,
    WindowGeometry, WindowStorage, solve_window_layout,
};
