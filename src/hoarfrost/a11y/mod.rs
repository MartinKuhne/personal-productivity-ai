//! The accessibility projection and the composite focus boundary
//! (`UIF-PRIN-008`).
//!
//! Two halves of one pillar, and they belong together because they are
//! both about what a widget *means* as opposed to what it looks like.
//!
//! * [`contract`] — [`A11yContract`], the pure nine-field semantic
//!   descriptor §1.1 requires every widget model to expose, plus the
//!   closed [`A11yRole`] enum that keeps `role` from being a typo that
//!   compiles.
//! * [`project`] — the projections that read a real model and fill in
//!   only the fields that model genuinely carries.
//! * [`tab_stop`] — [`CompositeBoundary`], the declared single-`Tab`
//!   stop, and the bounded-wraps / spatial-clamps rule that goes with
//!   it.
//!
//! # Why a subsystem rather than a field on each widget
//!
//! The alternative was giving every model nine more fields. That is nine
//! fields per model that no model actually needs — `ButtonModel` has no
//! use for `expanded`, and storing it there means the model can drift
//! out of agreement with itself about whether it is expanded.
//!
//! A projection cannot drift, because it is computed from the state the
//! model already has. It is also additive: a widget that already exists
//! gains a contract without changing its own shape, and a headless
//! selector can walk a whole screen's worth of semantics without a
//! renderer, a font or a layout pass.
//!
//! # What this makes possible that geometry never could
//!
//! §1.1's clause is a prohibition — *"never rely on pixel scraping"* —
//! and a prohibition needs a positive substitute to be actionable. This
//! is it. A test that asserts "the third row is selected" by sampling
//! the accent colour at a coordinate breaks on a palette change, a
//! density change and a subpixel rounding difference. The same test
//! against a projected contract breaks only when the selection actually
//! changes.
//!
//! # The honest limit
//!
//! Nothing in this module *enforces* that a widget exposes a contract.
//! A widget that never calls a projection still renders perfectly. The
//! projections exist so that framework composites and host plans can be
//! checked against them, and the enforcement point — every composite
//! publishing its contract on the way out — is the residual named in
//! the ledger.
//!
//! Unit tests live in the sibling `*_tests.rs` sidecars of each file.

pub mod contract;
pub mod project;
pub mod tab_stop;

pub use contract::A11yContract;
pub use contract::A11yRole;
pub use contract::A11yValue;
pub use contract::Checked;
pub use project::project_button;
pub use project::project_checkbox;
pub use project::project_chip;
pub use project::project_disclosure;
pub use project::project_menu_row;
pub use project::project_menu_row_open;
pub use project::project_progress;
pub use project::project_radio_group;
pub use project::project_radio_option;
pub use project::project_segment;
pub use project::project_segmented;
pub use project::project_text_input;
pub use project::project_tree_row;
pub use tab_stop::CompositeBoundary;
pub use tab_stop::CompositeCategory;
pub use tab_stop::EdgeBehaviour;
pub use tab_stop::TabStopViolation;
pub use tab_stop::check_composite;
pub use tab_stop::check_roster;
pub use tab_stop::total_members;
pub use tab_stop::total_tab_stops;
