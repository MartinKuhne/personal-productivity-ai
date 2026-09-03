//! Export and print subsystem — HTML printing and Typst-powered PDF generation.

pub mod print;

#[cfg(feature = "pdf-export")]
pub mod pdf;

pub use print::{PrintJob, cleanup_temp_files, execute_print_blocking};

#[cfg(feature = "pdf-export")]
pub use pdf::{
    SaveAsPdfJob, compile_and_save_pdf, execute_save_as_pdf_blocking, open_pdf_in_viewer,
};
