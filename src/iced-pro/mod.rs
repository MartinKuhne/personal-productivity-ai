//! A reusable iced widget library implementing the FastMD UI framework
//! specification.
//!
//! This crate realises the design specification in
//! `doc/planning/ui-framework.md`. Requirement IDs of the form `UIF-*` in that
//! document map to the code in this crate; each module cites the IDs it
//! implements in its doc comments.
//!
//! The core rule is Perceptual Harmony Through Tokenization (`UIF-PRIN-001`):
//! every visual property — colour, elevation, spacing, sizing, border radius,
//! stroke weight and typography — is selected from a closed set of semantic
//! design tokens. Raw numeric literals in individual widget definitions are
//! forbidden; widgets resolve their style through the token modules only.
//!
//! The crate is standalone: the `fastmd` application does not consume it yet.
//! Wiring the app onto this framework is a separate, later project (decision
//! D-3 in `doc/planning/iced-pro-tasks.md`).

pub mod bridge;
pub mod focus;
pub mod geometry;
pub mod state;
pub mod token;
pub mod widget;

#[cfg(test)]
pub mod test_helpers;
