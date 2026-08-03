//! Text encoding helpers — robust decoding for legacy single-byte
//! encodings (ISO-8859-1 / Windows-1252) with UTF-8 priority.

use encoding_rs::{UTF_8, WINDOWS_1252};
use std::fs;
use std::io;
use std::path::Path;

/// Read a text file with UTF-8 priority, falling back to
/// Windows-1252 (superset of ISO-8859-1) when the content is not
/// valid UTF-8. Returns the decoded string.
///
/// The heuristic: try strict UTF-8 decode; if that produces errors,
/// decode with Windows-1252. This handles the common case of German
/// and other European legacy text files (e.g. Usenet posts from the
/// 1990s/2000s) which declare `charset=iso-8859-1` but use byte
/// values like 0xFC (`ü`), 0xDF (`ß`), 0xE4 (`ä`), 0xF6 (`ö`),
/// 0xDC (`Ü`) — all preserved correctly by Windows-1252.
///
/// # Errors
/// Returns an `io::Error` if the file cannot be read.
pub fn read_text_file(path: &Path) -> io::Result<String> {
    let bytes = fs::read(path)?;
    let (cow, _, had_errors) = UTF_8.decode(&bytes);
    if !had_errors {
        return Ok(cow.into_owned());
    }
    let (cow, _, _) = WINDOWS_1252.decode(&bytes);
    Ok(cow.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Valid UTF-8 (including non-ASCII) passes through unchanged.
    #[test]
    fn test_utf8_passthrough() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "Grüße aus Berlin — café").unwrap();
        let content = read_text_file(f.path()).unwrap();
        assert_eq!(content, "Grüße aus Berlin — café\n");
    }
    /// Latin-1 / Windows-1252 encoded bytes decode to correct Unicode.
    #[test]
    fn test_latin1_fallback() {
        // Bytes for "Asthma: Foradil — 0xFC=ü, 0xDF=ß, 0xE4=ä, 0xF6=ö, 0xDC=Ü"
        let bytes: [u8; 25] = [
            b'A', b's', b't', b'h', b'm', b'a', b':', b' ', b'F', b'o', b'r', b'a', b'd', b'i',
            b'l', b' ', 0xFC, b' ', 0xDF, b' ', 0xE4, b' ', 0xF6, b' ', 0xDC,
        ];
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(&bytes).unwrap();
        let content = read_text_file(f.path()).unwrap();
        assert_eq!(content, "Asthma: Foradil ü ß ä ö Ü");
    }
    /// Windows-1252 smart quotes (0x93/0x94) decode correctly.
    #[test]
    fn test_windows1252_smart_quotes() {
        // 0x93 = U+201C ("), 0x94 = U+201D (")
        let bytes: [u8; 7] = [0x93, b'h', b'e', b'l', b'l', b'o', 0x94];
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(&bytes).unwrap();
        let content = read_text_file(f.path()).unwrap();
        assert_eq!(content, "“hello”");
    }

    /// Empty file returns empty string.
    #[test]
    fn test_empty_file() {
        let f = NamedTempFile::new().unwrap();
        let content = read_text_file(f.path()).unwrap();
        assert_eq!(content, "");
    }

    /// Non-existent file returns io::Error.
    #[test]
    fn test_nonexistent_file() {
        let result = read_text_file(Path::new("/nonexistent/path.md"));
        assert!(result.is_err());
    }
}
