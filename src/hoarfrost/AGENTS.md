# Agent Instructions — `src/hoarfrost/`

## User-facing text

- Isolate all user-facing text into [`strings.rs`](strings.rs) as `pub const`
  values: control labels, headers, placeholders and validation messages a host
  renders verbatim.
- "User-facing" means text a human reads. Glyphs used as layout markers
  (`GLYPH_ADDED`, `CHEVRON_DOWN`), a11y role names, token names and gallery
  fixtures are not user-facing and stay with the module that uses them.
- Define each constant in [`strings.rs`](strings.rs) and re-export it from the
  module that originally owned it
  (`pub use crate::strings::LABEL_APPLY;` in `widget::color_picker`), so the
  historical public path stays valid for hosts.
- Every `pub const` in [`strings.rs`](strings.rs) MUST have a `///` doc comment
  naming the control or message it labels, and cite the `spec.md` section /
  `UIF-*` ID it implements.
- [`microcopy/`](microcopy/mod.rs) is a different concern: it holds the *rules*
  for shaping text (truncation, case, tabular figures), not the text itself.

## Quality gate

- Each requirement ID cited in a doc comment (`UIF-*`) exists in `spec.md`.
- Re-exported labels keep their original module path importable — a host that
  referenced `widget::color_picker::LABEL_APPLY` must not need to change.
