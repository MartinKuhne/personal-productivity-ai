# Markdown Subsystem Specification

> **GUARDRAIL**: This specification file is managed by the spec-split workflow. Do not edit
> this file directly unless explicitly instructed. Any changes to requirements must be
> reflected in the corresponding implementation code. If drift is detected between
> this spec and the actual code behavior, notify the user immediately.
>
> Part of [`SPEC.md`](../../SPEC.md) (FastMD crate). This subsystem is the single
> import point for `pulldown-cmark`, `serde_norway` front-matter/table parsing, and
> Markdown AST types. `ui/`, `tools/`, `print.rs`, and `editor.rs` call into
> `markdown::` rather than handling Markdown directly.

## Requirements

The requirements below have been formatted using the **Easy Approach to Requirements Syntax (EARS)**, utilizing Ubiquitous, Event-Driven (When), State-Driven (While), Unwanted Behavior (If), and Optional Feature (Where) templates.

* [MD-001] GFM Parsing: The Markdown parser shall support GitHub Flavored Markdown (GFM) features including tables, footnotes, strikethrough text, and task lists. Hard line breaks shall be disabled by default.
* [MD-002] Heading Sizing: The FastMD Viewer shall render H1 through H6 headings at 32px, 24px, 18px, 14px, 12px, and 12px respectively.
* [MD-003] Break Semantics:
    * [MD-004]: When a single carriage return (newline) is encountered, the Markdown parser shall render it as a single space character (soft break).
    * [MD-005]: Hard breaks (two trailing spaces followed by a newline, or backslash-newline) shall force a line split when hard break parsing is enabled.
* [MD-006] Bullet Lists: The FastMD Viewer shall render list items with indentation and bullet points (`\u{2022}`).
    * [MD-007]: If a list item contains hard breaks, then the FastMD Viewer shall render the bullet only on the first line; subsequent wrapped lines shall be indented without a bullet. [Note: Current implementation renders bullet on each Item start; this is a known gap.]
* [MD-008] Table Layout:
    * [MD-009]: The FastMD Viewer shall render tables inside a horizontally scrollable container with striped rows and column spacing.
    * [MD-010]: The FastMD Viewer shall arrange table cells into styled rows and columns with alternating row backgrounds and column padding.
    * [MD-011]: The FastMD Viewer shall render header row cells with a bold font weight.
    * [MD-012]: Tables shall be rendered with a distinct visual frame (rounded corners, background color) to separate from body text. [Gap: Not yet implemented.]
* [MD-013] Off-Viewport Text Guarantees: Where a markdown document contains a table that cannot fit the available content width even at the table's min-content width per column, the FastMD Viewer shall render the table inside a horizontally scrollable container so that no cell text is permanently clipped from the user's viewport. Where a heading, paragraph, code block, list item, or any other text-bearing surface would otherwise place its text rect outside its containing clip area with no scrollable ancestor that could bring it into view, the FastMD Viewer shall instead wrap, wrap-into-scroll, or otherwise guarantee the text is reachable.
* [MD-014] YAML Front-Matter: Where a document contains a YAML front-matter header, the FastMD Viewer shall parse the metadata into key-value pairs and render them inside a dedicated container table.
* [MD-015] Table of Contents (ToC) Navigation:
    * [MD-016]: The ToC panel shall display H1–H6 headers indented by header depth.
    * [MD-017]: When a ToC element is clicked, the FastMD Viewer shall invoke a viewport scroll event to the selected heading.
* [MD-018] Markdown Cheatsheet Conformance: The FastMD Viewer shall conform to the [Markdown Cheatsheet](https://github.com/adam-p/markdown-here/wiki/markdown-cheatsheet) specification for all core Markdown features including: Headers (H1–H6, setext-style), Emphasis (bold, italic, strikethrough), Lists (ordered, unordered, nested), Links (inline, reference, auto-links), Images (inline, reference), Code (inline, fenced blocks with syntax highlights), Footnotes, Tables (with alignment), Blockquotes, Inline HTML, Horizontal Rules, and Line Breaks (soft and hard).

## YAML Frontmatter Template

```yaml
---
title: A brief title
summary: A three sentence summary of the contents
tags: ["tag1","tag2"]
header-date: <RFC 3339 timestamp>
---
```