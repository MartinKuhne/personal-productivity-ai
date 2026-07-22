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

        let vp = match VirtualPath::parse(vpath) {
            Ok(vp) => vp,
            Err(VirtualPathError::InvalidFormat(_)) => {
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
        assert!(result
            .unwrap_err()
            .contains("Content library 'NonExistent' not found"));
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
}
