//! System-prompt builder — augments the base config prompt with context about the active file, directory, and selected files.
//!
//! Unit tests live in the sibling `prompt_builder_tests.rs` sidecar.

use crate::agent::datamark::{SECURITY_HEADER, wrap_user_md};
use crate::config::AppConfig;
use std::collections::HashSet;
use std::path::PathBuf;

pub struct SystemPromptBuilder {
    base_prompt: String,
    active_file: Option<PathBuf>,
    active_dir: Option<PathBuf>,
    selected_files: HashSet<PathBuf>,
}

impl SystemPromptBuilder {
    pub fn new(config: &AppConfig) -> Self {
        Self {
            base_prompt: build_base_prompt(config),
            active_file: None,
            active_dir: None,
            selected_files: HashSet::new(),
        }
    }

    pub fn with_active_file(mut self, path: Option<PathBuf>) -> Self {
        self.active_file = path;
        self
    }

    pub fn with_active_dir(mut self, path: Option<PathBuf>) -> Self {
        self.active_dir = path;
        self
    }

    pub fn with_selected_files(mut self, files: HashSet<PathBuf>) -> Self {
        self.selected_files = files;
        self
    }

    pub fn build(self, config: &AppConfig) -> String {
        let mut prompt = self.base_prompt;
        let to_virtual = |path: &PathBuf| -> String {
            crate::config::library_display_label(&config.content_libraries, path)
                .unwrap_or_else(|| path.to_string_lossy().to_string())
        };

        if let Some(active) = &self.active_file {
            prompt.push_str(&format!(
                " The user is currently viewing the file: {}",
                to_virtual(active)
            ));
        } else if let Some(dir) = &self.active_dir {
            prompt.push_str(&format!(
                " The user has selected the directory context: {}",
                to_virtual(dir)
            ));
        }

        if !self.selected_files.is_empty() {
            prompt.push_str(" The user has also selected the following files:");
            let mut sorted_files: Vec<_> = self.selected_files.iter().collect();
            sorted_files.sort();
            for f in sorted_files {
                prompt.push_str(&format!(" {}", to_virtual(f)));
            }
            prompt.push('.');
        }

        for lib in &config.content_libraries {
            let user_md = std::path::Path::new(&lib.root_folder).join("USER.md");
            if user_md.exists()
                && let Ok(content) = std::fs::read_to_string(&user_md)
            {
                // USER.md is user-placed content, not a system
                // instruction. Wrap it in a datamark envelope so the
                // LLM treats it as data, not directives. R1 (see
                // doc/planning/prompt-injection-security.md).
                prompt.push_str(&format!(
                    "\n\nUser Context (from {}):\n{}",
                    lib.name,
                    wrap_user_md(&lib.name, &content)
                ));
            }
        }
        prompt
    }
}

fn build_base_prompt(config: &AppConfig) -> String {
    let date_str = chrono::Local::now().format("%Y-%m-%d").to_string();
    // Prepend the security header BEFORE the role definition. Research
    // (Microsoft Spotlighting) shows the LLM follows security
    // instructions more reliably when they are the first thing in the
    // system prompt, before any user-placed content like
    // `system_prompt_extension` or USER.md is appended below.
    let mut prompt = format!(
        "{SECURITY_HEADER}\n\nYou are FastMD Agent, an autonomous assistant helper for managing the Markdown workspace. You can read, create, search, and edit files, fetch web pages, and manage tags using your tools. Help the user achieve their goal by using tools step by step. Respond to the user using Markdown format.\n\nCRITICAL: Avoid context bloat! Do NOT use the `read_note` tool on multiple notes in a single step. Always prefer `read_yaml_header` to survey notes, or `search_notes` to extract specific information without reading entire notes.\n\nToday's date and time is: {}",
        date_str
    );
    if let Some(name) = &config.user_name {
        prompt.push_str(&format!("\nUser's Name: {}", name));
    }
    if let Some(address) = &config.user_address {
        prompt.push_str(&format!("\nUser's Address: {}", address));
    }
    if let Some(birthdate) = &config.user_birthdate {
        append_birthdate_info(&mut prompt, birthdate);
    }
    if let Some(gender) = &config.user_gender {
        prompt.push_str(&format!("\nUser's Gender: {}", gender));
    }
    if let Some(ext) = &config.system_prompt_extension {
        prompt.push_str(&format!("\n{}", ext));
    }
    prompt
}

fn append_birthdate_info(prompt: &mut String, birthdate: &str) {
    let age_str = parse_age(birthdate);
    if let Some(a) = age_str {
        prompt.push_str(&format!("\nUser's Age: {}", a));
    } else {
        prompt.push_str(&format!("\nUser's Birthdate/Age info: {}", birthdate));
    }
}

fn parse_age(birthdate: &str) -> Option<String> {
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
// Tests live in the sibling `prompt_builder_tests.rs` sidecar.
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "prompt_builder_tests.rs"]
mod tests;
