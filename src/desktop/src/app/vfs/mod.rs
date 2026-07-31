//! Virtual File System — domain subsystem owned by `app/vfs/`.
//!
//! This module groups the three pieces of VFS behaviour that today are
//! scattered across `config/`, `tools/`, and the root `SPEC.md`:
//!
//! - [`virtual_path::VirtualPath`] — parses library-prefixed paths
//!   (e.g. `Wiki/Career/SQA.md`), rejects `..` traversal.
//! - [`library`] — content-library behaviour and the real-path →
//!   virtual-label reverse mapping.
//! - [`resolve::resolve`] — the tool-facing entry point that turns a
//!   virtual path into an absolute filesystem path, enforcing the
//!   read-only / write rules.
//!
//! The spec for this module lives in [`SPEC.md`](../vfs/SPEC.md) (VFS-001..VFS-009).

pub mod library;
pub mod resolve;
pub mod virtual_path;

pub use library::{ContentLibraryExt, library_display_label};
pub use virtual_path::{VirtualPath, VirtualPathError};
