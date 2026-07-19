use crate::utils::markdown::parse_front_matter;
use crate::utils::tags::extract_tags_from_file;
use std::path::Path;
use walkdir::WalkDir;

pub fn tool_grep(root_path: &Path, query: &str) -> String {
    let mut results = Vec::new();
    let query_lower = query.to_lowercase();
    for entry in WalkDir::new(root_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.path().is_file() {
            if let Some(ext) = entry.path().extension() {
                if ext == "md" || ext == "markdown" {
                    if let Ok(content) = std::fs::read_to_string(entry.path()) {
                        for (idx, line) in content.lines().enumerate() {
                            if line.to_lowercase().contains(&query_lower) {
                                results.push(format!(
                                    "{}:{} - {}",
                                    entry.path().display(),
                                    idx + 1,
                                    line
                                ));
                            }
                        }
                    }
                }
            }
        }
    }
    if results.is_empty() {
        "No matches found.".to_string()
    } else {
        results.join("\n")
    }
}

pub fn tool_read_tags(root_path: &Path) -> String {
    let mut all_tags = std::collections::BTreeSet::new();
    for entry in WalkDir::new(root_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.path().is_file() {
            if let Some(ext) = entry.path().extension() {
                if ext == "md" || ext == "markdown" {
                    let tags = extract_tags_from_file(entry.path());
                    for tag in tags {
                        all_tags.insert(tag);
                    }
                }
            }
        }
    }
    let count = all_tags.len();
    format!("Tags found: {}", count)
}

pub fn tool_list_files_by_tag(root_path: &Path, tag: &str) -> String {
    let mut matching_files = Vec::new();
    for entry in WalkDir::new(root_path)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if entry.path().is_file() {
            if let Some(ext) = entry.path().extension() {
                if ext == "md" || ext == "markdown" {
                    let tags = extract_tags_from_file(entry.path());
                    if tags.contains(&tag.to_string()) {
                        matching_files.push(entry.path().to_string_lossy().into_owned());
                    }
                }
            }
        }
    }
    if matching_files.is_empty() {
        "No matching files found.".to_string()
    } else {
        matching_files.join("\n")
    }
}

pub fn tool_list_files(target_dir: &Path) -> String {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(target_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    let path = entry.path();
                    if let Some(ext) = path.extension() {
                        if ext == "md" || ext == "markdown" {
                            files.push(path.to_string_lossy().into_owned());
                        }
                    }
                }
            }
        }
    }
    if files.is_empty() {
        "No markdown files found.".to_string()
    } else {
        files.join("\n")
    }
}

pub fn tool_read_file(path_str: &str) -> String {
    match std::fs::read_to_string(path_str) {
        Ok(content) => content,
        Err(e) => format!("Error reading file: {}", e),
    }
}

pub fn tool_read_file_lines(path_str: &str, start_line: usize, end_line: usize) -> String {
    match std::fs::read_to_string(path_str) {
        Ok(content) => {
            let lines: Vec<&str> = content.lines().collect();
            if lines.is_empty() && start_line == 1 {
                return "".to_string();
            }
            if start_line == 0 || start_line > lines.len() {
                return "Error: Start line out of range.".to_string();
            }
            let end = std::cmp::min(end_line, lines.len());
            if start_line > end {
                return "Error: Start line greater than end line.".to_string();
            }
            let selected_lines = &lines[start_line - 1..end];
            selected_lines.join("\n")
        }
        Err(e) => format!("Error reading file: {}", e),
    }
}

pub fn tool_create_file(path_str: &str, content: &str) -> String {
    if !path_str.to_lowercase().ends_with(".md") {
        return "Error: Only markdown files (.md) are allowed.".to_string();
    }

    if content.starts_with("---\n") && parse_front_matter(content).is_none() {
        return "Error: Invalid YAML front-matter in markdown.".to_string();
    }

    // Validate the markdown by ensuring it parses successfully
    let parser = pulldown_cmark::Parser::new(content);
    for _ in parser {}

    let path = Path::new(path_str);
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return format!("Error creating parent directories: {}", e);
        }
    }
    match std::fs::write(path, content) {
        Ok(_) => "File created successfully.".to_string(),
        Err(e) => format!("Error writing file: {}", e),
    }
}

pub fn tool_insert_lines(path_str: &str, line_index: usize, lines_to_insert: &[String]) -> String {
    match std::fs::read_to_string(path_str) {
        Ok(content) => {
            let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
            if line_index == 0 || line_index > lines.len() + 1 {
                return "Error: Line index out of range.".to_string();
            }
            let idx = line_index - 1;
            for (offset, line) in lines_to_insert.iter().enumerate() {
                lines.insert(idx + offset, line.clone());
            }
            let new_content = lines.join("\n");
            match std::fs::write(path_str, new_content) {
                Ok(_) => "Lines inserted successfully.".to_string(),
                Err(e) => format!("Error writing file: {}", e),
            }
        }
        Err(e) => format!("Error reading file: {}", e),
    }
}

pub fn tool_delete_lines(path_str: &str, start_line: usize, end_line: usize) -> String {
    match std::fs::read_to_string(path_str) {
        Ok(content) => {
            let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
            if start_line == 0 || start_line > lines.len() {
                return "Error: Start line out of range.".to_string();
            }
            let end = std::cmp::min(end_line, lines.len());
            if start_line > end {
                return "Error: Start line greater than end line.".to_string();
            }
            lines.drain((start_line - 1)..end);
            let new_content = lines.join("\n");
            match std::fs::write(path_str, new_content) {
                Ok(_) => "Lines deleted successfully.".to_string(),
                Err(e) => format!("Error writing file: {}", e),
            }
        }
        Err(e) => format!("Error reading file: {}", e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_tool_grep() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "# Hello\nWorld content\nAnother line").unwrap();
        
        let result = tool_grep(dir.path(), "World");
        assert!(result.contains("World content"));
        assert!(result.contains("test.md"));
    }

    #[test]
    fn test_tool_list_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.md"), "content").unwrap();
        fs::write(dir.path().join("b.txt"), "content").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        fs::write(dir.path().join("sub").join("c.md"), "content").unwrap();
        
        let result = tool_list_files(dir.path());
        assert!(result.contains("a.md"));
        assert!(!result.contains("c.md")); // Non-recursive, should not find c.md
        assert!(!result.contains("b.txt"));
    }

    #[test]
    fn test_tool_read_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "Hello World").unwrap();
        
        let result = tool_read_file(file_path.to_str().unwrap());
        assert_eq!(result, "Hello World");
    }

    #[test]
    fn test_tool_read_file_not_found() {
        let result = tool_read_file("/nonexistent/path.md");
        assert!(result.starts_with("Error reading file"));
    }

    #[test]
    fn test_tool_read_file_lines() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "Line 1\nLine 2\nLine 3\nLine 4").unwrap();
        
        let result = tool_read_file_lines(file_path.to_str().unwrap(), 2, 3);
        assert_eq!(result, "Line 2\nLine 3");
    }

    #[test]
    fn test_tool_read_file_lines_empty_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("empty.md");
        fs::write(&file_path, "").unwrap();
        
        let result = tool_read_file_lines(file_path.to_str().unwrap(), 1, 50);
        assert_eq!(result, "");
    }

    #[test]
    fn test_tool_create_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("new.md");
        
        let result = tool_create_file(file_path.to_str().unwrap(), "---\ntitle: Test\n---\n# Hello");
        assert_eq!(result, "File created successfully.");
        assert!(file_path.exists());
        
        let content = fs::read_to_string(&file_path).unwrap();
        assert!(content.contains("title: Test"));
        assert!(content.contains("# Hello"));
    }

    #[test]
    fn test_tool_create_file_invalid_extension() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("new.txt");
        
        let result = tool_create_file(file_path.to_str().unwrap(), "content");
        assert_eq!(result, "Error: Only markdown files (.md) are allowed.");
    }

    #[test]
    fn test_tool_create_file_invalid_yaml() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("new.md");
        
        let result = tool_create_file(file_path.to_str().unwrap(), "---\ninvalid: [unclosed\n---\nContent");
        assert_eq!(result, "Error: Invalid YAML front-matter in markdown.");
    }

    #[test]
    fn test_tool_insert_lines() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "Line 1\nLine 2\nLine 3").unwrap();
        
        let result = tool_insert_lines(file_path.to_str().unwrap(), 2, &["New Line".to_string()]);
        assert_eq!(result, "Lines inserted successfully.");
        
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Line 1\nNew Line\nLine 2\nLine 3");
    }

    #[test]
    fn test_tool_insert_lines_out_of_range() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "Line 1\nLine 2").unwrap();
        
        let result = tool_insert_lines(file_path.to_str().unwrap(), 5, &["New".to_string()]);
        assert_eq!(result, "Error: Line index out of range.");
    }

    #[test]
    fn test_tool_delete_lines() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "Line 1\nLine 2\nLine 3\nLine 4").unwrap();
        
        let result = tool_delete_lines(file_path.to_str().unwrap(), 2, 3);
        assert_eq!(result, "Lines deleted successfully.");
        
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Line 1\nLine 4");
    }

    #[test]
    fn test_tool_delete_lines_out_of_range() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "Line 1\nLine 2").unwrap();
        
        let result = tool_delete_lines(file_path.to_str().unwrap(), 5, 6);
        assert_eq!(result, "Error: Start line out of range.");
    }

    #[test]
    fn test_tool_read_tags() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.md");
        fs::write(&file_path, "---\ntags: [tag1, tag2]\n---\n# Hello").unwrap();
        
        let result = tool_read_tags(dir.path());
        assert_eq!(result, "Tags found: 2");
    }
}