//! Tier 3 visual regression snapshot tests for CommonMark spec 0.31.2.
//!
//! These tests exercise the full `render_markdown()` rendering pipeline in
//! an `egui_kittest::Harness` with the `wgpu` test renderer enabled, taking
//! pixel-level PNG snapshot comparisons to catch visual regressions (font wrapping,
//! spacing, padding, line heights, border alignment, colors, and layout bounds).
//!
//! Reference: `tests/collateral/commonmark.md`.
//! Each test annotates the CM example numbers it exercises via `[CM-NNN]`.
//!
//! # Why a single harness for every case
//!
//! Building an `egui_kittest::Harness` is the expensive part of a snapshot
//! test: it initialises a `wgpu` renderer, an egui context and the font
//! atlas, then does a sizing pass. When each case had its own test
//! function, every one rebuilt that harness. Amortising the harness over
//! all 19 cases flips the cost from one harness init per input to one
//! harness init total. Each case reuses the same renderer/context; the
//! markdown is swapped via a shared `Rc<RefCell<..>>` and a fresh frame is
//! run before each `snapshot`.

#![cfg(test)]

use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::ui::table_width::DeficitStrategy;
use crate::ui::test_helpers::snapshot::{DEFAULT_VIEWPORT, snapshot_harness};

/// One CommonMark snapshot case: the snapshot file name (the PNG baseline
/// is `tests/snapshots/{name}.png`) and the markdown input rendered.
struct SnapshotCase {
    name: &'static str,
    md: &'static str,
}

/// Every CommonMark 0.31.2 visual snapshot, grouped by spec section.
/// The `name` must stay stable — it is the on-disk baseline identity.
const SNAPSHOT_CASES: &[SnapshotCase] = &[
    // 2.1 Tabs & Whitespace (CM-001 to CM-011)
    // [CM-001, CM-002, CM-004] Tab alignment in code blocks and indents.
    SnapshotCase {
        name: "cm_snapshot_tabs_and_indentation",
        md: "```\n\tfoo\tbaz\t\tbim\n```\n\n    \tfoo\tbaz",
    },
    // 2.4 Backslash Escapes & 2.5 Entity References (CM-012 to CM-041)
    // [CM-012, CM-025] Escaped characters and HTML entity resolution.
    SnapshotCase {
        name: "cm_snapshot_escapes_and_entities",
        md: r"
\*Not emphasized\* \& \[not a link\]

&copy; &auml; &#35; &#1234; &#x2260; &amp; &lt; &gt;
",
    },
    // 4.1 Thematic Breaks (CM-043 to CM-061)
    // [CM-043, CM-047, CM-050] Various valid thematic break styles.
    SnapshotCase {
        name: "cm_snapshot_thematic_breaks",
        md: r"
Paragraph before break.

***

---

___

   - - -

Paragraph after break.
",
    },
    // 4.2 ATX Headings (CM-062 to CM-079)
    // [CM-062, CM-065, CM-068] ATX headings at levels 1-6 with trailing closing hashes.
    SnapshotCase {
        name: "cm_snapshot_atx_headings",
        md: r"
# Heading Level 1
## Heading Level 2
### Heading Level 3
#### Heading Level 4
##### Heading Level 5
###### Heading Level 6

# Heading with trailing hashes ###
## Empty Heading ##
",
    },
    // 4.3 Setext Headings (CM-080 to CM-112)
    // [CM-080, CM-081] Setext headings H1 (=) and H2 (-).
    SnapshotCase {
        name: "cm_snapshot_setext_headings",
        md: r"
Setext Heading Level 1
======================

Setext Heading Level 2
----------------------

Multi-line setext
heading text here
=================
",
    },
    // 4.4 Indented Code Blocks (CM-113 to CM-125)
    // [CM-113, CM-116] Indented code blocks (4 spaces).
    SnapshotCase {
        name: "cm_snapshot_indented_code_blocks",
        md: r#"
Regular paragraph before code.

    fn main() {
        println!("Hello world!");
    }

Regular paragraph after code.
"#,
    },
    // 4.5 Fenced Code Blocks (CM-126 to CM-236)
    // [CM-126, CM-138, CM-142] Fenced code blocks with language tags and backticks/tildes.
    SnapshotCase {
        name: "cm_snapshot_fenced_code_blocks",
        md: r#"
```rust
fn calculate_sum(a: i32, b: i32) -> i32 {
    a + b
}
```

~~~python
def greet(name):
    print(f"Hello, {name}!")
~~~

```
plain text code block without language
```
"#,
    },
    // 4.6 HTML Blocks (CM-237 to CM-295)
    // [CM-237, CM-242] Block HTML rendered as plain text.
    SnapshotCase {
        name: "cm_snapshot_html_blocks",
        md: r#"
<table>
  <tr>
    <td>HTML Table Cell</td>
  </tr>
</table>

<div class="container">
  <p>Raw HTML Block</p>
</div>
"#,
    },
    // 4.7 Link Reference Definitions (CM-296 to CM-330)
    // [CM-296, CM-300] Link reference definitions and resolved links.
    SnapshotCase {
        name: "cm_snapshot_link_ref_defs",
        md: r#"
[Foo bar]: https://example.com/foo "Title for Foo"
[Baz]: /url 'Title for Baz'

Here is [foo bar] link and another [baz] link.
"#,
    },
    // 4.8 Paragraphs & 4.9 Blank Lines (CM-331 to CM-356)
    // [CM-331, CM-354] Paragraph wrapping and vertical spacing between blocks.
    SnapshotCase {
        name: "cm_snapshot_paragraphs_and_spacing",
        md: r"
First paragraph with several words to test text wrapping behavior across multiple lines in the viewport.

Second paragraph after blank line.


Third paragraph after multiple blank lines.
",
    },
    // 5.1 Block Quotes (CM-357 to CM-388)
    // [CM-357, CM-363, CM-368] Single and nested block quotes.
    SnapshotCase {
        name: "cm_snapshot_block_quotes",
        md: r"
> This is a top-level blockquote.
>
> > This is a nested blockquote level 2.
> > With multi-line text inside the quote.
>
> Back to level 1 blockquote.
",
    },
    // 5.2 List Items & 5.3 Lists (CM-389 to CM-539)
    // [CM-389, CM-400, CM-470, CM-500] Unordered, ordered, and nested lists.
    SnapshotCase {
        name: "cm_snapshot_lists_unordered_and_ordered",
        md: r"
- Unordered item 1
- Unordered item 2
  - Sub-item 2.1
  - Sub-item 2.2
- Unordered item 3

1. First ordered item
2. Second ordered item
   1. Sub-ordered item 2.1
   2. Sub-ordered item 2.2
3. Third ordered item
",
    },
    // [CM-475, CM-480] Tight vs loose list spacing.
    SnapshotCase {
        name: "cm_snapshot_lists_tight_vs_loose",
        md: r"
Tight list:
- One
- Two
- Three

Loose list:
- Item one

- Item two

- Item three
",
    },
    // 6.1 Code Spans (CM-540 to CM-574)
    // [CM-540, CM-545] Monospace inline code spans with backticks.
    SnapshotCase {
        name: "cm_snapshot_code_spans",
        md: "Use `let x = 42;` in Rust or `` `code with backtick` `` inside code spans.",
    },
    // 6.2 Emphasis & Strong Emphasis (CM-575 to CM-753)
    // [CM-575, CM-640, CM-650] Italic, bold, combined emphasis, and strikethrough.
    SnapshotCase {
        name: "cm_snapshot_emphasis_and_strong",
        md: r"
*Italic text* and _also italic_.

**Bold text** and __also bold__.

***Combined bold and italic*** text.

~~Strikethrough text fragment~~.
",
    },
    // 6.3 Links, 6.4 Images, 6.5 Autolinks (CM-754 to CM-876)
    // [CM-754, CM-860, CM-865] Links, images, and autolinks.
    SnapshotCase {
        name: "cm_snapshot_links_images_autolinks",
        md: r#"
[Inline Link](https://example.com "Example Domain")

Autolink: <https://rust-lang.org> and email <user@example.com>

![Image Alt Text](https://example.com/logo.png "Logo")
"#,
    },
    // 6.6 Raw HTML, 6.7 Line Breaks, 6.8 Soft Breaks (CM-877 to CM-927)
    // [CM-877, CM-911, CM-922] Inline raw HTML, hard line breaks, and soft line breaks.
    SnapshotCase {
        name: "cm_snapshot_html_and_line_breaks",
        md: "Inline <span>HTML tag</span> rendered as plain text.\n\nFirst line with hard break  \nSecond line after hard break.\n\nSoft break line 1\nSoft break line 2.",
    },
    // 6.9 Textual Content & Symbols (CM-928 to CM-935)
    // [CM-928, CM-930] Unicode characters, symbols, and punctuation.
    SnapshotCase {
        name: "cm_snapshot_textual_content_and_unicode",
        md: "Unicode test: Hello World! 🚀 🦀 💡 — En-dash – Em-dash — Ellipsis… Quotes “hello” ‘world’.",
    },
    // GFM Extension: Tables
    // GFM table rendering snapshot.
    SnapshotCase {
        name: "cm_snapshot_gfm_tables",
        md: r"
| Header 1 | Header 2 | Header 3 |
| :--- | :---: | ---: |
| Left aligned | Centered | Right aligned |
| Cell 4 | Cell 5 | Cell 6 |
",
    },
];

/// Build one wgpu harness and run every CommonMark case through it.
///
/// The renderer, egui context and font atlas are constructed a single
/// time; each case swaps the shared markdown buffer and runs a fresh
/// frame before snapshotting, so the expensive one-time init is paid once
/// for all 19 baselines instead of once per baseline.
#[test]
fn cm_snapshot_all_cases() {
    let currently_rendering = Rc::new(RefCell::new(String::new()));
    let shared_md = Rc::clone(&currently_rendering);
    let mut harness = snapshot_harness("cm_snapshot_all_cases", DEFAULT_VIEWPORT, move |ui| {
        egui::CentralPanel::default().show(ui, |ui| {
            let mut scroll = None;
            let mut toggles = Vec::new();
            render_markdown(
                ui,
                &shared_md.borrow(),
                &mut scroll,
                &mut toggles,
                DeficitStrategy::ProportionalToSlack,
                None,
            );
        });
    });

    for case in SNAPSHOT_CASES {
        *currently_rendering.borrow_mut() = case.md.to_string();
        // Render a fresh frame with the new markdown, wait for any
        // repaints/animations to settle, then snapshot the frame.
        harness.run();
        harness.snapshot(case.name);
    }
}
