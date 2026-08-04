//! Virtual File System — domain subsystem owned by `app/vfs/`.
//!
//! This module groups the three pieces of VFS behaviour that today are
//! scattered across `config/`, `tools/`, and the root `SPEC.md`:
//!
//! - [`virtual_path::VirtualPath`] — parses library-prefixed paths
//!   (e.g. `Wiki/Career/SQA.md`), rejects `..` traversal.
//! - [`behaviour`] — content-library behaviour, the real-path →
//!   virtual-label reverse mapping, and the tool-facing `resolve` /
//!   `resolve_writable` entry points that turn a virtual path into an
//!   absolute filesystem path (with traversal protection and read-only
//!   enforcement).
//!
//! The spec for this module lives in [`SPEC.md`](../vfs/SPEC.md) (VFS-001..VFS-009).

pub mod behaviour;
pub mod virtual_path;

pub use behaviour::{ContentLibraryExt, library_display_label, resolve, resolve_writable};
pub use virtual_path::{VirtualPath, VirtualPathError};
