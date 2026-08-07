//! egui widgets. The functions in this module are pure renderers: they
//! receive the data they need as parameters and call into egui's immediate
//! mode API. All mutable state lives on [`crate::app::FastMdApp`].

pub mod file_tree;
pub mod file_viewer;
pub mod sign_in;
