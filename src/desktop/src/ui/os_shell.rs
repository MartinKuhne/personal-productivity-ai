//! Platform-independent helpers — open a file in the system default editor and reveal a file in the OS file explorer.

use std::path::Path;

/// Open a file in the system default editor.
pub fn open_in_system_editor(path: &Path) {
    let _ = opener::open(path);
}

/// Show a file in the system file explorer with the file selected.
pub fn show_in_file_explorer(path: &Path) {
    let _ = opener::reveal(path);
}
