use encoding_rs::{UTF_8, WINDOWS_1252};
use std::fs;
use std::io;
use std::path::Path;

/// Read a text file with UTF-8 priority, falling back to
/// Windows-1252 (superset of ISO-8859-1) when the content is not
/// valid UTF-8. Returns the decoded string.
pub fn read_text_file(path: &Path) -> io::Result<String> {
    let bytes = fs::read(path)?;
    let (cow, _, had_errors) = UTF_8.decode(&bytes);
    if !had_errors {
        return Ok(cow.into_owned());
    }
    let (cow, _, _) = WINDOWS_1252.decode(&bytes);
    Ok(cow.into_owned())
}
