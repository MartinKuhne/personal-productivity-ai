//! Document content parsing, front-matter extraction, task list toggles, and document state models.

use crate::markdown::ast::RenderEvent;
use crate::markdown::parser::parse_markdown_to_events;
use serde_yaml::Value;

/// The result of successfully parsing a YAML front matter block.
#[derive(Debug)]
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
        if let Ok(yaml) = serde_yaml::from_str::<Value>(yaml_source) {
            return Some(FrontMatter {
                yaml,
                source: yaml_source.to_string(),
                body: body.to_string(),
            });
        }
    }
    None
}

/// Toggles the checkbox marker for the Nth task list item in the
/// markdown source. Called after rendering when the user clicks a
/// task checkbox, so the change persists across re-parses.
pub fn apply_task_toggle(markdown: &mut String, task_index: usize, checked: bool) {
    use pulldown_cmark::{Event, Options, Parser};

    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(markdown, options).into_offset_iter();
    let new_marker = if checked { "[x]" } else { "[ ]" };
    let mut count = 0usize;

    for (event, range) in parser {
        if let Event::TaskListMarker(_) = event {
            if count == task_index {
                let slice = &markdown[range.clone()];
                let offset = slice
                    .find('[')
                    .or_else(|| markdown[range.start..].find('['));
                if let Some(off) = offset {
                    let start = range.start + off;
                    if start + 3 <= markdown.len() {
                        markdown.replace_range(start..start + 3, new_marker);
                    }
                }
                return;
            }
            count += 1;
        }
    }
}

/// A reactive container for a Markdown document's source text, parsed AST events,
/// and revision state. Bypasses re-parsing when content is unchanged.
#[derive(Clone, Debug)]
pub struct DocumentModel {
    source: String,
    revision: u64,
    events: Vec<RenderEvent>,
}

impl DocumentModel {
    /// Creates a new `DocumentModel` from a raw Markdown source string.
    pub fn new(source: String) -> Self {
        let events = parse_markdown_to_events(&source);
        Self {
            source,
            revision: 1,
            events,
        }
    }

    /// Returns the current Markdown source string.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns the current revision number.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Returns the cached sequence of render events.
    pub fn events(&self) -> &[RenderEvent] {
        &self.events
    }

    /// Updates the Markdown source text and increments revision counter if content changed.
    pub fn update_source(&mut self, new_source: String) {
        if self.source != new_source {
            self.events = parse_markdown_to_events(&new_source);
            self.source = new_source;
            self.revision += 1;
        }
    }

    /// Toggles the checkbox marker for the specified task item.
    pub fn toggle_task(&mut self, task_index: usize, checked: bool) {
        apply_task_toggle(&mut self.source, task_index, checked);
        self.events = parse_markdown_to_events(&self.source);
        self.revision += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_model_caching_and_revision() {
        let mut model = DocumentModel::new("- [ ] Task 1".to_string());
        assert_eq!(model.revision(), 1);
        assert_eq!(model.events().len(), 1);

        // Update with identical source — revision stays unchanged
        model.update_source("- [ ] Task 1".to_string());
        assert_eq!(model.revision(), 1);

        // Toggle task — mutates source and increments revision
        model.toggle_task(0, true);
        assert_eq!(model.revision(), 2);
        assert_eq!(model.source(), "- [x] Task 1");
    }

    #[test]
    fn test_parse_front_matter_basic() {
        let content = "---\ntitle: Test Document\nauthor: John Doe\n---\n# Hello World";
        let fm = parse_front_matter(content).unwrap();
        assert_eq!(fm.yaml["title"].as_str(), Some("Test Document"));
        assert_eq!(fm.yaml["author"].as_str(), Some("John Doe"));
        assert_eq!(fm.body.trim(), "# Hello World");
    }
}
