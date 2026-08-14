//! Virtual File System — domain subsystem owned by `vfs`.

pub mod behaviour;
pub mod virtual_path;

pub use crate::config::ContentLibraryExt;
pub use behaviour::{library_display_label, resolve, resolve_writable, ResolvedVirtualPath};
pub use virtual_path::{VirtualPath, VirtualPathError};
