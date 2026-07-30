//! Re-export shim — preserves the crate-root path `fastmd::background_task`
//! while the implementation lives in [`app::background_task`].

pub use crate::app::background_task;
