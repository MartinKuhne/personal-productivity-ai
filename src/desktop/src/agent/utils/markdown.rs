use serde_norway::Value;

/// The result of successfully parsing a YAML front matter block.
#[derive(Clone, Debug)]
pub struct FrontMatter {
    /// The parsed YAML value.
    pub yaml: Value,
    /// The original YAML source text between the `---` delimiters.
    pub source: String,
    /// The body content: everything after the closing `---` delimiter.
    pub body: String,
}

/// Parse YAML front matter from markdown content.
pub fn parse_front_matter(content: &str) -> Option<FrontMatter> {
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let parts: Vec<&str> = content.splitn(3, "---").collect();
    if parts.len() == 3 && parts[0].trim().is_empty() {
        let yaml_source = parts[1];
        let body = parts[2];
        if let Ok(yaml) = serde_norway::from_str::<Value>(yaml_source) {
            return Some(FrontMatter {
                yaml,
                source: yaml_source.to_string(),
                body: body.to_string(),
            });
        }
    }
    None
}
