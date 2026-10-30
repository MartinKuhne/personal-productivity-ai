//! Composite framework surfaces — the layered, multi-widget surfaces built on
//! top of the atoms (`UIF-ZORD-*`, `UIF-FMENU-*`, `UIF-CMENU-*`,
//! `UIF-MOD-*`, `UIF-INP-*`, `UIF-MOV-*`).
//!
//! Where `widget::` holds single-control atoms, this subsystem holds whole
//! surfaces: the menu engine, the z-band model and Escape unwinding, the
//! modal and input-dialog hosts, and the movable overlay. Each surface owns
//! its own data model separately from its renderer so the rules stay
//! testable without a window.
//!
//! Three modules carry the `UIF-ATOM-000b` binding foundation *for* surfaces
//! rather than being surfaces themselves, because each is the half of a
//! surface that has to remember something: [`dialog_button`] is a dialog
//! footer's activation engine, [`modal_tier`] is a modal's pending-action
//! half, and [`drag_tier`] is one movable-overlay drag's tier transcript.
//! The menu's equivalent lives inside [`menu`] as
//! [`row_action`](menu::row_action).

pub mod dialog_button;
pub mod drag_tier;
pub mod input_dialog;
pub mod menu;
pub mod modal;
pub mod modal_tier;
pub mod movable;
pub mod popover;
pub mod tooltip;
pub mod zorder;
