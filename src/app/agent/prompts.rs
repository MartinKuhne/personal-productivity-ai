//! System-prompt assembly — the agent's domain runs prompts that are
//! pre-built by the caller, but the *construction* of those prompts
//! (security header, current date, user info, system-prompt extension,
//! active file/dir, selected files, USER.md content from content
//! libraries) lives here. The agent module no longer reads
//! `AppConfig::user_*` / `system_prompt_extension` / `content_libraries`
//! — those fields stay on the global config for users to edit, and the
//! prompt assembler reads them at submit time.
//!
//! The output is a `Vec<String>` of system messages, one per
//! "block" (static, dynamic, USER.md). Each block is delivered to
//! the LLM as a separate `role=system` message — this is the
//! standard OpenAI/Anthropic pattern and lets the model distinguish
//! instructions from user content (R1 Spotlighting).
//!
//! Unit tests live in the sibling `prompts_tests.rs` sidecar.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::agent::datamark::{SECURITY_HEADER, wrap_user_md};
use crate::config::AppConfig;

/// Finds a `User.md` (or `USER.md` / `user.md`) file at the root of the specified directory (VFS-130, AGENT-020).
pub fn find_user_md_file(dir: &Path) -> Option<PathBuf> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
                && name.eq_ignore_ascii_case("user.md")
            {
                return Some(path);
            }
        }
    }
    for name in &["User.md", "USER.md", "user.md"] {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

/// Build the system-prompt message blocks for an agent turn.
///
/// Reads the user's identity / system-prompt extension / content
/// libraries from the global [`AppConfig`], wraps each library's
/// `USER.md` / `User.md` (if present) in a datamark envelope, and returns the
/// assembled messages. The caller is the UI's submit path or the
/// batch executor; both have the active file/dir/selected files
/// from the UI state and the global config from the orchestrator.
///
/// Returns a `Vec<String>` where each entry is delivered to the
/// LLM as a separate `role=system` message. The order is:
/// 1. Static prompt (security header + role + critical rules).
/// 2. Dynamic prompt (date, user info, extension, active context).
/// 3. One block per `USER.md` / `User.md` found in the content libraries and system library (VFS-130).
pub fn build_system_prompts(
    config: &AppConfig,
    active_file: Option<&Path>,
    active_dir: Option<&Path>,
    selected_files: &HashSet<PathBuf>,
) -> Vec<String> {
    let mut out = Vec::new();
    out.push(build_static_system_prompt());
    out.push(build_dynamic_system_prompt(
        config,
        active_file,
        active_dir,
        selected_files,
    ));

    let sys_name = config.system_library_display_name();
    let mut visited_paths = HashSet::new();
    for lib in &config.content_libraries {
        let root = Path::new(&lib.root_folder);
        if let Some(user_md) = find_user_md_file(root)
            && let Ok(raw_content) = std::fs::read_to_string(&user_md)
        {
            let content = crate::markdown::DocumentContent::parse(&raw_content).body.trim().to_string();
            if let Ok(canon) = user_md.canonicalize()
                && !visited_paths.insert(canon)
            {
                continue;
            }
            if lib.name == sys_name {
                // VFS-130: System library User.md provided directly without additional context or guardrails.
                out.push(content);
            } else {
                // AGENT-020: Content library USER.md wrapped in datamark envelope.
                out.push(format!(
                    "\nUser Context (from {}):\n{}",
                    lib.name,
                    wrap_user_md(&lib.name, &content)
                ));
            }
        }
    }

    out
}

/// Static base prompt — security header + role + critical rules.
/// Same wording the previous in-agent `build_static_system_prompt`
/// produced; extracted so the prompt assembler owns the full
/// construction pipeline.
fn build_static_system_prompt() -> String {
    format!(
        "{SECURITY_HEADER}\n\nYou are FastMD Agent, a personal assistant grounded in the user's knowledge base — a library of Markdown notes that captures their information, preferences, and context. You help with everyday tasks by reasoning over these notes and using integrated tools: email, calendar, contacts, web search, to-dos, and file operations. Consult the user's own knowledge before reaching for external information, then take action step by step. Respond using Markdown format.\n\nCRITICAL: Avoid context bloat! Do NOT use the `read_note` tool on multiple notes in a single step. Always prefer `read_yaml_header` to survey notes, or `search_notes` to extract specific information without reading entire notes."
    )
}

/// Dynamic prompt — date, user identity, system-prompt extension,
/// active file/dir/selected files. Mirrors the wording of the
/// previous in-agent `build_dynamic_system_prompt`; the
/// active-context block is rendered as three "currently viewing" /
/// "directory context" / "selected the following files" lines.
fn build_dynamic_system_prompt(
    config: &AppConfig,
    active_file: Option<&Path>,
    active_dir: Option<&Path>,
    selected_files: &HashSet<PathBuf>,
) -> String {
    let date_str = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut prompt = format!("Today's date and time is: {}", date_str);
    if let Some(ext) = &config.system_prompt_extension {
        prompt.push_str(&format!("\n{}", ext));
    }
    let to_virtual = |path: &Path| -> String {
        crate::config::library_display_label(&config.content_libraries, path)
            .unwrap_or_else(|| path.to_string_lossy().to_string())
    };
    if let Some(active) = active_file {
        prompt.push_str(&format!(
            "\nThe user is currently viewing the file: {}",
            to_virtual(active)
        ));
    } else if let Some(dir) = active_dir {
        prompt.push_str(&format!(
            "\nThe user has selected the directory context: {}",
            to_virtual(dir)
        ));
    }
    if !selected_files.is_empty() {
        prompt.push_str("\nThe user has also selected the following files:");
        let mut sorted_files: Vec<_> = selected_files.iter().collect();
        sorted_files.sort();
        for f in sorted_files {
            prompt.push_str(&format!(" {}", to_virtual(f)));
        }
        prompt.push('.');
    }
    prompt
}

/// Parse a birthdate into a human-readable age. Accepts
/// `YYYY-MM-DD`, `MM/DD/YYYY`, `DD/MM/YYYY`, `DD-MM-YYYY`,
/// `Month DD, YYYY`, a bare year (`1990`), or a small integer
/// already representing an age. Returns `None` for unparseable input.
pub fn parse_age(birthdate: &str) -> Option<String> {
    use chrono::Datelike;
    if let Ok(parsed) = chrono::NaiveDate::parse_from_str(birthdate, "%Y-%m-%d")
        .or_else(|_| chrono::NaiveDate::parse_from_str(birthdate, "%m/%d/%Y"))
        .or_else(|_| chrono::NaiveDate::parse_from_str(birthdate, "%d/%m/%Y"))
        .or_else(|_| chrono::NaiveDate::parse_from_str(birthdate, "%d-%m-%Y"))
        .or_else(|_| chrono::NaiveDate::parse_from_str(birthdate, "%B %d, %Y"))
    {
        let today = chrono::Local::now().naive_local().date();
        let mut age = today.year() - parsed.year();
        if today.month() < parsed.month()
            || (today.month() == parsed.month() && today.day() < parsed.day())
        {
            age -= 1;
        }
        return Some(age.to_string());
    }
    if let Ok(num) = birthdate.trim().parse::<i32>() {
        let current_year = chrono::Local::now().year();
        if num > 1900 && num <= current_year {
            return Some(format!("~{}", current_year - num));
        }
        if num > 0 && num < 150 {
            return Some(num.to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests live in the sibling `prompts_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "prompts_tests.rs"]
mod tests;
