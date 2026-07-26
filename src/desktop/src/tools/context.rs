//! Tool context — provides tools with access to `AppConfig` and the file event bus, plus safe virtual-path resolution.

use crate::config::{AppConfig, VirtualPath, VirtualPathError};
use crate::file_events::{Bus, FileEvent, FileEventKind, FileEventProducer};
use std::path::{Path, PathBuf};

pub struct ToolContext<'a> {
    pub config: &'a AppConfig,
    pub file_event_bus: &'a Bus<FileEvent>,
}

impl<'a> ToolContext<'a> {
    pub fn new(config: &'a AppConfig, file_event_bus: &'a Bus<FileEvent>) -> Self {
        Self {
            config,
            file_event_bus,
        }
    }

    pub fn resolve_virtual_path(
        &self,
        vpath: &str,
        allow_write: bool,
    ) -> Result<Option<(PathBuf, bool)>, String> {
        let normalized = vpath.replace('\\', "/");
        let trimmed = normalized.trim_matches('/');
        if trimmed.is_empty() || trimmed == "." {
            return Ok(None);
        }

        // Parse the leading/trailing-slash-stripped form so that inputs like
        // `/Wiki/Career/SQA.md` resolve to `library="Wiki", sub="Career/SQA.md"`
        // instead of being misrejected as an empty library name. The raw `vpath`
        // is retained only for the error message below.
        let vp = match VirtualPath::parse(trimmed) {
            Ok(vp) => vp,
            Err(VirtualPathError::InvalidFormat(_)) => {
                // Library-only inputs (no sub-path, e.g. "Wiki" or "/Wiki")
                // land here; treat the entire trimmed string as a library name.
                let lib = self
                    .config
                    .content_libraries
                    .iter()
                    .find(|l| l.name == trimmed);
                if let Some(lib) = lib {
                    if allow_write && !lib.is_writable() {
                        return Err(
                            "Cannot perform this operation on a read-only library".to_string()
                        );
                    }
                    return Ok(Some((lib.root_path(), lib.readonly)));
                }
                return Err(format!(
                    "Content library '{}' not found in virtual path '{}'",
                    trimmed, vpath
                ));
            }
            Err(e) => return Err(e.to_string()),
        };

        let lib = self
            .config
            .content_libraries
            .iter()
            .find(|l| l.name == vp.library)
            .ok_or_else(|| {
                format!(
                    "Content library '{}' not found in virtual path '{}'",
                    vp.library, vpath
                )
            })?;

        if allow_write && !lib.is_writable() {
            return Err("Cannot perform this operation on a read-only library".to_string());
        }

        Ok(Some((lib.resolve(&vp.sub_path), lib.readonly)))
    }

    /// Resolve a virtual path for a mutating tool. Bundles the three
    /// lines every mutating tool needed: path resolution, the
    /// virtual-root rejection, and the read-only-library rejection.
    /// [`Self::resolve_virtual_path`] already returns an error when
    /// the target library is read-only, so this helper is sufficient
    /// on its own (no separate `if readonly` check needed at the
    /// call site). Returns the absolute filesystem path on success.
    pub fn resolve_writable(&self, vpath: &str) -> Result<PathBuf, String> {
        match self.resolve_virtual_path(vpath, true)? {
            Some((path, _readonly)) => Ok(path),
            None => Err("Cannot perform this operation on the virtual root".to_string()),
        }
    }

    pub fn publish_file_event(&self, kind: FileEventKind, path: &Path) {
        let producer = FileEventProducer::new(self.file_event_bus);
        match kind {
            FileEventKind::Discovered => producer.publish_discovered(path),
            FileEventKind::Updated => producer.publish_updated(path),
            FileEventKind::Removed => producer.publish_removed(path),
            FileEventKind::DirDiscovered => producer.publish_dir_discovered(path),
            FileEventKind::DirRemoved => producer.publish_dir_removed(path),
        }
    }

    pub fn file_event_producer(&self) -> FileEventProducer<'a> {
        FileEventProducer::new(self.file_event_bus)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ContentLibrary;

    fn test_config() -> AppConfig {
        let mut config = AppConfig::default();
        config.content_libraries.push(ContentLibrary {
            name: "TestLib".to_string(),
            root_folder: "/tmp/testlib".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        });
        config.content_libraries.push(ContentLibrary {
            name: "ReadOnlyLib".to_string(),
            root_folder: "/tmp/readonly".to_string(),
            kind: "text".to_string(),
            readonly: true,
            priority: 0,
        });
        config
    }

    #[test]
    fn test_resolve_valid_path() {
        let config = test_config();
        let bus = Bus::new();
        let ctx = ToolContext::new(&config, &bus);
        let result = ctx.resolve_virtual_path("TestLib/sub/file.md", false);
        assert!(result.is_ok());
        let (path, readonly) = result.unwrap().unwrap();
        assert_eq!(path, PathBuf::from("/tmp/testlib/sub/file.md"));
        assert!(!readonly);
    }

    #[test]
    fn test_resolve_traversal_rejected() {
        let config = test_config();
        let bus = Bus::new();
        let ctx = ToolContext::new(&config, &bus);
        let result = ctx.resolve_virtual_path("TestLib/../outside", false);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("path traversal"));
    }

    #[test]
    fn test_resolve_unknown_library() {
        let config = test_config();
        let bus = Bus::new();
        let ctx = ToolContext::new(&config, &bus);
        let result = ctx.resolve_virtual_path("NonExistent/file.md", false);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .contains("Content library 'NonExistent' not found")
        );
    }

    #[test]
    fn test_resolve_readonly_write() {
        let config = test_config();
        let bus = Bus::new();
        let ctx = ToolContext::new(&config, &bus);
        let result = ctx.resolve_virtual_path("ReadOnlyLib/file.md", true);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("read-only library"));
    }

    #[test]
    fn test_resolve_readonly_read() {
        let config = test_config();
        let bus = Bus::new();
        let ctx = ToolContext::new(&config, &bus);
        let result = ctx.resolve_virtual_path("ReadOnlyLib/file.md", false);
        assert!(result.is_ok());
        let (path, readonly) = result.unwrap().unwrap();
        assert_eq!(path, PathBuf::from("/tmp/readonly/file.md"));
        assert!(readonly);
    }

    #[test]
    fn test_resolve_root_path() {
        let config = test_config();
        let bus = Bus::new();
        let ctx = ToolContext::new(&config, &bus);
        let result = ctx.resolve_virtual_path("/", false);
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());

        let result2 = ctx.resolve_virtual_path(".", false);
        assert!(result2.is_ok());
        assert!(result2.unwrap().is_none());
    }

    /// Regression: a leading slash on a library-prefixed path must not cause
    /// the entire trimmed path to be treated as a library name. Before the
    /// fix, `VirtualPath::parse` saw the leading `/` as the separator and
    /// returned `InvalidFormat("library name is empty")`, which routed the
    /// lookup through the fallback branch that compared the full
    /// `"Wiki/Career/SQA.md"` string against library names — failing with a
    /// misleading "Content library not found" error despite a valid library.
    #[test]
    fn test_resolve_leading_slash_path() {
        let mut config = test_config();
        config.content_libraries.push(ContentLibrary {
            name: "Wiki".to_string(),
            root_folder: "/tmp/wiki".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        });
        let bus = Bus::new();
        let ctx = ToolContext::new(&config, &bus);

        // Leading-slash form must resolve to lib="Wiki", sub="Career/SQA.md".
        let result = ctx.resolve_virtual_path("/Wiki/Career/SQA.md", false);
        assert!(
            result.is_ok(),
            "leading slash must resolve; got {:?}",
            result.err()
        );
        let (path, readonly) = result.unwrap().unwrap();
        assert_eq!(path, PathBuf::from("/tmp/wiki/Career/SQA.md"));
        assert!(!readonly);

        // Backslash + leading-slash form must also work.
        let result = ctx.resolve_virtual_path("\\Wiki\\Career\\SQA.md", false);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().unwrap().0,
            PathBuf::from("/tmp/wiki/Career/SQA.md")
        );

        // Trailing slash must not break resolution either.
        let result = ctx.resolve_virtual_path("/Wiki/Career/SQA.md/", false);
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().unwrap().0,
            PathBuf::from("/tmp/wiki/Career/SQA.md")
        );
    }

    /// Regression: a path referencing an unknown library with a leading slash
    /// still produces the expected "library not found" error (referencing the
    /// *library name*, not the full sub-path, in its error message).
    #[test]
    fn test_resolve_leading_slash_unknown_library() {
        let mut config = test_config();
        config.content_libraries.push(ContentLibrary {
            name: "Wiki".to_string(),
            root_folder: "/tmp/wiki".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        });
        let bus = Bus::new();
        let ctx = ToolContext::new(&config, &bus);

        let result = ctx.resolve_virtual_path("/UnknownLib/sub/file.md", false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        // Error should reference the library name "UnknownLib", not the
        // trimmed full path "UnknownLib/sub/file.md".
        assert!(
            err == "Content library 'UnknownLib' not found in virtual path '/UnknownLib/sub/file.md'"
                || err.contains("Content library 'UnknownLib' not found"),
            "unexpected error: {}",
            err
        );
    }
}
