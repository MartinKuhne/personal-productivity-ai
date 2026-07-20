# Implementation Plan: csv-database-tools

**Branch**: `[005-csv-database-tools]` | **Date**: 2026-07-19 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/005-csv-database-tools/spec.md`

## Summary

Implement context-aware CSV Database Tools (`add_rows`, `delete_rows`, `create_csv`, `list_csv`, `query`) for LLM agents. The `query` tool evaluates dynamic predicates and aggregations using the `evalexpr` crate on in-memory datasets. Tools are selectively exposed based on prompt keywords, and data is stored in the `%APPDATA%\fastmd\db\` location.

## Technical Context

**Language/Version**: Rust 1.75+

**Primary Dependencies**: `csv` (for parsing), `evalexpr` (for dynamic query evaluation)

**Storage**: Local File System (default: `%APPDATA%\fastmd\db\`)

**Testing**: `cargo test`

**Target Platform**: Desktop (Windows/macOS/Linux via standard directories)

**Project Type**: Rust Desktop Library / Tool Module

**Performance Goals**: < 1 second for querying 10,000 rows

**Constraints**: Strict schema validation on insert; Best-effort type inference on query evaluation.

**Scale/Scope**: 5 Tools, local files up to ~50MB.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **Testability**: YES. Core tool logic will be decoupled from side-effects (file I/O) to allow unit testing with mock data.
- **Security**: YES. Input parsing and CSV sanitization mitigates injection. `evalexpr` is a safe evaluator.
- **Modularity**: YES. Tools are independent functions.
- **Open Source Leverage**: YES. Using `csv` and `evalexpr` crates instead of reinventing parsers.
- **SDLC Best Practices**: YES. Test-driven changes planned.

## Project Structure

### Documentation (this feature)

```text
specs/005-csv-database-tools/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
└── contracts/
```

### Source Code (repository root)

```text
src/desktop/
├── src/
│   ├── tools/
│   │   ├── csv_db/
│   │   │   ├── mod.rs
│   │   │   ├── operations.rs   # Core logic for tools
│   │   │   ├── query.rs        # evalexpr integration
│   │   │   └── schema.rs       # Validation logic
```

**Structure Decision**: The logic will be housed under a new `csv_db` module in the `src/desktop/src/tools/` path (assuming typical tools architecture).

## Complexity Tracking

(No violations of the Constitution)
