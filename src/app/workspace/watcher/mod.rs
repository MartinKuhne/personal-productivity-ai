//! File-watcher plumbing — the [`Workspace`] event-sink projection and the
//! `notify`-based filesystem watcher.
//!
//! All items in this module are framework-free; they are unit-tested without
//! driving the UI. The UI layer treats them as pure state-holding types.
//!
//! The bus transport and event payload types live in [`crate::bus`].

pub(crate) mod file_watcher;
pub(crate) mod workspace;

pub use file_watcher::FileWatcher;
pub use workspace::Workspace;
