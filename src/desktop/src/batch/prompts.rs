use crate::batch::types::PromptInfo;
use crate::config::{AppConfig, ContentLibrary};
use crate::utils::markdown::parse_front_matter;
use serde_yaml::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Discovers all prompt files in content libraries.
/// A prompt file is a .md/.markdown file with `tags: [prompt]` in front matter.
pub fn discover_prompts(config: &AppConfig) -> Vec<PromptInfo> {
    let mut prompts = Vec::new();

    for library in &config.content_libraries {
        let root = Path::new(&library.root_folder);
        if !root.exists() {
            continue;
        }

        for entry in WalkDir::new(root).into_iter().filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("");
            if ext != "md" && ext != "markdown" {
                continue;
            }

            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to read prompt file {}: {}", path.display(), e);
                    continue;
                }
            };

            let Some((yaml_val, body)) = parse_front_matter(&content) else {
                continue;
            };

            if !has_prompt_tag(&yaml_val) {
                continue;
            }

            let relative = path.strip_prefix(root).unwrap_or(path).to_path_buf();
            let display_name = format!("{} / {}", library.name, relative.display());

            prompts.push(PromptInfo {
                path: path.to_path_buf(),
                display_name,
                library_name: library.name.clone(),
                content: body.trim().to_string(),
            });
        }
    }

    prompts.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    prompts
}

/// Resolve a set of prompt paths into full `PromptInfo` objects.
///
/// Unlike `discover_prompts` which walks the filesystem, this function
/// reads the actual file content for each path in `prompt_paths` and
/// builds `PromptInfo` using the provided content libraries for display
/// name resolution. It is intended to be called with the tag manager's
/// current `prompt_paths()` set so the prompt list stays in sync with
/// the tag index.
pub fn resolve_prompts(
    prompt_paths: &BTreeSet<PathBuf>,
    libraries: &[ContentLibrary],
) -> Vec<PromptInfo> {
    let mut prompts: Vec<PromptInfo> = prompt_paths
        .iter()
        .filter_map(|path| {
            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to read prompt file {}: {}", path.display(), e);
                    return None;
                }
            };
            let body = match parse_front_matter(&content) {
                Some((_, body)) => body.trim().to_string(),
                None => content.trim().to_string(),
            };
            // Find the library this path belongs to.
            let mut library_name = String::new();
            let mut display_name = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            for lib in libraries {
                let lib_root = Path::new(&lib.root_folder);
                if let Ok(relative) = path.strip_prefix(lib_root) {
                    library_name = lib.name.clone();
                    display_name = format!("{} / {}", lib.name, relative.display());
                    break;
                }
            }
            Some(PromptInfo {
                path: path.clone(),
                display_name,
                library_name,
                content: body,
            })
        })
        .collect();
    prompts.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    prompts
}

/// Check if a parsed YAML front matter value contains a `prompt` tag (case-insensitive).
fn has_prompt_tag(yaml_val: &Value) -> bool {
    let Some(mapping) = yaml_val.as_mapping() else {
        return false;
    };
    let Some(tags_val) = mapping.get(&Value::String("tags".to_string())) else {
        return false;
    };
    if let Some(arr) = tags_val.as_sequence() {
        arr.iter().any(|item| {
            item.as_str()
                .map_or(false, |s| s.eq_ignore_ascii_case("prompt"))
        })
    } else if let Some(s) = tags_val.as_str() {
        s.eq_ignore_ascii_case("prompt")
    } else {
        false
    }
}

/// Reads prompt content from file (body only, excluding YAML front matter).
pub fn read_prompt_content(path: &Path) -> Result<String, std::io::Error> {
    let content = std::fs::read_to_string(path)?;
    let Some((_, body)) = crate::utils::markdown::parse_front_matter(&content) else {
        return Ok(content); // No front matter, return whole content
    };
    Ok(body.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_read_prompt_content_with_front_matter() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-prompt.md");
        fs::write(
            &path,
            "---\ntitle: My Prompt\ntags: [prompt]\n---\nThis is the prompt body.",
        )
        .unwrap();

        let content = read_prompt_content(&path).unwrap();
        assert_eq!(content, "This is the prompt body.");
    }

    #[test]
    fn test_read_prompt_content_no_front_matter() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("simple.md");
        fs::write(&path, "Just plain content without front matter.").unwrap();

        let content = read_prompt_content(&path).unwrap();
        assert_eq!(content, "Just plain content without front matter.");
    }

    #[test]
    fn test_read_prompt_content_empty_body() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("empty.md");
        fs::write(&path, "---\ntitle: Empty\n---\n   ").unwrap();

        let content = read_prompt_content(&path).unwrap();
        assert_eq!(content, "");
    }

    #[test]
    fn test_read_prompt_content_nonexistent_file() {
        let result = read_prompt_content(Path::new("/nonexistent/file.md"));
        assert!(result.is_err());
    }
}
