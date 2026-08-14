use std::borrow::Cow;

/// Resolve an executable name to a platform-appropriate invocable string.
pub fn resolve_executable_path(command: &str) -> Cow<'_, str> {
    #[cfg(windows)]
    {
        if !command.contains('.')
            && !command.contains('/')
            && !command.contains('\\')
            && let Ok(path_var) = std::env::var("PATH")
        {
            let pathext =
                std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
            let exts: Vec<&str> = pathext.split(';').filter(|s| !s.is_empty()).collect();

            for dir in std::env::split_paths(&path_var) {
                for ext in &exts {
                    let candidate = dir.join(format!("{}{}", command, ext));
                    if candidate.is_file() {
                        return Cow::Owned(candidate.to_string_lossy().into_owned());
                    }
                }
            }
        }
    }
    Cow::Borrowed(command)
}
