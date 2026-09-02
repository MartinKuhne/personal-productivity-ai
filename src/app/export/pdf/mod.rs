//! Typst-powered PDF generation engine and export actions.
//!
//! This module is now a facade over the independent `fastmd-pdf` crate
//! (`src/md2pdf`). The pure conversion pipeline (Typst template, font
//! caching via `OnceLock`, markdown → Typst translation) lives in that
//! crate so it can be used without pulling in the desktop app. The app
//! layer retains only the orchestration (`SaveAsPdfJob`, file IO, viewer
//! launch, background-bus logging).

#[cfg(feature = "pdf-export")]
pub mod save;

#[cfg(feature = "pdf-export")]
pub mod typst_translator;

#[cfg(feature = "pdf-export")]
pub use fastmd_pdf::{TYPST_THREAD_STACK_SIZE, compile_markdown_to_pdf, generate};

#[cfg(feature = "pdf-export")]
pub use save::{
    SaveAsPdfJob, compile_and_save_pdf, execute_save_as_pdf_blocking, open_pdf_in_viewer,
};
