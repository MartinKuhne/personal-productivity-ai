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
//!
//! All values are `f32` logical pixels in the single root viewport
//! coordinate space of `UIF-WIN-002`: origin `(0, 0)` at the top-left of
//! the Main Window content area, `+x` right, `+y` down. Every spacing
//! number the solver uses — the `2px` cursor offset, the `4px` trigger
//! gap and the `8px` viewport margin — is read from
//! [`crate::token::space`]; nothing in this directory invents a spacing
//! value.

pub mod anchor;
