//! Re-export shim — preserves the crate-root path `fastmd::editor_egui`
//! while the implementation lives in [`ui::editor_egui`].

pub use crate::ui::editor_egui;
