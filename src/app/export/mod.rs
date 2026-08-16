//! Export and print subsystem — HTML printing and Typst-powered PDF generation.

pub mod print;

#[cfg(feature = "pdf-export")]
pub mod pdf;

pub use print::{PrintJob, cleanup_temp_files, execute_print_blocking};

#[cfg(feature = "pdf-export")]
pub use pdf::{
    SaveAsPdfJob, TYPST_THREAD_STACK_SIZE, compile_and_save_pdf, compile_markdown_to_pdf,
    execute_save_as_pdf_blocking, open_pdf_in_viewer, typst_translator,
};
