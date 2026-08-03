//! Shared utility functions for markdown parsing, path validation, and tag extraction.

pub mod markdown;
pub mod path;
pub mod tags;

pub use tags::extract_tags_from_file;

pub use path::has_pdf_backing;
