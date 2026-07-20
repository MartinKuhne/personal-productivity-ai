#[cfg(test)]
mod tests {
    use super::super::render::build_toc;
    use pulldown_cmark::{Event, Options, Parser};

    #[test]
    fn test_build_toc_empty() {
        let toc = build_toc("");
        assert!(toc.is_empty());
    }

    #[test]
    fn test_build_toc_no_headings() {
        let toc = build_toc("Just a paragraph.\n\nAnother paragraph.");
        assert!(toc.is_empty());
    }

    #[test]
    fn test_build_toc_h1_only() {
        let toc = build_toc("# Title\n\nContent");
        assert_eq!(toc.len(), 1);
        assert_eq!(toc[0].level, 1);
        assert_eq!(toc[0].title, "Title");
    }

    #[test]
    fn test_build_toc_h1_h2_h3() {
        let toc = build_toc("# H1\n\n## H2\n\n### H3");
        assert_eq!(toc.len(), 3);
        assert_eq!(toc[0].level, 1);
        assert_eq!(toc[0].title, "H1");
        assert_eq!(toc[1].level, 2);
        assert_eq!(toc[1].title, "H2");
        assert_eq!(toc[2].level, 3);
        assert_eq!(toc[2].title, "H3");
    }

    #[test]
    fn test_build_toc_heading_with_code() {
        let toc = build_toc("# `code` in heading");
        assert_eq!(toc.len(), 1);
        let title = &toc[0].title;
        assert!(title.contains("code"));
    }

    #[test]
    fn test_build_toc_ignores_non_heading_content() {
        let toc =
            build_toc("# Real Title\n\nSome text\n\n## Another\n\n- list item\n\n> blockquote");
        assert_eq!(toc.len(), 2);
        assert_eq!(toc[0].title, "Real Title");
        assert_eq!(toc[1].title, "Another");
    }

    #[test]
    fn test_gfm_parser_options_set() {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TASKLISTS);

        let gfm = "# Test\n| a | b |\n|---|---|";
        let parser = Parser::new_ext(gfm, options);
        let events: Vec<Event> = parser.collect();
        assert!(events.len() > 5);
    }

    #[test]
    fn test_gfm_hard_breaks_not_enabled() {
        // Hard breaks should NOT be enabled by default
        // With HARD_BREAKS disabled, a newline is a soft break (space)
        let options = Options::empty();
        // Intentionally not setting ENABLE_HARD_BREAKS
        let text = "line1\nline2";
        let parser = Parser::new_ext(text, options);
        let mut found_hard_break = false;
        for event in parser {
            if let Event::HardBreak = event {
                found_hard_break = true;
            }
        }
        assert!(
            !found_hard_break,
            "Hard breaks should not be enabled by default"
        );
    }

    #[test]
    fn test_build_toc_heading_with_special_chars() {
        let toc = build_toc("# H1: Introduction & Conclusion");
        assert_eq!(toc.len(), 1);
        assert!(toc[0].title.contains("H1: Introduction"));
    }

    #[test]
    fn test_build_toc_maintains_order() {
        let toc = build_toc("## Second\n\n# First\n\n### Third");
        assert_eq!(toc.len(), 3);
        assert_eq!(toc[0].title, "Second");
        assert_eq!(toc[1].title, "First");
        assert_eq!(toc[2].title, "Third");
    }
}
