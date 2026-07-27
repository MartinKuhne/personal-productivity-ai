//! System-prompt builder — augments the base config prompt with context about the active file, directory, and selected files.

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
                prompt.push_str(&format!(
                    "\n\nUser Context (from {}):\n{}",
                    lib.name, content
                ));
            }
        }
        prompt
    }
}

fn build_base_prompt(config: &AppConfig) -> String {
    let date_str = chrono::Local::now().format("%Y-%m-%d").to_string();
    let mut prompt = format!(
        "You are FastMD Agent, an autonomous assistant helper for managing the Markdown workspace. You can read, create, search, and edit files, fetch web pages, and manage tags using your tools. Help the user achieve their goal by using tools step by step. Respond to the user using Markdown format.\n\nCRITICAL: Avoid context bloat! Do NOT use the `read_file` tool on multiple files in a single step. Always prefer `read_yaml_header` to survey documents, or `grep` to extract specific information without reading entire files.\n\nToday's date and time is: {}",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_prompt_contains_date() {
        let config = AppConfig::default();
        let prompt = build_base_prompt(&config);
        assert!(prompt.contains("FastMD Agent"));
        assert!(prompt.contains("Today's date and time is"));
    }

    #[test]
    fn test_base_prompt_with_user_info() {
        let config = AppConfig {
            user_name: Some("Alice".to_string()),
            user_gender: Some("female".to_string()),
            ..AppConfig::default()
        };
        let prompt = build_base_prompt(&config);
        assert!(prompt.contains("Alice"));
        assert!(prompt.contains("female"));
    }

    #[test]
    fn test_base_prompt_with_extension() {
        let config = AppConfig {
            system_prompt_extension: Some("Custom instructions.".to_string()),
            ..AppConfig::default()
        };
        let prompt = build_base_prompt(&config);
        assert!(prompt.contains("Custom instructions."));
    }

    #[test]
    fn test_builder_with_active_file() {
        let config = AppConfig::default();
        let prompt = SystemPromptBuilder::new(&config)
            .with_active_file(Some(PathBuf::from("test.md")))
            .build(&config);
        assert!(prompt.contains("viewing the file"));
    }

    #[test]
    fn test_builder_with_active_dir() {
        let config = AppConfig::default();
        let prompt = SystemPromptBuilder::new(&config)
            .with_active_dir(Some(PathBuf::from("mydir")))
            .build(&config);
        assert!(prompt.contains("directory context"));
    }

    #[test]
    fn test_builder_with_selected_files() {
        let config = AppConfig::default();
        let mut files = HashSet::new();
        files.insert(PathBuf::from("a.md"));
        let prompt = SystemPromptBuilder::new(&config)
            .with_selected_files(files)
            .build(&config);
        assert!(prompt.contains("selected the following files"));
    }

    #[test]
    fn test_builder_active_file_takes_priority_over_dir() {
        let config = AppConfig::default();
        let prompt = SystemPromptBuilder::new(&config)
            .with_active_file(Some(PathBuf::from("test.md")))
            .with_active_dir(Some(PathBuf::from("dir")))
            .build(&config);
        assert!(prompt.contains("viewing the file"));
        assert!(!prompt.contains("directory context"));
    }

    #[test]
    fn test_parse_age_valid_date() {
        assert!(parse_age("1990-01-01").is_some());
    }

    #[test]
    fn test_parse_age_year_only() {
        let result = parse_age("1990");
        assert!(result.is_some());
        assert!(result.unwrap().starts_with('~'));
    }

    #[test]
    fn test_parse_age_invalid() {
        assert!(parse_age("not-a-date").is_none());
    }

    #[test]
    fn test_base_prompt_date_format_no_seconds() {
        let config = AppConfig::default();
        let prompt = build_base_prompt(&config);
        let date_line = prompt
            .lines()
            .find(|l| l.contains("Today's date and time is:"))
            .expect("date line should exist");
        let date_val = date_line
            .split("Today's date and time is: ")
            .nth(1)
            .unwrap();
        assert_eq!(
            date_val.len(),
            10,
            "Date format should be YYYY-MM-DD (10 chars), got: {}",
            date_val
        );
    }

    #[test]
    fn test_builder_selected_files_deterministic_order() {
        let config = AppConfig::default();
        let mut files = HashSet::new();
        files.insert(PathBuf::from("z_file.md"));
        files.insert(PathBuf::from("a_file.md"));
        files.insert(PathBuf::from("m_file.md"));
        files.insert(PathBuf::from("b_file.md"));

        let prompt = SystemPromptBuilder::new(&config)
            .with_selected_files(files)
            .build(&config);

        let selected_part = prompt
            .split("The user has also selected the following files:")
            .nth(1)
            .expect("should contain selected files section");
        assert!(
            selected_part.contains("a_file.md b_file.md m_file.md z_file.md"),
            "Selected files must be sorted alphabetically, got: {}",
            selected_part
        );
    }
}
