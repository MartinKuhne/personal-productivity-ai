//! The atomic widgets of the framework (`UIF-ATOM-*`).
//!
//! An atom is the smallest thing in the interface that has a complete
//! interaction state machine: it can rest, be hovered, be pressed, hold
//! focus, be disabled and — where it carries a value — be invalid. §3 of
//! `doc/planning/ui-framework.md` specifies each atom as a matrix of
//! those states, and this subsystem is where those matrices become code.
//!
//! What lives here is the **resolver**, not the renderer. Each atom module
//! exposes a pure function from `(variant, state, palette)` to a resolved
//! style value, which is what makes every cell of the spec's matrices
//! checkable without a window, a renderer or a GPU — the `UIF-VER-001`
//! and `UIF-VER-003` gates are assertions on those values. The iced
//! widget wrappers that turn a resolved style into an `Element` are a
//! separate increment (`INC-07b`) and are deliberately absent.
//!
//! Like [`crate::token`], this module does **not** glob-re-export its
//! children. Atom type names collide across the set — every atom will want
//! to call its own resolved paint a `Style` — so call sites stay
//! qualified: [`button::ButtonStyle`], [`button::resolve_style`].
//!
//! * [`button`] — the four button variants across the full state matrix
//!   (`UIF-ATOM-001`)
//!
//! Atoms read every visual number from [`crate::token`] and never
//! restate it (`UIF-PRIN-001`). The only numeric literals an atom file
//! may carry are the absence of a property — a zero-width ring, a fully
//! transparent fill — and the opacity modifiers the spec itself prints,
//! and in both cases the value is a named constant rather than an inline
//! number.

pub mod button;
