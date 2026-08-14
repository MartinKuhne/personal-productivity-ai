//! Filesystem agent tools — search notes, read note, list notes by tag, create/update/delete notes, and directory listing.
//!
//! Unit tests live in the sibling `filesystem_tests.rs` sidecar.

use crate::utils::markdown::parse_front_matter;
use crate::utils::tags::extract_tags_from_file;
use std::path::Path;
use walkdir::WalkDir;

/// Default maximum number of match lines the `search_notes` tool returns in
/// a single response. Kept here (rather than inlined at the call site)
/// so the constant has one canonical home and tests can reference it.
pub const DEFAULT_SEARCH_NOTES_MAX_RESULTS: usize = 200;

/// Grep a single content library for a query string, case-insensitively.
/// Returns every matching line as `virtual/path:line - content`, scoped
/// strictly to Markdown (`.md`) files under `root_path`. The caller
/// (the tool registry) is responsible for applying the result cap
/// across libraries, so this function returns all matches unfiltered.
pub fn tool_search_notes(
    ctx: &crate::tools::context::ToolContext,
    root_path: &Path,
    virtual_prefix: &str,
    query: &str,
) -> Result<Vec<String>, String> {
    let mut results = Vec::new();
    let query_lower = query.to_lowercase();
    for entry in WalkDir::new(root_path).into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_file()
            && let Some(ext) = entry.path().extension()
            && ext == "md"
            && let Some(rel_path) = entry.path().strip_prefix(root_path).ok()
            && let Ok(content) = ctx.vfs().read_to_string(entry.path().as_ref())
        {
            let (_, body) = split_file_content(&content);
            let virtual_path = Path::new(virtual_prefix).join(rel_path);
            for (idx, line) in body.lines().enumerate() {
                if line.to_lowercase().contains(&query_lower) {
                    results.push(format!("{}:{} - {}", virtual_path.display(), idx + 1, line));
                }
            }
        }
    }
    Ok(results)
}

pub fn tool_read_tags(
    _ctx: &crate::tools::context::ToolContext,
    root_path: &Path,
) -> Result<crate::tools::dtos::ReadTagsResponse, String> {
    let mut all_tags = std::collections::BTreeSet::new();
    for entry in WalkDir::new(root_path).into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_file()
            && let Some(ext) = entry.path().extension()
            && (ext == "md" || ext == "markdown")
        {
            let tags = extract_tags_from_file(entry.path());
            for tag in tags {
                all_tags.insert(tag);
            }
        }
    }
    Ok(crate::tools::dtos::ReadTagsResponse {
        tags: all_tags.into_iter().collect(),
    })
}

/// Scan a single content library and return every Markdown file whose
/// front-matter contains the given tag, as a sorted list of virtual
/// paths.
///
/// Paging is intentionally **not** applied here — the call site
/// (`registry.rs`) is responsible for slicing the combined
/// cross-library result, so the page and total fields stay consistent
/// regardless of how many libraries the user has configured.
pub fn tool_list_notes_by_tag(
    _ctx: &crate::tools::context::ToolContext,
    root_path: &Path,
    virtual_prefix: &str,
    tag: &str,
) -> Result<Vec<String>, String> {
    let mut matching_files = Vec::new();
    for entry in WalkDir::new(root_path).into_iter().filter_map(|e| e.ok()) {
        if entry.path().is_file()
            && let Some(ext) = entry.path().extension()
            && (ext == "md" || ext == "markdown")
        {
            let tags = extract_tags_from_file(entry.path());
            if tags.contains(&tag.to_string()) {
                let rel_path = entry.path().strip_prefix(root_path).unwrap_or(entry.path());
                let virtual_path = Path::new(virtual_prefix).join(rel_path);
                matching_files.push(virtual_path.to_string_lossy().into_owned());
            }
        }
    }
    // Sort for deterministic paging at the call site — without a
    // stable order the same page could return different files on each
    // call.
    matching_files.sort();
    Ok(matching_files)
}

/// Scan a single directory (non-recursive) and return every Markdown
/// file's virtual path, sorted. Paging is intentionally **not**
/// applied here — the call site (`registry.rs`) is responsible for
/// slicing the result so the page and total fields stay consistent
/// regardless of how the call is dispatched.
pub fn tool_list_notes(
    ctx: &crate::tools::context::ToolContext,
    target_dir: &Path,
    virtual_prefix: &str,
) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    if let Ok(entries) = ctx.vfs().read_dir(target_dir) {
        for entry in entries.into_iter() {
            if entry.is_file {
                let path = &entry.path;
                if let Some(ext) = path.extension()
                    && (ext == "md" || ext == "markdown")
                    && let Some(name) = path.file_name()
                {
                    let virtual_path = Path::new(virtual_prefix).join(name);
                    files.push(virtual_path.to_string_lossy().into_owned());
                }
            }
        }
    }
    // Sort for deterministic paging at the call site.
    files.sort();
    Ok(files)
}

pub fn tool_read_note(
    _ctx: &crate::tools::context::ToolContext,
    path_str: &str,
) -> Result<crate::tools::dtos::ReadNoteResponse, String> {
    match crate::utils::read_text_file(Path::new(path_str)) {
        Ok(content) => {
            let (_, body) = split_file_content(&content);
            Ok(crate::tools::dtos::ReadNoteResponse { content: body })
        }
        Err(e) => Err(format!("Failed to read file: {}", e)),
    }
}

/// Read a contiguous slice of lines from a file.
///
/// `offset` is 0-indexed (`0` is the first line). `limit` is the
/// maximum number of lines to return. The slice is clamped to the
/// file's line count, so a `limit` that overflows returns the
/// remainder; an `offset` past the end returns an empty `content`.
pub fn tool_window_note(
    _ctx: &crate::tools::context::ToolContext,
    path_str: &str,
    offset: usize,
    limit: usize,
) -> Result<crate::tools::dtos::WindowNoteResponse, String> {
    match crate::utils::read_text_file(Path::new(path_str)) {
        Ok(content) => {
            let (_, body) = split_file_content(&content);
            let lines: Vec<&str> = body.lines().collect();
            let slice = if offset >= lines.len() {
                &[][..]
            } else {
                let end = (offset + limit).min(lines.len());
                &lines[offset..end]
            };
            Ok(crate::tools::dtos::WindowNoteResponse {
                content: slice.join("\n"),
            })
        }
        Err(e) => Err(format!("Failed to read file: {}", e)),
    }
}

pub fn tool_create_note(
    ctx: &crate::tools::context::ToolContext,
    path_str: &str,
    content: &str,
    producer: &dyn crate::tools::observer::OnFileChanged,
) -> Result<crate::tools::dtos::CreateNoteResponse, String> {
    if !path_str.to_lowercase().ends_with(".md") {
        return Err("Only markdown files (.md) are allowed.".to_string());
    }

    if content.starts_with("---\n") && parse_front_matter(content).is_none() {
        return Err("Invalid YAML front-matter in markdown.".to_string());
    }

    let path = Path::new(path_str);
    if path.exists() {
        return Err("File already exists. This tool can only create new files.".to_string());
    }

    if let Some(parent) = path.parent()
        && let Err(e) = ctx.vfs().create_dir_all(parent)
    {
        return Err(format!("Failed to create parent directories: {}", e));
    }
    match ctx.vfs().write(path, content.as_bytes()) {
        Ok(_) => {
            let size_bytes = ctx.vfs().metadata(path).map(|m| m.len).unwrap_or(0);
            // Tell the rest of the app this file now exists so the
            // directory tree, tag manager, etc. can pick it up without
            // waiting for an OS-level notify event.
            producer.on_file_changed(path);
            Ok(crate::tools::dtos::CreateNoteResponse {
                result: "File created successfully.".to_string(),
                size_bytes,
            })
        }
        Err(e) => Err(format!("Failed to write file: {}", e)),
    }
}

/// Insert `lines_to_insert` into the file at 0-indexed position `offset`.
///
/// `offset == 0` inserts at the top; `offset == lines.len()` appends
/// to the end. `offset > lines.len()` returns an error.
pub fn tool_insert_into_note(
    ctx: &crate::tools::context::ToolContext,
    path_str: &str,
    offset: usize,
    lines_to_insert: &[String],
    producer: &dyn crate::tools::observer::OnFileChanged,
) -> Result<crate::tools::dtos::InsertIntoNoteResponse, String> {
    match crate::utils::read_text_file(Path::new(path_str)) {
        Ok(content) => {
            let (header, body) = split_file_content(&content);
            let mut lines: Vec<String> = body.lines().map(|s| s.to_string()).collect();
            if offset > lines.len() && offset > 0 {
                return Err("Offset out of range.".to_string());
            }
            for (delta, line) in lines_to_insert.iter().enumerate() {
                lines.insert((offset + delta).min(lines.len()), line.clone());
            }
            let new_body = lines.join("\n");
            let new_content = reconstruct_file_content(header, &new_body);
            match ctx.vfs().write(path_str.as_ref(), new_content.as_bytes()) {
                Ok(_) => {
                    producer.on_file_changed(Path::new(path_str));
                    Ok(crate::tools::dtos::InsertIntoNoteResponse {
                        result: "Lines inserted successfully.".to_string(),
                    })
                }
                Err(e) => Err(format!("Failed to write file: {}", e)),
            }
        }
        Err(e) => Err(format!("Failed to read file: {}", e)),
    }
}

pub fn tool_patch_note(
    ctx: &crate::tools::context::ToolContext,
    path_str: &str,
    old_string: &str,
    new_string: &str,
    producer: &dyn crate::tools::observer::OnFileChanged,
) -> Result<crate::tools::dtos::PatchNoteResponse, String> {
    match crate::utils::read_text_file(Path::new(path_str)) {
        Ok(content) => {
            let (header, body) = split_file_content(&content);
            if !body.contains(old_string) {
                return Err("The specified old_string was not found in the file body.".to_string());
            }
            let count = body.matches(old_string).count();
            let new_body = body.replace(old_string, new_string);
            let new_content = reconstruct_file_content(header, &new_body);
            match ctx.vfs().write(path_str.as_ref(), new_content.as_bytes()) {
                Ok(_) => {
                    producer.on_file_changed(Path::new(path_str));
                    Ok(crate::tools::dtos::PatchNoteResponse {
                        result: format!("Successfully replaced {} occurrence(s).", count),
                    })
                }
                Err(e) => Err(format!("Failed to write file: {}", e)),
            }
        }
        Err(e) => Err(format!("Failed to read file: {}", e)),
    }
}

fn split_file_content(raw_content: &str) -> (Option<String>, String) {
    match parse_front_matter(raw_content) {
        Some(fm) => {
            let mut body = fm.body.as_str();
            if body.starts_with("\r\n") {
                body = &body[2..];
            } else if body.starts_with('\n') {
                body = &body[1..];
            }
            (Some(fm.source), body.to_string())
        }
        None => (None, raw_content.to_string()),
    }
}

fn reconstruct_file_content(header_source: Option<String>, body: &str) -> String {
    match header_source {
        Some(src) => format!("---{}---\n{}", src, body),
        None => body.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `filesystem_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "filesystem_tests.rs"]
mod tests;
