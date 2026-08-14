//! Agent internal utilities.

pub mod encoding;
pub mod markdown;
pub mod path;
pub mod tags;
pub mod uuid;

pub use encoding::read_text_file;
pub use markdown::{FrontMatter, parse_front_matter};
pub use path::resolve_executable_path;
pub use tags::extract_tags_from_file;
pub use uuid::{FixedUuidGenerator, SystemUuidGenerator, UuidGenerator};
