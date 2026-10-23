//! The atomic widgets of the framework (`UIF-ATOM-*`).
//!
//! An atom is the smallest thing in the interface that has a complete
//! interaction state machine: it can rest, be hovered, be pressed, hold
//! focus, be disabled and — where it carries a value — be invalid. §3 of
//! `doc/planning/ui-framework.md` specifies each atom as a matrix of
//! those states, and this subsystem is where those matrices become code.
//!
//! Each atom here has two halves, kept deliberately separate. The
//! **resolver** is a pure function from `(variant, state, palette)` to a
//! resolved style value — that is what makes every cell of the spec's
//! matrices checkable without a window, a renderer or a GPU, and it is
//! what the `UIF-VER-001` and `UIF-VER-003` gates assert on. The
//! **widget** is the iced `Element` that paints that value and drives the
//! state from real pointer and keyboard events.
//!
//! Keeping them apart is not ceremony. A widget whose colour logic lives
//! inside its `draw` cannot be audited for contrast; a resolver with no
//! widget cannot be seen. Both halves exist so the audit and the artifact
//! are each checkable on their own terms.
//!
//! Like [`crate::token`], this module does **not** glob-re-export its
//! children. Atom type names collide across the set — every atom will want
//! to call its own resolved paint a `Style` — so call sites stay
//! qualified: [`button::ButtonStyle`], [`button::resolve_style`],
//! [`focus::resolve_ring_color`].
//!
//! * [`button`] — the four button variants across the full state matrix
//!   (`UIF-ATOM-001`)
//! * [`button_widget`] — the rendering, keyboard-focusable button that
//!   composes those resolvers into a real iced widget, the first atom in
//!   the framework to paint (`UIF-ATOM-001`, `UIF-PRIN-005`)
//! * [`button_model`] — the declarative `ButtonModel` a host owns and hands
//!   to the button each frame: label, icons, variant, declared density, busy
//!   and cancellation, and the tooltip a collapsed rung owes
//!   (`UIF-ATOM-001a`)
//! * [`button_controller`] — the button's activation engine: the linear
//!   gesture slot, the busy / double-activation gate and the three emission
//!   tiers a button can put on the wire (`UIF-ATOM-001a`, §3.0.4.3)
//! * [`focus`] — the adaptive keyboard focus rule every widget with a
//!   focus indicator resolves through, rather than re-deriving it per
//!   variant: one colour rule, two placements — the `2px` outer ring for
//!   buttons and inputs and the `1px` inset outline for menu and tree
//!   rows (`UIF-TOK-006` rule 2, `UIF-ATOM-000` §3.0.2, decision **D-6**)
//! * [`focusable`] — the wrapper that registers a widget with iced's
//!   focus router, the mechanism decision **D-7** rests on and the
//!   reason a custom widget can hold keyboard focus at all
//!   (`UIF-PRIN-005`, gap **G-10**)
//! * [`text_input`] — the text entry, search field and dropdown trigger
//!   across the five-state matrix of §3.2, including the trailing slot
//!   that is reserved whether or not anything is in it
//!   (`UIF-ATOM-002`, `UIF-PRIN-003`)
//!
//! Atoms read every visual number from [`crate::token`] and never
//! restate it (`UIF-PRIN-001`). The only numeric literals an atom file
//! may carry are the absence of a property — a zero-width ring, a fully
//! transparent fill — and the opacity modifiers the spec itself prints,
//! and in both cases the value is a named constant rather than an inline
//! number.

pub mod button;
pub mod button_controller;
pub mod button_model;
pub mod button_widget;
pub mod focus;
pub mod focusable;
pub mod text_input;
