//! Typst-powered PDF generation engine and export actions.
//!
//! The pure conversion pipeline (Typst template, font caching via
//! `OnceLock`, markdown → Typst translation) lives in the independent
//! `fastmd-pdf` crate (`src/md2pdf`). The app layer retains only the
//! orchestration (`SaveAsPdfJob`, file IO, viewer launch).

#[cfg(feature = "pdf-export")]
pub mod save;

#[cfg(feature = "pdf-export")]
pub use save::{
    SaveAsPdfJob, compile_and_save_pdf, execute_save_as_pdf_blocking, open_pdf_in_viewer,
};
