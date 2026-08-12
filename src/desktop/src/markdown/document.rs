//! Document content parsing, front-matter extraction, task list toggles, and document state models.
//!
//! [`Document`] is the parsed-document newtype: one value that carries
//! the raw source, the parsed AST events, the parsed front-matter (if
//! any), and a revision counter. Callers that previously called
//! [`parse_front_matter`] and [`parse_markdown_to_events`] separately
//! on the same content now hold one `Document` and reach both
//! derived views through it.
//!
//! Conventions:
//! - `events()` returns the events parsed from the *body* (source with
//!   the front-matter block stripped), not the full source. This
//!   matches the previous behaviour of every call site that did
//!   `parse_markdown_to_events(&tab_manager.current_markdown)` after
//!   `tab_manager.current_markdown` was set to the body.
//! - `body()` returns the full source when there is no front matter
//!   and the post-`---` body when there is. This is what tools and
//!   renderers should pass to a pure markdown parser.

use crate::markdown::model::RenderEvent;
use crate::markdown::parser::parse_markdown_to_events;
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

/// Parse YAML front matter from markdown content. The returned [`FrontMatter`]
/// gives access to the parsed YAML value, the original YAML source text
/// (preserved verbatim for round-tripping on save), and the body content
/// that follows the closing `---` delimiter.
///
/// Most callers should construct a [`Document`] instead — it pairs
/// the front matter with the parsed AST events so the two cannot
/// drift out of sync.
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

/// The parsed-markdown document — a single value that carries the raw
/// source, the parsed AST events, the parsed front-matter (if any),
/// and a revision counter that bumps on every source change.
///
/// The newtype replaces the previous pattern of threading a
/// `(Option<serde_norway::Value>, &str)` pair — `(yaml, body)` — plus a
/// separately-cached `Vec<RenderEvent>` through every function. Each
/// call site that used to do
/// ```ignore
/// let fm = parse_front_matter(&content);
/// let events = parse_markdown_to_events(fm.as_ref().map(|fm| fm.body.as_str()).unwrap_or(&content));
/// ```
/// now holds one `Document` and reaches both derived views through
/// `doc.front_matter()` and `doc.events()`.
///
/// Conventions (see the module docs for the full list):
/// - `events()` returns events parsed from the *body*, not the
///   full source. This matches the pre-refactor behaviour of every
///   renderer call site that parsed `&tab_manager.current_markdown`.
/// - `body()` returns the full source when there is no front matter
///   and the post-`---` body when there is.
#[derive(Clone, Debug)]
pub struct Document {
    /// The original full source text (including the front-matter block
    /// when present).
    source: String,
    /// Parsed front matter, if the source starts with a valid YAML
    /// `--- ... ---` block. `None` when there is no front matter OR
    /// the YAML fails to parse.
    front_matter: Option<FrontMatter>,
    /// AST events derived from `body()` (the source with the
    /// front-matter block stripped, when present). Refreshed on
    /// every [`Document::update_source`] call.
    events: Vec<RenderEvent>,
    /// Monotonically increasing counter, bumped on every
    /// [`Document::update_source`] call. Callers that previously
    /// hashed the source themselves can use this instead.
    revision: u64,
}

impl Document {
    /// Parses `source` into a `Document`. Both the front-matter and
    /// the AST events are derived in this call; the result is the
    /// single value callers should thread through their pipeline
    /// instead of re-parsing.
    pub fn new(source: String) -> Self {
        let front_matter = parse_front_matter(&source);
        let body = match &front_matter {
            Some(fm) => fm.body.as_str(),
            None => source.as_str(),
        };
        let events = parse_markdown_to_events(body);
        Self {
            source,
            front_matter,
            events,
            revision: 1,
        }
    }

    /// The original full source text. Includes the front-matter
    /// block when present.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// The current revision number. Bumped on every
    /// [`Document::update_source`] and [`Document::toggle_task`]
    /// call that mutates the source. Callers that cache derived
    /// data (heading IDs, etc.) can key the cache on this instead
    /// of hashing the source themselves.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The parsed front matter, or `None` if the source has no
    /// `--- ... ---` block or the YAML inside it fails to parse.
    pub fn front_matter(&self) -> Option<&FrontMatter> {
        self.front_matter.as_ref()
    }

    /// The parsed AST events for the body. Refreshed on every
    /// [`Document::update_source`] call.
    pub fn events(&self) -> &[RenderEvent] {
        &self.events
    }

    /// The body text — the part of the source that follows the
    /// closing `---` of the front-matter block. When there is no
    /// front matter, this is the full source. Renderers and tools
    /// that want "the markdown" should pass this to a pure
    /// markdown parser.
    pub fn body(&self) -> &str {
        match &self.front_matter {
            Some(fm) => fm.body.as_str(),
            None => self.source.as_str(),
        }
    }

    /// Convenience accessor: the parsed YAML value, or `None` when
    /// the document has no front matter.
    pub fn yaml(&self) -> Option<&Value> {
        self.front_matter.as_ref().map(|fm| &fm.yaml)
    }

    /// Replaces the source text and re-parses both the front matter
    /// and the AST events if the source actually changed. The
    /// revision counter bumps on a real change and stays put on a
    /// no-op (so cached derived data keyed on `revision()` stays
    /// valid).
    pub fn update_source(&mut self, new_source: String) {
        if self.source == new_source {
            return;
        }
        self.front_matter = parse_front_matter(&new_source);
        let body = match &self.front_matter {
            Some(fm) => fm.body.as_str(),
            None => new_source.as_str(),
        };
        self.events = parse_markdown_to_events(body);
        self.source = new_source;
        self.revision += 1;
    }

    /// Toggles the checkbox marker for the specified task item and
    /// re-parses the events. The revision counter bumps.
    pub fn toggle_task(&mut self, task_index: usize, checked: bool) {
        // `apply_task_toggle` works on the same buffer it parsed,
        // which for us is the *body* (see `Document::events`).
        // When front matter is present the body lives inside the
        // `FrontMatter` struct; we toggle it there and then
        // re-stitch the source so the verbatim text stays in
        // sync. When front matter is absent, the body IS the
        // source and we toggle in place.
        if let Some(fm) = self.front_matter.as_mut() {
            apply_task_toggle(&mut fm.body, task_index, checked);
            // Re-stitch the source: `parse_front_matter` puts
            // everything between the delimiters into `fm.source`
            // and everything after the closing `---` into
            // `fm.body`, so the verbatim reconstruction is
            // `---{fm.source}---{fm.body}`.
            self.source = format!("---{}---{}", fm.source, fm.body);
        } else {
            apply_task_toggle(&mut self.source, task_index, checked);
        }
        let body = match &self.front_matter {
            Some(fm) => fm.body.as_str(),
            None => self.source.as_str(),
        };
        self.events = parse_markdown_to_events(body);
        self.revision += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_caching_and_revision() {
        let mut doc = Document::new("- [ ] Task 1".to_string());
        assert_eq!(doc.revision(), 1);
        assert_eq!(doc.events().len(), 1);

        // Update with identical source — revision stays unchanged
        doc.update_source("- [ ] Task 1".to_string());
        assert_eq!(doc.revision(), 1);

        // Toggle task — mutates source and increments revision
        doc.toggle_task(0, true);
        assert_eq!(doc.revision(), 2);
        assert_eq!(doc.source(), "- [x] Task 1");
    }

    #[test]
    fn test_document_body_is_source_without_front_matter() {
        let doc = Document::new("# Hello".to_string());
        assert!(doc.front_matter().is_none());
        assert_eq!(doc.body(), "# Hello");
        assert_eq!(doc.source(), "# Hello");
        assert_eq!(doc.yaml(), None);
    }

    #[test]
    fn test_document_body_is_post_delimiter_with_front_matter() {
        let doc = Document::new("---\ntitle: T\n---\n# Body".to_string());
        let fm = doc.front_matter().expect("front matter should parse");
        assert_eq!(fm.yaml["title"].as_str(), Some("T"));
        assert_eq!(doc.body(), "\n# Body");
        // Source preserves the delimiters verbatim.
        assert_eq!(doc.source(), "---\ntitle: T\n---\n# Body");
    }

    #[test]
    fn test_document_invalid_yaml_front_matter_is_treated_as_no_front_matter() {
        // Malformed YAML is not a valid front-matter block; the
        // whole source becomes the body.
        let doc = Document::new("---\ninvalid: [unclosed\n---\nBody".to_string());
        assert!(doc.front_matter().is_none());
        assert_eq!(doc.body(), "---\ninvalid: [unclosed\n---\nBody");
    }

    #[test]
    fn test_document_update_source_reparses_front_matter() {
        let mut doc = Document::new("body".to_string());
        assert!(doc.front_matter().is_none());
        doc.update_source("---\ntitle: T2\n---\nbody2".to_string());
        assert!(doc.front_matter().is_some());
        assert_eq!(
            doc.front_matter().unwrap().yaml["title"].as_str(),
            Some("T2")
        );
        assert_eq!(doc.body(), "\nbody2");
        assert_eq!(doc.revision(), 2);
    }

    #[test]
    fn test_document_toggle_task_in_body_with_front_matter() {
        // When the front matter is present, the task list lives in
        // the body, not the source. `toggle_task` must still flip
        // the marker and re-parse the events.
        let mut doc = Document::new("---\ntitle: T\n---\n- [ ] todo".to_string());
        assert_eq!(doc.body(), "\n- [ ] todo");
        doc.toggle_task(0, true);
        // Body now contains the toggled marker.
        assert_eq!(doc.body(), "\n- [x] todo");
        // Source is the verbatim text with delimiters.
        assert_eq!(doc.source(), "---\ntitle: T\n---\n- [x] todo");
        assert_eq!(doc.revision(), 2);
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

#[cfg(test)]
#[path = "document_proptests.rs"]
mod document_proptests;
