//! Prompt discovery - walks content libraries to find prompt files (markdown tagged with `tags: [prompt]`) for the batch dialog.
//!
//! Unit tests live in the sibling `prompts_tests.rs` sidecar.

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
#[path = "prompts_tests.rs"]
mod tests;
