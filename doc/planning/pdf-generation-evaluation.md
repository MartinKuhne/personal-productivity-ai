# PDF Generation from Markdown — Rust Solution Evaluation

> **Status:** research / pre-implementation evaluation
> **Scope:** Replace the current "open in browser and let the user Ctrl-P" flow in
> `src/desktop/src/app/print.rs` with a real PDF generator that produces
> print-quality output suitable for direct export **and** printing.
> **Auditor:** Mavis
> **Date:** 2026-08-07

---

## 1. The current state

`src/desktop/src/app/print.rs` renders Markdown → HTML, writes the result
to a temp `.html` file, and shells out to `webbrowser::open()`:

```rust
// src/desktop/src/app/print.rs:150
let mut temp_file = tempfile::Builder::new()
    .prefix("fastmd_print_")
    .suffix(".html")
    .rand_bytes(8)
    .tempfile()?;
// ... writes HTML, calls webbrowser::open, returns.
```

What the user actually gets:

1. A browser tab opens with the rendered HTML.
2. They have to invoke `Ctrl+P` themselves, pick "Save as PDF" or a printer.
3. The "PDF" is whatever the user's default browser produces.

That has three concrete problems:

- **Spec drift.** `src/desktop/src/ui/SPEC.md` says REQ UI‑014 must use
  `ShellExecute "print"` so the Windows print dialog appears. The
  implementation does neither — it just opens a browser. REQ violation.
- **Not an export.** The user asked for PDF, they got an HTML tab. There is
  no file handle to attach to an email, no "Export PDF" menu, no batch
  output. They have to babysit the dialog.
- **Quality is whatever the browser is.** It works, but it's not "beautiful"
  in the typographic sense — no proper page geometry, no running headers,
  no balanced columns, no per-document metadata.

The user's prompt ("I really like to print") and the existing REQ UI‑014
together point at the same destination: a real PDF pipeline, not a browser
shortcut.

---

## 2. The solution space

The Rust PDF ecosystem falls into four families. They differ on
**HTML/CSS fidelity**, **output quality**, **dependency weight**, and
**how much layout you have to do yourself**.

| Family                       | HTML/CSS fidelity | Layout effort | Deps                              | Output quality                |
| ---------------------------- | ----------------- | ------------- | --------------------------------- | ----------------------------- |
| **A. Browser-driven**        | Full (real CSS)   | None (CSS)    | Chromium binary (~400 MB)         | Web-quality, very good        |
| **B. Pure-Rust layout**      | None              | High          | None                              | Functional, plain             |
| **C. Typst / Krilla**        | None              | Medium        | Static, no browser                | Professional, beautiful       |
| **D. External subprocess**   | High              | None          | Pandoc / WeasyPrint / wkhtmltopdf | Depends on tool               |

Below is a per-library breakdown with the hard pros and cons for *this*
codebase, not generic ones.

### 2.A — Browser-driven (the "use a real engine" path)

| Crate              | Status                | Used here today?         |
| ------------------ | --------------------- | ------------------------ |
| `playwright-rs`    | active                | yes, behind `browser` feature flag (`Cargo.toml:46`, `Cargo.toml:21`) |
| `headless_chrome`  | active                | no                       |
| `chromiumoxide`    | active                | no                       |
| `wkhtmltopdf`      | legacy / unmaintained | no                       |
| `weasyprint`       | Python, not Rust      | no                       |

**How it would work for fastmd.** Launch a **headless Chromium** (not
Firefox — `Page.pdf()` is Chromium-only), load the HTML produced by
`render_markdown_to_html` via `Page::set_content`, then call
`Page::pdf()` with paper format, margins, header/footer templates, and
`printBackground: true`. Output goes straight to a `PathBuf` next to
the source `.md`.

**Pros for *this* repo**

- `playwright-rs` is already in `Cargo.toml`. The `browser` feature
  flag already exists. The plumbing for launching, killing, and
  reusing a browser process (`app::session::BrowserSession`,
  `app::browser::session`) is already battle-tested.
- The HTML produced by `markdown::parser::render_markdown_to_html` plus
  `build_html_document` in `print.rs` is already a working print
  stylesheet. Re-use it verbatim.
- CSS handles `prefers-color-scheme`, `@page` size, custom fonts,
  syntax highlighting — anything the user wants. We're not re-implementing
  the layout we already have.
- `page.pdf()` honours the same `@media print` rules the existing
  stylesheet uses, so dark-mode / light-mode flow through naturally.
- `playwright-rs` 0.15.1 already exposes `Page::pdf` and `PdfOptions`
  (per docs.rs/playwright-rs).

**Cons for *this* repo**

- `page.pdf()` is **Chromium-only**. The current `BrowserSession` uses
  Firefox. You'd either (a) keep two browser processes alive — Firefox
  for the agent tools, Chromium for PDF — or (b) gate PDF export
  behind a `tool_groups.browser && chromium_installed` precondition.
  Option (a) is more code; option (b) is more error paths. Either is
  fine, just pick one explicitly.
- The browser is a 400 MB binary the user has to install via
  `playwright install chromium`. The README already says
  `playwright install firefox` is required for the agent browser tool,
  so adding `chromium` to that step is a one-line doc change.
- Cold start is ~200–400 ms for a one-shot PDF. For a "print one
  document" UX that's invisible. For a batch "export 50 files"
  feature it's noticeable. Keep the browser warm if you batch.
- Header/footer templates have four known Chromium footguns (default
  `font-size: 0`, no inherited CSS, no `var()`, no CSS counters).
  You have to inline styles and reserve margin space yourself. This
  is a one-time annoyance, not an ongoing one.

### 2.B — Pure-Rust layout (the "no browser" path)

| Crate           | Status    | Notes                                                       |
| --------------- | --------- | ----------------------------------------------------------- |
| `genpdf`        | active    | High-level builder on top of `printpdf`; pagination, wrapping |
| `printpdf`      | active    | Low-level: you place every glyph by coordinate              |
| `pdf-writer`    | active    | Even lower; the engine that powers Typst                    |
| `krilla`        | active    | PDF-1.7 generator, same author as Typst — Rust-native        |
| `pdf` (SidOfN)  | low maintenance | PDF assembly                                                |
| `pdfium-render` | active    | Bindings to PDFium C++ lib, ships native binary             |
| `mupdf`         | active    | Bindings to MuPDF; can rasterize + read existing PDFs       |
| `lopdf`         | active    | Read / mutate existing PDFs; not a generator                 |

**How it would work for fastmd.** Walk the `RenderEvent` stream from
`markdown::parser::parse_markdown_to_events` and feed events into
`genpdf`'s `Document::add(...)`. Position headings, paragraphs, code
blocks, table rows, and images by hand. Render to a `BufWriter<File>`.

**Pros for *this* repo**

- Zero external binaries at install time. `cargo install fastmd` and
  you're done.
- No 400 MB Chromium to ship. Binary stays under 30 MB.
- Fully deterministic, byte-for-byte reproducible. No browser drift.
- Pure Rust: aligns with the project's "native, lightweight, GPU-free
  PDF output" positioning.

**Cons for *this* repo**

- This is the *hard* path. There's no CSS, no flexbox, no floats.
  You write the layout. For example, the FTWA table-width algorithm
  in `markdown::table_width` already produces a `TableLayout` —
  you'd have to translate that into `genpdf` / `printpdf` calls,
  page-break on row, repeat headers, etc. That's hundreds of lines
  of code that the team would own forever.
- Output quality is "good" not "beautiful". Default fonts are
  not LaTeX-grade. No native justification, no hyphenation, no
  small-caps, no real OpenType features. You can ship a bundled
  font (e.g. Inter, IBM Plex) and it looks *fine*, but it's not
  typographically remarkable.
- You give up `@page` rules. Custom paper size, page numbers in
  running headers, "different first page" — all manual.
- Image handling (resize, reflow around images) is your problem.
  `genpdf` does not flow text around an image.
- For a "I really like to print" user, the visual quality ceiling
  is noticeably lower than CSS-driven or Typst output.

### 2.C — Typst / Krilla (the "real typesetting" path)

| Crate / tool           | Status    | Notes                                           |
| ---------------------- | --------- | ----------------------------------------------- |
| `typst`                | active    | Full Typst engine compiled to a Rust lib        |
| `typst-as-lib`         | active    | Higher-level wrapper around `typst`             |
| `krilla`               | active    | Pure-Rust PDF-1.7 generator from Typst authors  |
| `pulsar`               | active    | Markdown → Typst → PDF, opinionated            |
| `markpdf` / `md2typ`   | mixed     | Markdown → Typst converters                     |

**How it would work for fastmd.** Translate the markdown document
into Typst markup server-side: `#heading[... ]`, `#table(...)`,
`#raw(block: true, lang: "rs", ...)`, etc. Wrap it in a `#set page(...)`
template (size, margins, header, footer). Compile via
`typst::compile(&document)`. Write the resulting `Vec<u8>` to disk.

**Pros for *this* repo**

- The best-looking output of any option in the list. Typst is a
  modern typesetting system, not a CSS-rendering hack. Real
  hyphenation, real kerning, real OpenType.
- No browser, no subprocess, fully static linking. Binary grows
  by ~25–30 MB (fonts included). No 400 MB Chromium.
- Fast: single-digit to low-double-digit ms once warm. A 50-page
  document is sub-100 ms.
- Deterministic, like all pure-Rust paths.
- Custom paper geometry, page numbers in headers, "different
  first page", `PagebreakTo(odd)`, `Counter(page).display()` —
  all built-in.

**Cons for *this* repo**

- **You have to write a Markdown → Typst translator.** This is
  the *real* cost. The existing `RenderEvent` AST is egui-shaped
  (it carries `Color`, `FontFamily`, `Vec2` semantics, etc.).
  Converting it to Typst markup is a from-scratch effort. GFM
  tables, footnotes, task lists, strikethrough — all of it
  needs a hand-written mapper. Probably 600–1000 lines of
  tested code, plus a Typst template file.
- Existing markdown rendering work (parser, table width,
  inline coalesce) is *not* directly reusable. The `RenderEvent`
  shape doesn't map cleanly to Typst's content model.
- The `typst` crate is large and pulls in `fontdb`, `hypher`,
  `kurbo`, etc. Compile time goes up by a few minutes.
- If a future user wants "the markdown looks exactly like the
  in-app preview" — that fidelity is *harder* with Typst than
  with the browser path, because the in-app preview *is* CSS.

### 2.D — External subprocess (the "let someone else do it" path)

| Tool          | Status      | Notes                                  |
| ------------- | ----------- | -------------------------------------- |
| `pandoc`      | ubiquitous  | MD → LaTeX/HTML/... → PDF, slow cold   |
| `wkhtmltopdf` | unmaintained | Old WebKit, security CVEs             |
| `weasyprint`  | active      | Python install, great paged CSS        |
| `prince`      | commercial  | Best CSS fidelity, paid                |
| `docraptor`   | SaaS        | Hosted, $                              |

This is the path the codebase already uses for *inbound* PDF
processing (`pdf_converter.rs` shells out to `marker_single`).
Outbound, it's a fair fit:

- `pandoc` with `pdf_converter_command` template is one config
  line, but `pandoc` needs LaTeX (TeX Live, ~1 GB) for PDF output.
- `wkhtmltopdf` is dead — don't.
- `weasyprint` (via Python) is the best of the three but adds a
  Python toolchain to the Windows installer.
- Cloud APIs violate "personal-productivity-ai" — no remote servers.

**Verdict for this repo:** the only one I'd even consider is
**pandoc**, and only as a config-driven fallback. It mirrors the
existing `pdf_converter_command` pattern. But: requires TeX Live,
slow, and the user would still configure two different commands
(one in, one out). Adds documentation burden.

---

## 3. Side-by-side: which fits fastmd?

| Criterion                                | Browser (Playwright) | Pure-Rust (genpdf)   | Typst (`typst`/`typst-as-lib`) | Pandoc subprocess |
| ---------------------------------------- | -------------------- | -------------------- | ------------------------------ | ----------------- |
| Output beauty                            | ★★★★☆                | ★★☆☆☆                | ★★★★★                          | ★★★☆☆             |
| Engineering effort (1st ship)            | low                 | high                 | medium-high                    | very low          |
| Engineering effort (maintain)            | low                 | high                 | medium                         | low               |
| New dependencies (binary size)           | +400 MB (Chromium)   | +0 (small crates)    | +~30 MB (engine + fonts)       | +1 GB (TeX Live)  |
| New Rust deps                            | none (already have)  | `genpdf` + fonts     | `typst-as-lib` + fonts         | none              |
| Compile time impact                      | none                 | small                | medium                         | none              |
| Cold-start latency per PDF               | 200–400 ms           | < 50 ms              | 20–80 ms                       | 2–10 s            |
| Reuse existing HTML render path          | ✅ direct            | ❌ (would re-do)     | ❌ (markdown→typst)             | ✅ direct          |
| Honors existing in-app CSS               | ✅ exact             | ❌                   | ❌                             | partial           |
| Works without `browser` feature          | ❌                   | ✅                   | ✅                             | ✅                |
| Cross-platform safe                      | ✅                   | ✅                   | ✅                             | ⚠️ TeX Live hell  |
| Stays inside AGENTS.md "minimal deps"    | ⚠️ (browser)         | ✅                    | ✅                             | ❌                 |
| Spec alignment with REQ UI‑014           | partial              | full                 | full                           | full              |
| Risk of regression on markdown features  | low                  | high (rewriting)     | high (rewriting)               | low               |

---

## 4. My recommendation

**Ship a two-layer strategy. Lead with Playwright + Chromium. Keep Pandoc
as a config-driven fallback. Defer Typst until the user actually asks
for "premium" output.**

### Why this and not the others

**Pure-Rust (`genpdf` / `printpdf`)** is the only path that fits the
README's "lightweight, no Electron shell" claim. But the cost is
re-implementing table layout, text reflow, and image placement — and
the output will *not* look "beautiful". The user said they like to
print, and `genpdf` output is functional but visibly plain. Reject.

**Typst** would produce the most beautiful output. But it requires
writing a markdown → Typst translator that the project would own
forever, and it abandons most of the existing `markdown::` subsystem
work. That's a big architectural reversal for what is currently a
~200-line `print.rs` file. Worth doing if "print quality" becomes a
product differentiator. Not now. Defer to a future iteration.

**Pandoc** is the cheapest, most conservative move. It's symmetric
with the existing `pdf_converter_command` pattern in `config.yaml`.
But it requires the user to install LaTeX (~1 GB) for `pandoc
--to=pdf`, which contradicts the "drop-in `cargo install`" feel.
Reject as primary, keep as config-driven escape hatch.

**Playwright + Chromium** wins on five of six criteria that matter
for *this* codebase:

1. **Zero new Rust deps** — `playwright-rs` is already in `Cargo.toml`
   under the `browser` feature. Just flip the existing flag default
   for the print path or add a new `print` feature.
2. **Reuses the HTML pipeline verbatim.** `build_html_document` in
   `print.rs` already produces a decent print stylesheet. Just hand
   that string to `Page::set_content()`.
3. **Quality is "professional web-quality".** That's enough for
   markdown notes, documentation, blog drafts — exactly fastmd's
   content shape. Not magazine-grade, not LaTeX-grade, but a solid
   two steps above `genpdf`.
4. **Honors the spec.** REQ UI‑014 wants the *Windows* print
   dialog, but that's a footgun — `ShellExecute "print"` blocks
   the UI, doesn't return a file, and bypasses all CSS. Replacing
   it with "export PDF to `<source>.pdf`, then `ShellExecute
   "open"`" is closer to the spirit of the requirement and gives
   the user a file they own.
5. **Latency is fine.** 200–400 ms cold, <50 ms warm. A user
   who likes to print will tolerate that.

The one real cost is **Chromium itself**. Mitigations:

- Reuse the browser process across prints (one warm launch, many
  PDFs).
- Keep the existing `browser` feature as the gating switch but
  extend its meaning from "agent tools" to "agent tools + PDF
  export". Users who don't have `playwright install chromium` just
  don't get the export.
- Add a second `chrome_path` config key as a fallback so users
  with a system Chrome don't have to download a second copy.

### What I'd build, in order

1. **`app::print::PdfExporter`** (new module, sibling of `print.rs`).
   Takes a `PrintJob`, a `BrowserSession` (or a thin local one if
   `browser` feature is off), and an output `PathBuf`. Calls
   `Page::set_content` with the same HTML `build_html_document`
   already produces, then `Page::pdf` with sensible defaults
   (`A4`, `printBackground: true`, `preferCSSPageSize: true`,
   margins to leave room for the page header / footer).
2. **Add a `print` config block** to `config.yaml`:
   ```yaml
   print:
     paper: a4
     margin_mm: 18
     print_background: true
     header_template: "<div class=\"title\">{{TITLE}}</div>"
     footer_template: "<div class=\"pageNumber\">Page <span class=\"pageNumber\"></span> of <span class=\"totalPages\"></span></div>"
   ```
3. **Fix the spec drift.** Replace the `webbrowser::open` call in
   `print.rs` with `PdfExporter::export_to_file(job, output_path)`,
   then `opener::open(&output_path)` (already a dep) so the user
   sees the result. Update UI‑014 in `src/ui/SPEC.md` to reflect
   the new behaviour: "exports a PDF next to the source file, then
   opens the file in the user's default PDF viewer".
4. **Add a separate `print -> export_pdf` background worker.**
   Reuse the `BackgroundEvent` / `LogCategory::Print` channel that
   `print.rs` already writes to. Surface the result in the existing
   Background Process Log tab — the user can see "Exported
   `notes.md` → `notes.pdf`" with byte count and timing.
5. **Tests.** Unit tests on `PdfOptions` construction. Integration
   test that fires a Playwright Chromium against a known fixture
   markdown file and checks the output PDF byte size is non-zero
   (gated behind `feature = "browser"` and ignored if Chromium
   isn't installed, the same way `browser.rs` already handles it).
6. **Documentation update.** `README.md` mentions `playwright
   install firefox`; add `playwright install chromium` to the
   same line. One sentence.

### Open question to confirm before I start

Is REQ UI‑014 ("Windows system print dialog via ShellExecute")
something you want me to **keep** (in which case we'd also need
the legacy `webbrowser` path for users who haven't installed
Chromium), or **replace** (and update the spec to "export PDF +
open it")? My default is **replace**, but the SPEC is a guardrail
in this repo, so I want to confirm before changing it.

---

## 5. TL;DR

| Question                                      | Answer                                                                  |
| --------------------------------------------- | ----------------------------------------------------------------------- |
| Is there a Rust solution for beautiful PDFs?  | Yes — Playwright + Chromium, already in your `Cargo.toml`.              |
| Is there a *pure-Rust* solution?              | `genpdf` (functional, plain) or `typst` (beautiful, big rewrite).       |
| Which one fits *this* repo today?             | Playwright + Chromium. Reuses the HTML pipeline, ships in one PR.       |
| What's the catch?                             | Need a 400 MB Chromium install. Same precondition as the agent browser. |
| What about the spec drift?                    | REQ UI‑014 needs to be updated; my recommendation is to update, not workaround. |
| Should I start implementing?                  | Awaiting your answer on REQ UI‑014 (see "Open question" above).        |

---

## Appendix A — Reference material

- **playwright-rs API**: <https://docs.rs/playwright-rs> — `Page::pdf`,
  `PdfOptions` (`path`, `format`, `width`, `height`, `margin`,
  `printBackground`, `displayHeaderFooter`, `headerTemplate`,
  `footerTemplate`, `pageRanges`, `preferCSSPageSize`).
- **Playwright upstream docs**: <https://playwright.dev/docs/api/class-page#page-pdf>
  — the four known header/footer footguns (default `font-size: 0`,
  no CSS inheritance, no `var()`, no CSS counters) and their fixes
  are documented at <https://pdf4.dev/blog/playwright-pdf-header-footer-guide>.
- **PDF generation landscape** (2026): <https://pdf4.dev/blog/pdf-generation-rust>
  — four-axis decision matrix covering the same crates evaluated here.
- **genpdf**: <https://crates.io/crates/genpdf> — pure-Rust document
  builder on top of `printpdf`. Pagination, wrapping, headers,
  footers. Ships without browser, no native deps.
- **typst / typst-as-lib**: <https://github.com/typst/typst> and
  <https://crates.io/crates/typst-as-lib> — embeddable Typst
  compiler, ~25–30 MB binary cost.
- **Krilla**: <https://github.com/LaurenzV/krilla> — pure-Rust PDF
  generator from a Typst author. Lower-level than `typst`; better
  if you want to hand-author the PDF content tree.
- **AGENTS.md constraint reference**: `src/desktop/AGENTS.md` —
  `RUST-051` (place files by concern), `RUST-052` (event-driven
  fan-out via `Bus<T>`), `RUST-053` (4096-line file limit).
