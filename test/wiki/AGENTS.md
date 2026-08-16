# AI Agent Instructions — `src/test/wiki/`

Test wiki fixtures used as input by the `fastmd` integration tests. Repo-root
`AGENTS.md` provides shared principles; this file pins fixture rules.

## 1. Fixtures only
- Content here exists solely to exercise the viewer / indexer / editor / agent.
  Do not add real personal data, real secrets, or working credentials.
- File names and front-matter should be deliberately varied (front-matter vs
  none, UTF-8 BOM vs none, CRLF vs LF, nested vs flat) so tests can rely on the
  coverage.

## 2. Hygiene
- UTF-8, newline at EOF. Markdown must be valid GFM (pulldown-cmark-parsable).
- Keep fixtures small; large fixtures inflate CI cost without test value.
- When deleting a fixture, search the test suite for references and update them
  in the same change.

## 3. Quality gate
- `src/desktop/` `cargo test` still green after any fixture change.
- No real secrets; if in doubt, run `github_run_secret_scanning` over the
  changed content before commit.
