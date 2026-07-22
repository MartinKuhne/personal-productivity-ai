# Markdown Cheatsheet Compliance Audit

Reference: https://github.com/adam-p/markdown-here/wiki/markdown-cheatsheet

## Legend
- [x] = Fully supported (compliant)
- [~] = Partially supported (some gaps)
- [ ] = Not supported

---

## Headers

- [x] H1–H6 via `#` syntax (render.rs:336-346, 141-160 — fixed 2026-07-21)
- [ ] Setext-style headers (`===` / `---` underline) — pulldown-cmark parses to `Heading`, but needs verification
- [x] Header rendering in UI content
- [x] Header rendering in Table of Contents

## Emphasis

- [x] Bold (`**text**` or `__text__`)
- [x] Italic (`*text*` or `_text_`)
- [x] Combined bold+italic
- [x] Strikethrough (`~~text~~`)

## Lists

- [~] Ordered lists — basic support works; nested list indentation needs review
- [~] Unordered lists (`*`, `-`, `+`) — basic support works
- [ ] Multi-paragraph list items (blank line + indent within item)
- [x] Task lists (`- [x]` / `- [ ]`)

## Links

- [x] Inline-style links (`[text](url)`)
- [x] Reference-style links (`[text][ref]`) — resolved by pulldown-cmark parser
- [x] Auto-links (`<url>`) — parser emits inline links
- [ ] Bare URL auto-linking (`http://...`) — depends on pulldown-cmark; check if `Options::ENABLE_AUTOLINK` is needed
- [ ] Link titles (`[text](url "title")`) — parser strips title; not rendered

## Images

- [~] Inline images — rendered as `[Image: url]` text; no actual image display (render.rs:95-97)
- [~] Reference-style images — resolved by parser; same inline rendering
- [ ] Image alt text — not included in rendering

## Code

- [x] Inline code `` `code` `` — rendered with monospace + gray background
- [x] Fenced code blocks (```` ``` ````)
- [ ] Syntax highlighting (language-specific coloring) — code blocks rendered as plain monospace text
- [ ] Language label display (filename or language tag in fenced blocks)

## Footnotes

- [~] Footnote references (`[^1]`) — rendered as monospace `[^n]` text
- [~] Footnote definitions — rendered with bold label; no dedicated visual container
- [ ] Multi-line footnote content — needs review

## Tables

- [x] GFM table syntax (pipes, dashes)
- [ ] Column alignment (colons in separator row) — alignment info from `Tag::TableHead` is not applied
- [x] Striped rows
- [x] Bold header row
- [ ] Visual frame with rounded corners (REQ-211b — documented gap)

## Blockquotes

- [ ] Blockquote visual rendering — content is flushed as regular inline; no left border, indent, or distinct background
- [ ] Nested blockquotes
- [ ] Markdown inside blockquotes (e.g., `*emphasis*`, `code`)

## Inline HTML

- [~] Inline HTML tags — rendered as gray italic text (render.rs:98-100)
- [ ] HTML blocks — rendered as gray italic text; no HTML rendering engine
- [ ] Definition lists (`<dl>`, `<dt>`, `<dd>`)

## Horizontal Rules

- [x] `---`, `***`, `___` — rendered via `ui.separator()`

## Line Breaks

- [x] Soft breaks (single newline → space) — ENABLE_HARD_BREAKS disabled per spec
- [x] Hard breaks (two trailing spaces + newline) — `Event::HardBreak` handled in parser loop (render.rs:538-552)

## Other

- [ ] YouTube video embedding (image+link workaround) — not applicable for native renderer
- [ ] Reference link definitions — resolved by parser; not exposed in UI

---

## Summary

| Category | Status |
|----------|--------|
| Headers | ✅ FIXED (was H1-H3 only) |
| Emphasis | ✅ |
| Lists | ⚠️ Partial (nested items, multi-paragraph) |
| Links | ⚠️ Partial (link titles, bare URLs) |
| Images | ⚠️ Partial (no image rendering, no alt text) |
| Code | ⚠️ Partial (no syntax highlighting) |
| Footnotes | ⚠️ Partial (no dedicated container) |
| Tables | ⚠️ Partial (no alignment, no frame) |
| Blockquotes | ❌ Not visually rendered |
| Inline HTML | ⚠️ Partial (text representation only) |
| Horizontal Rules | ✅ |
| Line Breaks | ✅ |

## Priority Gaps to Address (recommended order)

1. **Blockquote rendering** — most visible gap; needs left border/indent
2. **Table alignment** — affects readability of data tables
3. **Syntax highlighting** — improves code block readability significantly
4. **Image display** — show images instead of `[Image: url]` text
5. **Link titles** — render `title` attribute on hover
6. **Nested list indentation** — verify and fix multi-level list rendering
7. **Setext headers** — verify pulldown-cmark handles these correctly
