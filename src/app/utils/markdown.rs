//! Retained front-matter parsing tests.
//!
//! The former `crate::utils::markdown` re-export shim was removed so that
//! `parse_front_matter` keeps a single public path at
//! `crate::markdown::parse_front_matter`. These tests are kept against the
//! canonical path.
//!
//! Unit tests live in the sibling `markdown_tests.rs` sidecar.

#[cfg(test)]
#[path = "markdown_tests.rs"]
mod tests;
