//! User-visible description strings for the YAML front-matter tool family.

// --- read_yaml_header ---

pub const READ_YAML_HEADER_DESCRIPTION: &str = "Parse a YAML header from a markdown file and return its content representation. Tip: Use this to read a document's summary before reading the full file if you are not sure the full contents are needed, to protect context.";

// --- write_yaml_header ---

pub const WRITE_YAML_HEADER_DESCRIPTION: &str =
    "Write or update data in a YAML header to a markdown file.";
