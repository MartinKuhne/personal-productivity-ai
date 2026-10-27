//! The state-and-callback vocabulary `UIF-ATOM-000b` (§3.0.4) requires.
//!
//! §3.0.4 is the one section of the spec that is not about paint. It is about
//! **who owns what, and how intent travels back out**, and it exists because the
//! two failure modes it names are the ones that make a declarative UI feel broken:
//! a control that lags because its feedback round-tripped through a background
//! subsystem, and a control that lies because the framework kept its own copy of a
//! value the application already owns.
//!
//! This module is the **vocabulary only**. It defines the types INC-34 onward
//! adopts widget by widget; it does not rewire anything that already draws.
//! Nothing here imports iced, reads a clock, or resolves a colour.
//!
//! # The four pieces
//!
//! | Piece | Spec | File |
//! |---|---|---|
//! | Controlled vs Ephemeral separation | §3.0.4.1 | [`controlled`] |
//! | Three-tier callback lifecycle | §3.0.4.2 | [`tier`], [`snapshot`] |
//! | Pre-Rank pointer capture | §3.0.4.2, `UIF-WIN-003` step 1 | [`pre_rank`] |
//! | `ActionPending` | §3.0.4.3 | [`pending`] |
//! | `ResponsiveBreakpoint` | §3.0.4.4 | [`responsive`] |
//!
//! [`pre_rank`] is the one piece that is not per-control. Every other type
//! here describes a single control's own state; the Pre-Rank ledger records
//! one fact that has to be readable from *outside* whichever control holds it
//! — that a pointer-captured drag is in flight — because `Escape` has to
//! reach that fact before it reaches the surface that holds keyboard focus.
//!
//! # Why there is no trait hierarchy here
//!
//! The obvious shape is a `trait WidgetModel { fn value(); fn checked(); fn
//! selected_ids(); fn validation(); fn busy(); fn disabled(); }` that every
//! widget implements. It is wrong, for three reasons:
//!
//! 1. **No widget has all six fields.** `busy` belongs to action controls,
//!    `selected_ids` to multi-select surfaces, `checked` to booleans. A shared
//!    trait forces every model to carry, and every consumer to null-check,
//!    fields its control does not have.
//! 2. **A trait describes a capability the holder chooses; the spec describes an
//!    ownership boundary the framework imposes.** Ownership boundaries are
//!    carried by *who holds the reference*, not by what the type can answer.
//!    [`Controlled<'a, M>`][controlled::Controlled] is a borrow the widget
//!    cannot write through; that single fact does more work than any method set.
//! 3. **A shared trait is a merge point.** Once `Live<String>` and
//!    `Commit<String>` both implement `Payload<T>`, generic code can erase the
//!    tier — which is exactly the mistake §3.0.4.2 forbids. The three tiers
//!    therefore deliberately share **no** trait.
//!
//! So the foundation is small composable values: newtypes, a linear gesture
//! guard, a gate, and a pure solver.
//!
//! # The clock does not enter here
//!
//! Every type in this module is a function of its arguments alone. Time-dependent
//! ephemeral state already has its house pattern in
//! [`surface::tooltip::hover`](crate::surface::tooltip::hover), where the host
//! reads the clock once per frame and passes a [`TimeMs`](crate::surface::tooltip::hover::TimeMs)
//! in as a value. None of the types below needs time at all — the snapshot is a
//! value, the gate is a latch, and the breakpoint solver is arithmetic — so this
//! module has no clock to forget to inject.

pub mod controlled;
pub mod pending;
pub mod pre_rank;
pub mod responsive;
pub mod snapshot;
pub mod tier;

pub use controlled::Controlled;
pub use controlled::Ephemeral;
pub use pending::ActionPending;
pub use pending::Activation;
pub use pending::ActivationGate;
pub use pending::CancelRefusal;
pub use pending::CancelRequest;
pub use pending::SuppressedReason;
pub use pre_rank::CaptureId;
pub use pre_rank::CaptureStart;
pub use pre_rank::DragCapture;
pub use pre_rank::DragKind;
pub use pre_rank::PreRankCoordinator;
pub use pre_rank::PreRankOutcome;
pub use responsive::Breakpoint;
pub use responsive::LabelMode;
pub use responsive::Measurement;
pub use responsive::ResponsiveLayout;
pub use snapshot::Cancel;
pub use snapshot::Gesture;
pub use snapshot::GestureSlot;
pub use snapshot::GestureStart;
pub use snapshot::Snapshot;
pub use tier::Commit;
pub use tier::Live;
pub use tier::LiveCoalescer;
