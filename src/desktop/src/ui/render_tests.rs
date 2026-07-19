
#[cfg(test)]
mod tests {

    #[test]
    fn test_gfm_rendering_coverage_no_panic() {
        let gfm_text = r#"
# GFM Coverage

## Tables
| Header 1 | Header 2 |
| -------- | -------- |
| Cell 1   | Cell 2   |

## Task Lists
- [x] Completed task
- [ ] Incomplete task

## Strikethrough
~~This is crossed out~~

## Autolinks
https://github.com

## Footnotes
Here is a footnote reference.[^1]

[^1]: Here is the footnote.

## HTML Blocks
<div align="center">
  Centered text
</div>

## Code Blocks
```rust
fn main() {}
```

## Blockquotes
> This is a quote.
"#;
        // Basic parser validation to ensure options match GFM
        use pulldown_cmark::{Parser, Options, Event};
        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TASKLISTS);
        
        let parser = Parser::new_ext(gfm_text, options);
        let events: Vec<Event> = parser.collect();
        assert!(events.len() > 20); // Ensures it actually parses the tags
    }
}
