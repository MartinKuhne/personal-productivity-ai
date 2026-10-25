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

pub mod a11y;
pub mod binding;
pub mod bridge;
pub mod composite;
pub mod focus;
pub mod gallery;
pub mod gallery_badge;
pub mod gallery_breadcrumb;
pub mod gallery_chip;
pub mod gallery_combo;
pub mod gallery_composite;
pub mod gallery_hyperlink;
pub mod gallery_popover;
pub mod gallery_progress;
pub mod gallery_range;
pub mod gallery_secret;
pub mod gallery_segmented;
pub mod gallery_split_button;
pub mod gallery_suggest;
pub mod gallery_table;
pub mod gallery_toggle;
pub mod gallery_tree;
pub mod geometry;
pub mod layout;
pub mod microcopy;
pub mod spec_coverage;
pub mod state;
pub mod surface;
pub mod token;
pub mod token_lint;
pub mod validation;
pub mod widget;

#[cfg(test)]
pub mod test_helpers;
