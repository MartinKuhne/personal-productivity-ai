//! Re-export shim — preserves the crate-root path `fastmd::document`
//! while the implementation lives in [`app::document`].

pub use crate::app::document;
