//! Path helpers — virtual path prefix splitting and executable resolution.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

/// Split a virtual path into library name and remainder.
///
/// Strips leading `/` and `./` components, then returns the first normal
/// component as the library name and the rest as the sub-path.
/// Returns `None` when the path is empty or has no library segment.
pub(crate) fn split_library_prefix(vpath: &Path) -> Option<(String, PathBuf)> {
    let mut comps = vpath.components().peekable();
    while let Some(c) = comps.peek() {
        match c {
            std::path::Component::RootDir | std::path::Component::CurDir => {
                comps.next();
            }
            _ => break,
        }
    }
    let lib = match comps.next() {
        Some(std::path::Component::Normal(first)) => first.to_string_lossy().into_owned(),
        _ => return None,
    };
    let rest: PathBuf = comps.collect();
    Some((lib, rest))
}

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
