# Agent Instructions — `src/agent/`

## User-facing text

- Isolate all user-facing text into [`strings.rs`](strings.rs) as `pub const` values.
- "User-facing" means text a human reads: the OAuth callback page copy the browser
  renders, and error messages the application surfaces to the user.
- Do not catalogue log messages, `tracing` field values or messages, tool
  JSON-schema descriptions, protocol strings, or NFR-004 error codes. Those stay
  with the code that emits them.
- Reference the text through `crate::strings::<CONST_NAME>` (import it, do not
  re-declare it locally).
- Every `pub const` in [`strings.rs`](strings.rs) MUST have a `///` doc comment
  naming where the text appears.

## Quality gate

- New user-facing literals do not appear in product modules; `rg` for the literal
  finds exactly one definition, in `strings.rs`.
- Requirement IDs cited in doc comments (`AGENT-xxx`, `CONFIG-xxx`) exist in
  [`SPEC.md`](SPEC.md).
