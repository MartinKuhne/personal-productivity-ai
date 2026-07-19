

#[derive(Debug, Clone, PartialEq)]
pub struct DocumentContent {
    pub front_matter: Option<String>,
    pub body: String,
}

impl DocumentContent {
    /// Parses a raw document string into front-matter and body.
    pub fn parse(raw: &str) -> Self {
        let content = raw.strip_prefix('\u{feff}').unwrap_or(raw);
        
        // Match behavior similar to parse_front_matter where it must start with ---
        if content.starts_with("---") || content.trim_start().starts_with("---") {
            let parts: Vec<&str> = content.splitn(3, "---").collect();
            if parts.len() == 3 && parts[0].trim().is_empty() {
                // Return the exact front matter block, including delimiters
                let original_fm = format!("---{}---", parts[1]);
                let body = parts[2].to_string();
                
                return Self {
                    front_matter: Some(original_fm),
                    body,
                };
            }
        }
        
        Self {
            front_matter: None,
            body: raw.to_string(),
        }
    }

    /// Combines the front-matter and body into a single string to save.
    pub fn to_string(&self) -> String {
        if let Some(fm) = &self.front_matter {
            format!("{}{}", fm, self.body)
        } else {
            self.body.clone()
        }
    }
}
