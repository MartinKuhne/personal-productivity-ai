//! Re-export shim — preserves the crate-root path `fastmd::error`
//! while the implementation lives in [`agent::error`].

pub use crate::agent::error;
