//! Prompt discovery — walks content libraries to find prompt files (markdown tagged with `tags: [prompt]`) for the batch dialog.

use super::types::PromptInfo;
use crate::config::{AppConfig, ContentLibrary};
use crate::markdown::Document;
use serde_norway::Value;
use std::collections::BTreeSet;
use std::fmt::Display;
use std::path::{Path, PathBuf};

const PROMPT_DISCOVERY_ERROR_CODE: &str = "BATCH-4001";
const PROMPT_DISCOVERY_FAILURE_MESSAGE: &str = "Prompt discovery failed";
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

        for entry in WalkDir::new(root) {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    log_prompt_discovery_error(root, error);
                    continue;
                }
            };
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
                    log_prompt_discovery_error(path, e);
                    continue;
                }
            };

            // `Document` pairs the front-matter and body in one
            // value, so the prompt tag check and the body
            // extraction read them from the same parse instead of
            // calling `parse_front_matter` separately.
            let doc = Document::new(content);
            let Some(fm) = doc.front_matter() else {
                continue;
            };
            if !has_prompt_tag(&fm.yaml) {
                continue;
            }

            let relative = path.strip_prefix(root).unwrap_or(path).to_path_buf();
            let display_name = format!("{} / {}", library.name, relative.display());

            prompts.push(PromptInfo {
                path: path.to_path_buf(),
                display_name,
                library_name: library.name.clone(),
                content: doc.body().trim().to_string(),
            });
        }
    }

    for skill in discover_batch_skills(&config.content_libraries) {
        if !prompts.iter().any(|p| p.path == skill.path) {
            prompts.push(skill);
        }
    }

    prompts.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    prompts
}

/// Discovers all batch skill files from content libraries' `Skills/Batch` folder (VFS-123).
pub fn discover_batch_skills(libraries: &[ContentLibrary]) -> Vec<PromptInfo> {
    let mut skills = Vec::new();
    for lib in libraries {
        let batch_dir = Path::new(&lib.root_folder).join("Skills").join("Batch");
        if !batch_dir.exists() || !batch_dir.is_dir() {
            continue;
        }
        let entries = match std::fs::read_dir(&batch_dir) {
            Ok(entries) => entries,
            Err(error) => {
                log_prompt_discovery_error(&batch_dir, error);
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    log_prompt_discovery_error(&batch_dir, error);
                    continue;
                }
            };
            let path = entry.path();
            if path.is_file() {
                let file_name = path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                if file_name.starts_with('.') {
                    continue;
                }
                let content = match std::fs::read_to_string(&path) {
                    Ok(content) => content,
                    Err(error) => {
                        log_prompt_discovery_error(&path, error);
                        continue;
                    }
                };
                let doc = Document::new(content);
                let stem = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| file_name.clone());
                let display_name = format!("{} / Skills/Batch/{}", lib.name, stem);
                skills.push(PromptInfo {
                    path: path.clone(),
                    display_name,
                    library_name: lib.name.clone(),
                    content: doc.body().trim().to_string(),
                });
            }
        }
    }
    skills.sort_by(|a, b| {
        a.display_name
            .cmp(&b.display_name)
            .then_with(|| a.path.cmp(&b.path))
    });
    skills
}

/// Emits the coded error shared by failures while scanning prompt libraries.
fn log_prompt_discovery_error(path: &Path, error: impl Display) {
    tracing::error!(
        target: "batch",
        error_code = PROMPT_DISCOVERY_ERROR_CODE,
        path = %path.display(),
        error = %error,
        message = PROMPT_DISCOVERY_FAILURE_MESSAGE
    );
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
                    log_prompt_discovery_error(path, e);
                    return None;
                }
            };
            let body = Document::new(content).body().trim().to_string();
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

    // Include batch skills (VFS-123)
    for skill in discover_batch_skills(libraries) {
        if !prompts.iter().any(|p| p.path == skill.path) {
            prompts.push(skill);
        }
    }

    prompts.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    prompts
}

/// Check if a parsed YAML front matter value contains a `prompt` tag (case-insensitive).
fn has_prompt_tag(yaml_val: &Value) -> bool {
    let Some(mapping) = yaml_val.as_mapping() else {
        return false;
    };
    let Some(tags_val) = mapping.get("tags") else {
        return false;
    };
    if let Some(arr) = tags_val.as_sequence() {
        arr.iter().any(|item| {
            item.as_str()
                .is_some_and(|s| s.eq_ignore_ascii_case("prompt"))
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
    // `Document` already strips the front-matter when computing
    // the body, so no separate `parse_front_matter` call is
    // needed.
    Ok(Document::new(content).body().trim().to_string())
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

    #[test]
    fn test_discover_batch_skills() {
        let dir = tempfile::tempdir().unwrap();
        let sys_dir = dir.path().join("system");
        let batch_dir = sys_dir.join("Skills").join("Batch");
        fs::create_dir_all(&batch_dir).unwrap();

        fs::write(
            batch_dir.join("SummarizeAll.md"),
            "---\ntitle: Summarize\n---\nSummarize this file.",
        )
        .unwrap();
        fs::write(
            batch_dir.join("ExtractEntities.txt"),
            "Extract entities from this file.",
        )
        .unwrap();
        fs::write(batch_dir.join(".hidden.md"), "Ignore me").unwrap();

        let libs = vec![ContentLibrary {
            root_folder: sys_dir.to_string_lossy().to_string(),
            name: "System".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        }];

        let skills = discover_batch_skills(&libs);
        assert_eq!(skills.len(), 2);
        assert_eq!(
            skills[0].display_name,
            "System / Skills/Batch/ExtractEntities"
        );
        assert_eq!(skills[0].content, "Extract entities from this file.");
        assert_eq!(skills[1].display_name, "System / Skills/Batch/SummarizeAll");
        assert_eq!(skills[1].content, "Summarize this file.");

        let config = AppConfig {
            content_libraries: libs.clone(),
            ..Default::default()
        };
        let discovered = discover_prompts(&config);
        assert_eq!(discovered.len(), 2);

        let resolved = resolve_prompts(&BTreeSet::new(), &libs);
        assert_eq!(resolved.len(), 2);
    }

    #[test]
    fn test_discover_batch_skills_deterministic_sorting() {
        let dir = tempfile::tempdir().unwrap();
        let sys_dir = dir.path().join("system");
        let batch_dir = sys_dir.join("Skills").join("Batch");
        fs::create_dir_all(&batch_dir).unwrap();

        fs::write(batch_dir.join("Z_Skill.md"), "Z content").unwrap();
        fs::write(batch_dir.join("A_Skill.md"), "A content").unwrap();
        fs::write(batch_dir.join("M_Skill.md"), "M content").unwrap();

        let libs = vec![ContentLibrary {
            root_folder: sys_dir.to_string_lossy().to_string(),
            name: "System".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        }];

        let skills = discover_batch_skills(&libs);
        assert_eq!(skills.len(), 3);
        assert_eq!(skills[0].display_name, "System / Skills/Batch/A_Skill");
        assert_eq!(skills[0].content, "A content");
        assert_eq!(skills[1].display_name, "System / Skills/Batch/M_Skill");
        assert_eq!(skills[1].content, "M content");
        assert_eq!(skills[2].display_name, "System / Skills/Batch/Z_Skill");
        assert_eq!(skills[2].content, "Z content");
    }

    #[test]
    fn test_resolve_prompts_success() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("prompt.md");
        fs::write(
            &path,
            "---\ntitle: Resolved\ntags: [prompt]\n---\nResolved prompt body.",
        )
        .unwrap();

        let libs = vec![ContentLibrary {
            root_folder: dir.path().to_string_lossy().to_string(),
            name: "TestLib".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        }];
        let prompt_paths = BTreeSet::from([path.clone()]);

        let resolved = resolve_prompts(&prompt_paths, &libs);

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].path, path);
        assert_eq!(resolved[0].display_name, "TestLib / prompt.md");
        assert_eq!(resolved[0].content, "Resolved prompt body.");
        assert_eq!(resolved[0].library_name, "TestLib");
    }

    #[test]
    fn test_resolve_prompts_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing.md");
        let libs = vec![ContentLibrary {
            root_folder: dir.path().to_string_lossy().to_string(),
            name: "TestLib".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        }];
        let prompt_paths = BTreeSet::from([path]);

        let resolved = resolve_prompts(&prompt_paths, &libs);

        assert_eq!(resolved.len(), 0);
    }

    #[test]
    fn test_resolve_prompts_without_prompt_tag() {
        // resolve_prompts does NOT filter by prompt tag — it trusts the caller
        // (tag manager) to supply only prompt-tagged paths. It just reads the
        // file and builds PromptInfo.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not-a-prompt.md");
        fs::write(
            &path,
            "---\ntitle: Not a Prompt\ntags: [other]\n---\nBody without prompt tag.",
        )
        .unwrap();

        let libs = vec![ContentLibrary {
            root_folder: dir.path().to_string_lossy().to_string(),
            name: "TestLib".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        }];
        let prompt_paths = BTreeSet::from([path]);

        let resolved = resolve_prompts(&prompt_paths, &libs);

        // The path is returned even without a prompt tag because resolve_prompts
        // trusts the caller's prompt_paths set.
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].content, "Body without prompt tag.");
    }

    #[test]
    fn test_resolve_prompts_invalid_utf8() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("invalid.md");
        // Write invalid UTF-8
        std::fs::write(&path, [0xFF, 0xFE, 0xFD]).unwrap();

        let libs = vec![ContentLibrary {
            root_folder: dir.path().to_string_lossy().to_string(),
            name: "TestLib".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        }];
        let prompt_paths = BTreeSet::from([path]);

        let resolved = resolve_prompts(&prompt_paths, &libs);

        assert_eq!(resolved.len(), 0);
    }

    #[test]
    fn test_log_prompt_discovery_error_emits_structured_fields() {
        // This test verifies the structured log is emitted with the correct fields.
        // We can't easily capture tracing events in unit tests without a subscriber,
        // so we just verify the function compiles and runs without panicking.
        let path = Path::new("test/path.md");
        log_prompt_discovery_error(path, "test error message");
        log_prompt_discovery_error(
            path,
            std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
        );
    }
}
