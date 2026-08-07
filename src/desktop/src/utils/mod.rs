//! Shared utility functions for markdown parsing, path validation, and tag extraction.

pub mod encoding;
pub mod markdown;
pub mod path;
pub mod tags;
pub mod clock;

pub use encoding::read_text_file;
pub use path::has_pdf_backing;
pub use tags::extract_tags_from_file;
