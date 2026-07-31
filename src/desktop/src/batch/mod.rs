//! Re-export shim — preserves the crate-root path `fastmd::batch`
//! while the implementation lives in [`app::batch`].

pub use crate::app::batch;
