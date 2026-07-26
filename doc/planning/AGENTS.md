# AI Agent Instructions — `doc/planning/`

Planning and design-record documents. Repo-root `AGENTS.md` provides shared
principles; this file adds doc-lifecycle rules.

## 1. Document types
- **Design records / ADRs** — titled `*.md`, follow the ADR header pattern:
  `# Title`, `Status: proposal | accepted | superseded`, `Date: YYYY-MM-DD`,
  then Context → Decision → Consequences.
- **Research notes** — time-stamped, single-purpose (e.g.
  `tool-call-arguments-null-research.md`). Keep them narrow; split a new note
  rather than appending unrelated findings.

## 2. Lifecycle
- A proposal becomes `accepted` only after review; capture the reviewer and
  date in the document.
- When a decision is overturned, mark the original `superseded` and link to
  the new ADR. Do not delete superseded files — they are the audit trail.
- Accepted ADRs that have landed in code should be reflected in
  `doc/technical-context/ARCHITECTURE_C4.md` and/or `SPEC.md`; the ADR stays
  for rationale, the technical-context doc carries the current state.

## 3. Quality gate
- Each file has a clear title and status line.
- No aspirational content leaks into `doc/technical-context/`; aspirational
  work lives here, labelled as such.
- Cross-links to `REQ-xxx` (in `SPEC.md`) and to source files are valid.
