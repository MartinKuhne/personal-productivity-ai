use std::path::Path;

/// Purpose: Open a file in the system default editor via the OS shell.
/// Inputs: `path` - The file path to open.
/// Outputs: None.
/// Purity: Impure (interacts with OS shell).
/// Preconditions: The file path should exist; missing files may pop a system error dialog.
/// Postconditions: The default application registered for the file type is launched.
pub fn open_in_system_editor(path: &Path) {
    let _ = std::process::Command::new("cmd")
        .args(["/c", "start", "", &path.to_string_lossy()])
        .spawn();
}

/// Purpose: Show a file in the system file explorer with the file selected (Windows) or its parent directory opened (other OS).
/// Inputs: `path` - The file path to reveal.
/// Outputs: None.
/// Purity: Impure (interacts with OS shell).
/// Preconditions: The file path should exist; behavior on missing files is OS-dependent.
/// Postconditions: The OS file explorer is launched; on Windows the file is selected via the `/select,` flag.
pub fn show_in_file_explorer(path: &Path) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("explorer")
            .raw_arg(format!("/select,\"{}\"", path.to_string_lossy()))
            .spawn();
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = std::process::Command::new("explorer")
            .arg(path.as_os_str())
            .spawn();
    }
}
