use crate::config::AppConfig;
use crate::file_events::{Bus, FileEvent, FileEventKind, FileEventProducer};
use std::path::{Component, Path, PathBuf};

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
        if Path::new(vpath)
            .components()
            .any(|c| c == Component::ParentDir)
        {
            return Err("Path traversal not allowed".to_string());
        }

        let path = Path::new(vpath);
        let mut components = path.components().peekable();

        while let Some(c) = components.peek() {
            match c {
                Component::RootDir | Component::CurDir => {
                    components.next();
                }
                _ => break,
            }
        }

        if components.peek().is_none() {
            return Ok(None);
        }

        if let Some(Component::Normal(first)) = components.next() {
            let first_str = first.to_string_lossy();
            for lib in &self.config.content_libraries {
                if lib.name == first_str {
                    if allow_write && lib.readonly {
                        return Err(
                            "Cannot perform this operation on a read-only library".to_string()
                        );
                    }
                    let rest: PathBuf = components.collect();
                    return Ok(Some((Path::new(&lib.root_folder).join(rest), lib.readonly)));
                }
            }
            Err(format!(
                "Content library '{}' not found in virtual path '{}'",
                first_str, vpath
            ))
        } else {
            Err(format!("Invalid virtual path: '{}'", vpath))
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
        assert!(result.unwrap_err().contains("Path traversal not allowed"));
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
