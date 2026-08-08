# CommonMark 0.31.2 spec fixture

This directory contains a vendored copy of the
[CommonMark 0.31.2 spec](https://spec.commonmark.org/0.31.2/) source
(`commonmark-0.31.2-spec.txt`) used by the integration test
`commonmark_spec_test.rs` in this crate's `tests/` directory.

## Why we vendor it

The spec source is the authoritative catalogue of all CommonMark test
cases (~650 numbered examples) and is the only sensible "ground truth"
for the markdown→Typst translator. The integration test parses the
file, extracts every example's markdown input, and asserts the
translator emits non-empty Typst and that the resulting document
compiles to a valid PDF through `typst-as-lib`.

## How to refresh

To pick up a newer spec release:

1. Download the new `spec.txt` from the matching tag on the
   [commonmark-spec GitHub repository](https://github.com/commonmark/commonmark-spec)
   (raw URL pattern: `https://raw.githubusercontent.com/commonmark/commonmark-spec/<TAG>/spec.txt`).
2. Save it as `commonmark-<VERSION>-spec.txt` in this directory.
3. Update the `SPEC_VERSION` constant in `commonmark_spec_test.rs` if
   you added the file under a new name; otherwise the test will
   auto-pick the new file.
4. Re-run `cargo nextest run -p fastmd commonmark_spec_test` to make
   sure the translator still passes.

## License

The vendored spec file is `spec.txt` from the
[commonmark-spec](https://github.com/commonmark/commonmark-spec)
repository, which is © John MacFarlane and licensed under
[Creative Commons Attribution-ShareAlike 4.0 International (CC-BY-SA 4.0)](https://creativecommons.org/licenses/by-sa/4.0/).

The original license notice is preserved in the YAML frontmatter at
the top of `commonmark-0.31.2-spec.txt`. Any modifications to the
file would need to track the same license per the ShareAlike clause.
