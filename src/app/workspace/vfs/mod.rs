//! Virtual File System — domain subsystem owned by `app/vfs/`.

pub use crate::agent::vfs::behaviour;
pub use crate::agent::vfs::virtual_path;
pub use crate::agent::vfs::{
    ContentLibraryExt, VirtualPath, VirtualPathError, library_display_label, resolve,
    resolve_writable,
};
