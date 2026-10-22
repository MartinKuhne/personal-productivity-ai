//! Composite framework surfaces — the layered, multi-widget surfaces built on
//! top of the atoms (`UIF-ZORD-*`, `UIF-FMENU-*`, `UIF-CMENU-*`,
//! `UIF-MOD-*`, `UIF-INP-*`, `UIF-MOV-*`).
//!
//! Where `widget::` holds single-control atoms, this subsystem holds whole
//! surfaces: the menu engine, the z-band model and Escape unwinding, the
//! modal and input-dialog hosts, and the movable overlay. Each surface owns
//! its own data model separately from its renderer so the rules stay
//! testable without a window.

pub mod menu;
pub mod zorder;
