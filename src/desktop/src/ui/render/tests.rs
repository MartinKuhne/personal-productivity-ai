//! Parser / structural tests for the render module.
//!
//! These tests exercise [`parse_markdown_to_events`], [`parse_yaml_to_pairs`],
//! [`build_toc`], and related helpers without going through egui. The
//! end-to-end render tests (with `ctx.run_ui` and shape inspection) live
//! in the `e2e_tests` submodule.

use super::*;

// The `pending_toggles` argument on `apply_task_toggle` is what allows a
// task checkbox, so the change persists across re-parses.

#[test]
fn test_parse_yaml_to_pairs() {
    let yaml_str = "key1: value1\nkey2: [item1, item2]\nkey3: 100\nkey4: true";
    let val: serde_norway::Value = serde_norway::from_str(yaml_str).unwrap();
    let pairs = parse_yaml_to_pairs(&val).unwrap();
    assert_eq!(pairs[0], ("key1".to_string(), "value1".to_string()));
    assert_eq!(pairs[1], ("key2".to_string(), "item1, item2".to_string()));
    assert_eq!(pairs[2], ("key3".to_string(), "100".to_string()));
    assert_eq!(pairs[3], ("key4".to_string(), "true".to_string()));
}

#[test]
fn test_parse_yaml_to_pairs_non_mapping() {
    let string_val = serde_norway::Value::String("just string".to_string());
    assert_eq!(parse_yaml_to_pairs(&string_val), None);

    let seq_val =
        serde_norway::Value::Sequence(vec![serde_norway::Value::String("item".to_string())]);
    assert_eq!(parse_yaml_to_pairs(&seq_val), None);

    let null_val = serde_norway::Value::Null;
    assert_eq!(parse_yaml_to_pairs(&null_val), None);
}

#[test]
fn test_parse_markdown_to_events() {
    // Uses structural lookups (find / filter) rather than indexed
    // access so the test doesn't break when events are reordered or
    // when the parser gains a new event type between existing ones.
    let md = "# Heading 1\nSome *text*\n- List item";
    let events = parse_markdown_to_events(md);

    // H1 heading must be present, regardless of position.
    assert!(
        events.iter().any(|e| matches!(
            e,
            RenderEvent::Heading { level: 1, elems } if heading_plain_text(elems) == "Heading 1"
        )),
        "missing H1 'Heading 1' in {events:?}"
    );

    // A FlushInline carrying "Some " (not italic) followed by "text"
    // (italic) — this is the paragraph that mixes emphasis.
    let paragraph = events.iter().find_map(|e| match e {
        RenderEvent::FlushInline {
            elems,
            needs_bullet: false,
            ..
        } if !elems.is_empty() => Some(elems),
        _ => None,
    });
    let elems = paragraph.expect("expected a non-bullet FlushInline for the paragraph");

    // The previous version of this test asserted on `elems[0]` /
    // `elems[1]`, which is fragile: a refactor that splits or merges
    // inline elements would fail the test even though no real
    // behaviour changed. The structural check below verifies the
    // same contract ("the paragraph mixes plain and italic
    // emphasis") without depending on element ordering.
    let plain_some = elems.iter().any(|e| {
        matches!(
            e,
            InlineElem::Text(t, style) if t == "Some " && !style.italic
        )
    });
    let italic_text = elems.iter().any(|e| {
        matches!(
            e,
            InlineElem::Text(t, style) if t == "text" && style.italic
        )
    });
    assert!(
        plain_some,
        "paragraph must contain a plain-text 'Some ' inline elem: {elems:?}"
    );
    assert!(
        italic_text,
        "paragraph must contain an italic 'text' inline elem: {elems:?}"
    );

    // The paragraph's trailing space event.
    assert!(
        events.iter().any(|e| matches!(e, RenderEvent::Space(4.0))),
        "missing Space(4.0) event in {events:?}"
    );

    // The bulleted list item, at indent 1.
    let list_item = events.iter().find_map(|e| match e {
        RenderEvent::FlushInline {
            elems,
            needs_bullet: true,
            indent: 1,
            ..
        } => Some(elems),
        _ => None,
    });
    let elems = list_item.expect("expected a bulleted FlushInline at indent 1");
    assert_eq!(elems.len(), 1, "list item should have 1 inline elem");
    match &elems[0] {
        InlineElem::Text(t, _) => assert_eq!(t, "List item"),
        other => panic!("expected 'List item' text, got {other:?}"),
    }
}

#[test]
fn test_parse_markdown_heading_levels() {
    // Structural check: every level 1..=4 appears with the right text.
    // Doesn't depend on event ordering or extra events between them.
    let md = "# H1\n## H2\n### H3\n#### H4";
    let events = parse_markdown_to_events(md);
    for (level, text) in [(1, "H1"), (2, "H2"), (3, "H3"), (4, "H4")] {
        assert!(
            events.iter().any(|e| matches!(
                e,
                RenderEvent::Heading { level: l, elems } if *l == level && heading_plain_text(elems) == text
            )),
            "missing H{level} '{text}' in {events:?}"
        );
    }
}

#[test]
fn test_parse_markdown_code_block() {
    let md = "```rust\nfn main() {\n    println!(\"Hello\");\n}\n```";
    let events = parse_markdown_to_events(md);
    assert_eq!(events.len(), 1);
    match &events[0] {
        RenderEvent::CodeBlock { content, language } => {
            assert!(content.contains("fn main()"));
            assert_eq!(language.as_deref(), Some("rust"));
        }
        _ => panic!("Expected CodeBlock event"),
    }
}

#[test]
fn test_parse_markdown_inline_elements() {
    let md = "**bold** *italic* ~~strikethrough~~ `code` [link](https://example.com) ![img](https://example.com/a.jpg)";
    let events = parse_markdown_to_events(md);
    assert!(!events.is_empty());
    match &events[0] {
        RenderEvent::FlushInline { elems, .. } => {
            let mut has_bold = false;
            let mut has_italic = false;
            let mut has_strikethrough = false;
            let mut has_code = false;
            let mut has_link = false;
            let mut has_image = false;

            for elem in elems {
                match elem {
                    InlineElem::Text(t, style) => {
                        if t == "bold" && style.bold {
                            has_bold = true;
                        }
                        if t == "italic" && style.italic {
                            has_italic = true;
                        }
                        if t == "strikethrough" && style.strikethrough {
                            has_strikethrough = true;
                        }
                        if t == "code" && style.code {
                            has_code = true;
                        }
                    }
                    InlineElem::Link(url, text) => {
                        if url == "https://example.com" && text == "link" {
                            has_link = true;
                        }
                    }
                    InlineElem::Image(url) if url == "https://example.com/a.jpg" => {
                        has_image = true;
                    }
                    _ => {}
                }
            }
            assert!(has_bold, "Missing bold element");
            assert!(has_italic, "Missing italic element");
            assert!(has_strikethrough, "Missing strikethrough element");
            assert!(has_code, "Missing code element");
            assert!(has_link, "Missing link element");
            assert!(has_image, "Missing image element");
        }
        _ => panic!("Expected FlushInline"),
    }
}

#[test]
fn test_parse_markdown_task_list() {
    let md = "- [ ] Task 1\n- [x] Task 2";
    let events = parse_markdown_to_events(md);

    let mut found_unchecked = false;
    let mut found_checked = false;

    for ev in &events {
        if let RenderEvent::FlushInline {
            task_checked,
            elems,
            ..
        } = ev
        {
            if let Some(false) = task_checked
                && elems.iter().any(|e| match e {
                    InlineElem::Text(t, _) => t == "Task 1",
                    _ => false,
                })
            {
                found_unchecked = true;
            }
            if let Some(true) = task_checked
                && elems.iter().any(|e| match e {
                    InlineElem::Text(t, _) => t == "Task 2",
                    _ => false,
                })
            {
                found_checked = true;
            }
        }
    }
    assert!(found_unchecked, "Missing unchecked task");
    assert!(found_checked, "Missing checked task");
}

#[test]
fn test_parse_markdown_table() {
    let md = "| Col A | Col B |\n|---|---|\n| Val A | Val B |";
    let events = parse_markdown_to_events(md);

    let mut found_table = false;
    for ev in events {
        if let RenderEvent::Table(rows) = ev {
            found_table = true;
            assert_eq!(rows.len(), 2); // Header row + 1 data row
            assert_eq!(rows[0].len(), 2);
            assert_eq!(rows[1].len(), 2);
        }
    }
    assert!(found_table, "Expected Table event");
}

#[test]
fn test_parse_markdown_table_empty_cells() {
    let md = "| A | | C |\n|---|---|---|\n| | B | |";
    let events = parse_markdown_to_events(md);

    let mut found_table = false;
    for ev in events {
        if let RenderEvent::Table(rows) = ev {
            found_table = true;
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0].len(), 3);
            assert_eq!(rows[1].len(), 3);
            assert!(rows[0][1].is_empty(), "Header cell 1 should be empty");
            assert!(rows[1][0].is_empty(), "Data cell 0 should be empty");
            assert!(rows[1][2].is_empty(), "Data cell 2 should be empty");
        }
    }
    assert!(found_table, "Expected Table event");
}

#[test]
fn test_parse_markdown_table_with_bold_and_special_chars() {
    let md = "| Name | Account | Amount | Type |\n|---|---|---|---|\n| **Vanguard** | #12345678 | $1 | Taxable (investment) |";
    let events = parse_markdown_to_events(md);

    let mut found_table = false;
    for ev in events {
        if let RenderEvent::Table(rows) = ev {
            found_table = true;
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0].len(), 4);
            assert_eq!(rows[1].len(), 4);
            let vanguard_cell = &rows[1][0];
            assert_eq!(vanguard_cell.len(), 1);
            match &vanguard_cell[0] {
                InlineElem::Text(t, style) => {
                    assert_eq!(t, "Vanguard");
                    assert!(style.bold, "Vanguard should be bold");
                }
                _ => panic!("Expected Text element"),
            }
        }
    }
    assert!(found_table, "Expected Table event");
}

/// Regression test for the "Laptops" table that drives the FTWA pipeline
/// at production width. This test pins down the *parser's* output for
/// the exact markdown source so any regression in `parse_markdown_to_events`
/// surfaces here instead of as a layout failure in the table-width
/// algorithm. The downstream layout passes this AST to `ftwa`; if the
/// AST is wrong, every algorithm (FTWA + the three survey ones) fails
/// on the same root cause, so the bug needs to be caught at the
/// parser boundary.
///
/// Markdown source (the production table that previously broke every
/// algorithm — all of them point at the pipeline rather than the math):
///
/// ```text
/// | Make | Model and Model Number | Market Price | Display | Processor | PassMark Single / Multi | Summary |
/// |------|----------------------|-------------|---------|-----------|------------------------|---------|
/// | Acer | Swift 16 AI (SF16-71T) | $1,249-$1,799 | 16" 3K (2880x1800) 120Hz OLED Touch | Intel Core Ultra 7 256V (8C/8T Lunar Lake) | ~4,031 / ~19,000 | Excellent value. ... |
/// ```
///
/// Expected AST shape: 2 rows (header + 1 data row), 7 cells per row,
/// each cell is a single plain-text `InlineElem::Text(_, TextStyle::default())`.
/// The 7th column ("Summary") is a long prose cell; the parser must
/// not break, split, or wrap it.
///
/// Note: the test asserts structural shape (row/cell counts) AND
/// content. The content assertion is the contract — if the parser
/// drops a cell, miscounts a column, or splits a cell on a special
/// character (`"`, `(`, `/`, `~`, `$`), this test fails with a clear
/// diff. The downstream algorithms can then be debugged separately.
///
/// **Known bug at the time of writing**: the parser's interpretation
/// of GFM strikethrough (`~text~`) interacts with table cells: cells
/// 5 (`~4,031 / ~19,000`) and 6 (`...lightweight at ~3.3 lbs...`) get
/// fragmented into multiple `InlineElem::Text` entries at each `~`.
/// This test fails on those cells today, which is the regression
/// signal we want. Fix the parser to preserve cell content as a
/// single text element (either by treating `~` as literal text inside
/// table cells, or by coalescing strikethrough spans into one element
/// with the style flag set) and this test goes green.
#[test]
fn test_parse_laptops_table_ast_shape() {
    let md = "| Make | Model and Model Number | Market Price | Display | Processor | PassMark Single / Multi | Summary |\n\
              |------|----------------------|-------------|---------|-----------|------------------------|---------|\n\
              | Acer | Swift 16 AI (SF16-71T) | $1,249-$1,799 | 16\" 3K (2880x1800) 120Hz OLED Touch | Intel Core Ultra 7 256V (8C/8T Lunar Lake) | ~4,031 / ~19,000 | Excellent value. Vibrant OLED display, exceptional battery life for a 16\" laptop, lightweight at ~3.3 lbs. Two Thunderbolt 4 ports. Praised by ZDNet, PCMag, and Notebookcheck. Great everyday performance and portability. |";
    let events = parse_markdown_to_events(md);

    // Find the table. There is exactly one in the source.
    let mut found_table: Option<Vec<Vec<Vec<InlineElem>>>> = None;
    for ev in &events {
        if let RenderEvent::Table(rows) = ev {
            assert!(
                found_table.is_none(),
                "expected exactly one table in the source; got a second with {} rows",
                rows.len()
            );
            found_table = Some(rows.clone());
        }
    }
    let rows = found_table.expect("Expected Table event for the laptops table");

    // Collect every per-cell failure into a single report so a future
    // bug doesn't get masked by the first panic. The expected bug at
    // the time of writing is cells 5 and 6 (the `~` characters get
    // interpreted as strikethrough delimiters by pulldown-cmark and
    // fragment the cell into multiple `InlineElem::Text` entries).
    let mut errors: Vec<String> = Vec::new();

    // Shape: 2 rows × 7 cells. The separator row is consumed by the
    // parser and does not appear in the AST (pulldown-cmark strips it
    // before the TableCell/TableRow events fire), so we expect header
    // + 1 data row = 2 rows.
    if rows.len() != 2 {
        errors.push(format!(
            "shape: expected 2 rows (header + 1 data); got {}",
            rows.len()
        ));
    }
    for (i, row) in rows.iter().enumerate() {
        if row.len() != 7 {
            let cell_texts: Vec<String> = row.iter().map(|c| cell_text(c)).collect();
            errors.push(format!(
                "shape: row {i} expected 7 cells; got {} (cells: {:?})",
                row.len(),
                cell_texts
            ));
        }
    }

    // Header row — each cell is a single plain-text InlineElem.
    let expected_header = [
        "Make",
        "Model and Model Number",
        "Market Price",
        "Display",
        "Processor",
        "PassMark Single / Multi",
        "Summary",
    ];
    for (j, expected) in expected_header.iter().enumerate() {
        check_cell(&rows[0][j], expected, "header", j, &mut errors);
    }

    // Data row — each cell is a single plain-text InlineElem. The
    // special characters (`"`, `(`, `/`, `~`, `$`) inside individual
    // cells must NOT split the cell, must NOT be interpreted as
    // markdown syntax, and must be preserved verbatim.
    let expected_data = [
        "Acer",
        "Swift 16 AI (SF16-71T)",
        "$1,249-$1,799",
        "16\" 3K (2880x1800) 120Hz OLED Touch",
        "Intel Core Ultra 7 256V (8C/8T Lunar Lake)",
        "~4,031 / ~19,000",
        "Excellent value. Vibrant OLED display, exceptional battery life for a 16\" laptop, lightweight at ~3.3 lbs. Two Thunderbolt 4 ports. Praised by ZDNet, PCMag, and Notebookcheck. Great everyday performance and portability.",
    ];
    for (j, expected) in expected_data.iter().enumerate() {
        check_cell(&rows[1][j], expected, "data", j, &mut errors);
    }

    if !errors.is_empty() {
        panic!(
            "laptops table AST does not match expected shape/content ({} issue{}):\n  - {}",
            errors.len(),
            if errors.len() == 1 { "" } else { "s" },
            errors.join("\n  - ")
        );
    }
}

/// Assert a single cell's content and shape. Pushes a descriptive
/// error message into `errors` on mismatch (does not panic) so the
/// outer test can report all failures together.
fn check_cell(
    cell: &[InlineElem],
    expected_text: &str,
    row_label: &str,
    j: usize,
    errors: &mut Vec<String>,
) {
    let actual = cell_text(cell);
    if actual != expected_text {
        errors.push(format!(
            "{row_label} cell {j}: expected text {expected_text:?}, got {actual:?}"
        ));
    }
    if cell.len() != 1 {
        errors.push(format!(
            "{row_label} cell {j}: expected 1 InlineElem (single plain-text cell); \
             got {} elems: {cell:?}",
            cell.len()
        ));
        // Don't return — we still want to assert the type of the
        // first elem below (which is the most likely "real" value).
    }
    if let Some(first) = cell.first() {
        match first {
            InlineElem::Text(_, style) => {
                if style != &TextStyle::default() {
                    errors.push(format!(
                        "{row_label} cell {j}: expected default TextStyle on first elem; got {style:?}"
                    ));
                }
            }
            other => errors.push(format!(
                "{row_label} cell {j}: first elem should be InlineElem::Text; got {other:?}"
            )),
        }
    }
}

/// Concatenate the plain-text content of a cell's inlines. Used by
/// the laptops-table test for readable failure messages.
fn cell_text(cell: &[InlineElem]) -> String {
    let mut s = String::new();
    for e in cell {
        match e {
            InlineElem::Text(t, _) => s.push_str(t),
            InlineElem::Link(_, t) => s.push_str(t),
            InlineElem::Image(url) => s.push_str(&format!("[Image: {url}]")),
            InlineElem::Html(h) => s.push_str(h),
            InlineElem::SoftBreak => s.push(' '),
        }
    }
    s
}

#[test]
fn test_parse_markdown_rule_and_blockquote() {
    let md = "---\n\n> Quote text";
    let events = parse_markdown_to_events(md);

    let has_rule = events.iter().any(|e| matches!(e, RenderEvent::Separator));
    assert!(has_rule, "Expected Separator event");

    let has_quote = events.iter().any(|e| match e {
        RenderEvent::FlushInline { elems, .. } => elems.iter().any(|elem| match elem {
            InlineElem::Text(t, _) => t.contains("Quote text"),
            _ => false,
        }),
        _ => false,
    });
    assert!(has_quote, "Expected blockquote text");
}

#[test]
fn test_format_delegate_trace_uses_double_angle() {
    use crate::agent::events::DelegateToolCall;
    use serde_json::json;
    let tool_calls = vec![DelegateToolCall {
        name: "web_fetch".to_string(),
        args: json!({"url": "https://example.com"}),
        result: json!({"status": "success"}),
    }];
    let msg = crate::ui::render::agent_render::format_delegate_trace(&tool_calls);
    assert!(msg.starts_with(">> "), "Expected >> prefix, got: {msg}");
    assert!(
        msg.contains("**Executing tool `web_fetch`**"),
        "Expected tool name"
    );
    assert!(!msg.contains("<span>"), "Should not contain HTML");
}

#[test]
fn test_parse_markdown_html_and_footnotes() {
    let md = "<span>Inline HTML</span>\n\nFootnote[^1]\n\n[^1]: Footnote details";
    let events = parse_markdown_to_events(md);

    let has_html = events.iter().any(|e| match e {
        RenderEvent::FlushInline { elems, .. } => {
            elems.iter().any(|elem| matches!(elem, InlineElem::Html(_)))
        }
        _ => false,
    });
    assert!(has_html, "Expected Html inline element");

    let has_fn_ref = events.iter().any(|e| match e {
        RenderEvent::FlushInline { elems, .. } => elems.iter().any(|elem| match elem {
            InlineElem::Text(t, _) => t.contains("[^1]"),
            _ => false,
        }),
        _ => false,
    });
    assert!(has_fn_ref, "Expected footnote reference");
}

#[test]
fn test_build_toc() {
    // Covers the full matrix: empty, missing headings, single and
    // multiple levels (H1..H6), code-in-heading, special chars,
    // and the order of headings in the source.
    let md = "# Title\nSome text\n## Subtitle";
    let toc = build_toc(md);
    assert_eq!(toc.len(), 2);
    assert_eq!(toc[0].title, "Title");
    assert_eq!(toc[0].level, 1);
    assert_eq!(toc[1].title, "Subtitle");
    assert_eq!(toc[1].level, 2);

    assert!(
        build_toc("").is_empty(),
        "empty input must produce empty TOC"
    );
    assert!(
        build_toc("Just a paragraph.\n\nAnother paragraph.").is_empty(),
        "no-heading input must produce empty TOC"
    );

    let h1 = build_toc("# Title\n\nContent");
    assert_eq!(h1.len(), 1);
    assert_eq!(h1[0].level, 1);
    assert_eq!(h1[0].title, "Title");

    let mixed = build_toc("# H1\n\n## H2\n\n### H3");
    assert_eq!(mixed.len(), 3);
    assert_eq!(mixed[0].level, 1);
    assert_eq!(mixed[0].title, "H1");
    assert_eq!(mixed[1].level, 2);
    assert_eq!(mixed[1].title, "H2");
    assert_eq!(mixed[2].level, 3);
    assert_eq!(mixed[2].title, "H3");

    let deep = build_toc("# H1\n\n#### H4\n\n##### H5\n\n###### H6");
    assert_eq!(deep.len(), 4);
    assert_eq!(deep[1].level, 4);
    assert_eq!(deep[2].level, 5);
    assert_eq!(deep[3].level, 6);

    let code_in_heading = build_toc("# `code` in heading");
    assert_eq!(code_in_heading.len(), 1);
    assert!(code_in_heading[0].title.contains("code"));

    let ignored =
        build_toc("# Real Title\n\nSome text\n\n## Another\n\n- list item\n\n> blockquote");
    assert_eq!(ignored.len(), 2);
    assert_eq!(ignored[0].title, "Real Title");
    assert_eq!(ignored[1].title, "Another");

    let order = build_toc("## Second\n\n# First\n\n### Third");
    assert_eq!(order.len(), 3);
    // Headings appear in source order, not sorted by level.
    assert_eq!(order[0].title, "Second");
    assert_eq!(order[1].title, "First");
    assert_eq!(order[2].title, "Third");

    let special = build_toc("# H1: Introduction & Conclusion");
    assert_eq!(special.len(), 1);
    assert!(special[0].title.contains("H1: Introduction"));
}

#[test]
fn test_parse_edge_cases_expose_quirks() {
    // Targeted probes for known-fragile areas. Each assertion captures
    // the expected behavior; a failure here is a parser defect.

    // Empty input must produce zero events (no spurious separators).
    assert_eq!(
        parse_markdown_to_events(""),
        vec![],
        "empty input should produce no events"
    );

    // Whitespace-only input must produce zero events.
    assert_eq!(
        parse_markdown_to_events("   \n\n\n"),
        vec![],
        "whitespace input should produce no events"
    );

    // A table with all empty cells must have all rows with N cells.
    let events = parse_markdown_to_events("| | | |\n|---|---|---|\n");
    for ev in &events {
        if let RenderEvent::Table(rows) = ev {
            for (i, row) in rows.iter().enumerate() {
                assert_eq!(row.len(), 3, "empty-cell table row {i} should have 3 cells");
            }
        }
    }

    // A table where the separator has fewer columns than the header
    // must still produce a rectangular table — pulldown-cmark normalizes
    // this. If the parser blindly concatenates, the row would be ragged.
    let events = parse_markdown_to_events("| a | b | c |\n|---|---|\n| 1 | 2 | 3 |");
    for ev in &events {
        if let RenderEvent::Table(rows) = ev {
            for (i, row) in rows.iter().enumerate() {
                assert!(
                    row.iter().all(|c| c.len() == row.len()),
                    "mismatched-col table row {i} has inconsistent cell count: {:?}",
                    row.iter().map(Vec::len).collect::<Vec<_>>()
                );
            }
        }
    }

    // Nested lists: every FlushInline must have `indent` ≤ the input's
    // list depth. A 3-deep nested list should produce indents up to 3.
    let events = parse_markdown_to_events("- a\n  - b\n    - c\n- d");
    for ev in &events {
        if let RenderEvent::FlushInline { indent, .. } = ev {
            assert!(*indent <= 3, "3-deep nested list produced indent={indent}");
        }
    }

    // Heading inside a blockquote: the heading must still emit a
    // Heading event, not be swallowed by the blockquote handling.
    let events = parse_markdown_to_events("> # heading in quote");
    assert!(
        events.iter().any(|e| matches!(
            e,
            RenderEvent::Heading { level: 1, elems } if heading_plain_text(elems).contains("heading in quote")
        )),
        "heading inside blockquote was lost: {events:?}"
    );
}

#[test]
fn test_parse_suspicious_paths() {
    // These probe paths the existing tests don't exercise.
    // Each captures an expected invariant; failure = parser bug.

    // Empty link: `[text]()` should produce a Link with empty URL.
    let events = parse_markdown_to_events("[text]()");
    assert!(
        events.iter().any(|e| matches!(
            e,
            RenderEvent::FlushInline { elems, .. } if elems.iter().any(|el| matches!(
                el,
                InlineElem::Link(url, text) if url.is_empty() && text == "text"
            ))
        )),
        "empty-URL link lost: {events:?}"
    );

    // Empty code block: ```\n``` should produce a CodeBlock with
    // empty content, not be dropped entirely.
    let events = parse_markdown_to_events("```\n```");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, RenderEvent::CodeBlock { content, .. } if content.is_empty())),
        "empty code block lost: {events:?}"
    );

    // Image in heading: `# ![alt](url)` — image must not be dropped.
    let events = parse_markdown_to_events("# ![alt text](https://x/y.png)");
    assert!(
        events.iter().any(|e| matches!(
            e,
            RenderEvent::Heading { level: 1, elems } if heading_plain_text(elems).contains("alt text")
        )),
        "image alt text lost from heading: {events:?}"
    );

    // Heading immediately followed by heading: `# A\n# B`
    let events = parse_markdown_to_events("# A\n# B");
    let headings: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            RenderEvent::Heading { level, elems } => Some((*level, heading_plain_text(elems))),
            _ => None,
        })
        .collect();
    assert_eq!(
        headings,
        vec![(1, "A".to_string()), (1, "B".to_string())],
        "consecutive headings: {events:?}"
    );

    // An empty list `- ` (item with no text). The parser should still
    // emit a FlushInline (with empty elems but bullet) so the bullet
    // gets rendered. The current `push_inline` helper skips when
    // `elems.is_empty() && !needs_bullet && task_checked.is_none()` —
    // but `needs_bullet` is true here, so the bullet *should* render.
    let events = parse_markdown_to_events("- ");
    assert!(
        events.iter().any(|e| matches!(
            e,
            RenderEvent::FlushInline {
                needs_bullet: true,
                ..
            }
        )),
        "empty list item lost: {events:?}"
    );

    // A table with only the header row, no data rows. The Table event
    // should still emit (with 1 row), not be dropped.
    let events = parse_markdown_to_events("| H1 | H2 |\n|---|---|\n");
    let table_event = events.iter().find_map(|e| {
        if let RenderEvent::Table(rows) = e {
            Some(rows.len())
        } else {
            None
        }
    });
    assert_eq!(
        table_event,
        Some(1),
        "header-only table dropped: {events:?}"
    );
}

// `# *italic*`, `# **bold**`, `# `code``, `# ~~strike~~`, and
/// `# [link](url)` all previously lost their inline formatting
/// because `RenderEvent::Heading` stored `text: String` (plain
/// concatenation) rather than `elems: Vec<InlineElem>`. The struct
/// now carries the styled elements; the renderer renders each
/// span with the heading's size and weight. These tests pin the
/// expected contract end-to-end. Each row of the table is one
/// case the old single-test-per-style version covered; the
/// closure asserts that the heading produced by `md_source`
/// contains an inline element satisfying `style_predicate`.
#[test]
fn test_heading_preserves_inline_formatting() {
    // One assertion predicate per case. Adding a new inline
    // formatting (e.g. underline) means adding one row here,
    // not a new 25-line `#[test] fn`.
    type StylePredicate = Box<dyn Fn(&InlineElem) -> bool>;
    let cases: &[(&str, &str, StylePredicate)] = &[
        (
            "# *hello*",
            "italic",
            Box::new(|e| matches!(e, InlineElem::Text(_, s) if s.italic)),
        ),
        (
            "# **loud**",
            "bold",
            Box::new(|e| matches!(e, InlineElem::Text(_, s) if s.bold)),
        ),
        (
            "# `code` in heading",
            "code",
            Box::new(|e| matches!(e, InlineElem::Text(_, s) if s.code)),
        ),
        (
            "# ~~old~~",
            "strikethrough",
            Box::new(|e| matches!(e, InlineElem::Text(_, s) if s.strikethrough)),
        ),
        (
            "# [click](https://example.com)",
            "link",
            Box::new(
                |e| matches!(e, InlineElem::Link(url, text) if url == "https://example.com" && text == "click"),
            ),
        ),
    ];

    for (md, label, style_predicate) in cases {
        let events = parse_markdown_to_events(md);
        let heading = events
            .iter()
            .find_map(|e| {
                if let RenderEvent::Heading { level, elems } = e {
                    Some((*level, elems))
                } else {
                    None
                }
            })
            .unwrap_or_else(|| panic!("{label}: no Heading event for {md:?}: {events:?}"));
        assert_eq!(heading.0, 1, "{label}: heading level should be 1");
        assert!(
            heading.1.iter().any(style_predicate),
            "{label}: predicate did not match any inline elem in {heading:?} for {md:?}",
        );
    }
}

#[test]
fn test_parse_markdown_fuzz_property() {
    use proptest::prelude::*;
    use proptest::strategy::ValueTree;

    // Generates a string of common markdown elements joined by blank
    // lines, so the parser sees a realistic mix of constructs.
    fn md_grammar() -> impl Strategy<Value = String> {
        let heading = "[#]{1,6}[ \\t]+[A-Za-z ]{1,30}";
        let para = "[A-Za-z ,.!?]{0,80}";
        let code_block = "```[a-z]*\\n[a-zA-Z0-9 ;]{0,40}\\n```";
        let bullet = "- [ \\t]{0,3}[A-Za-z ]{1,30}";
        let task = "- \\[[ x]\\] [A-Za-z ]{1,30}";
        let table_row = "\\|?[A-Za-z ]{1,5}(\\|[A-Za-z ]{1,5})*\\|?";
        let table_sep = "\\|?[ -]{3}(\\|[ -]{3})*\\|?";
        let link = "\\[[A-Za-z ]{1,20}\\]\\(https?://[a-z.]+\\)";
        let inline = prop_oneof![
            2 => Just(para.to_string()),
            1 => Just(heading.to_string()),
            1 => Just(code_block.to_string()),
            1 => Just(bullet.to_string()),
            1 => Just(task.to_string()),
            1 => Just(format!("{table_row}\\n{table_sep}\\n{table_row}")),
            1 => Just(link.to_string()),
        ];
        proptest::collection::vec(inline, 0..8).prop_map(|v| v.join("\n\n"))
    }

    let mut runner = proptest::test_runner::TestRunner::default();
    let strategy = md_grammar();
    for _ in 0..64 {
        let input = strategy
            .new_tree(&mut runner)
            .expect("strategy should generate a value")
            .current();
        let events = parse_markdown_to_events(&input);

        // Output must be bounded — no input of this size can produce
        // more than a small constant multiple of its byte count in events.
        assert!(
            events.len() < 1_000,
            "event count exploded for input {input:?}: {} events",
            events.len()
        );

        for event in &events {
            match event {
                RenderEvent::Heading { level, elems } => {
                    assert!(
                        (1..=6).contains(level),
                        "heading level out of range: {level} in {elems:?}"
                    );
                }
                RenderEvent::Table(rows) => {
                    // Tables must be rectangular — pulldown-cmark emits
                    // them as a sequence of `TableRow` / `TableCell`
                    // events; the parser concatenates them and a
                    // non-rectangular result is a parser bug.
                    if let Some(first) = rows.first() {
                        let expected = first.len();
                        for (i, row) in rows.iter().enumerate() {
                            assert_eq!(
                                row.len(),
                                expected,
                                "table row {i} has {} cells, expected {expected}",
                                row.len()
                            );
                        }
                    }
                }
                RenderEvent::FlushInline { indent, .. } => {
                    // `indent` must not exceed the observed list depth.
                    // The parser increments `list_depth` on `Tag::List`
                    // and decrements on `TagEnd::List`; an indent > 8
                    // is impossible for a small input.
                    assert!(*indent <= 8, "indent {indent} exceeds safe bound");
                }
                _ => {}
            }
        }
    }
}
