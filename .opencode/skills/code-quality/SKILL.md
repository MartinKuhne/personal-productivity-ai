---
name: code-quality
description: Run code quality audits for test coverage, logging, duplication, module boundaries, spec drift, spec gaps, architecture drift, and performance (dependencies, binary size, compile times)
---

# Code Quality Skill

## Purpose

This skill supports the code quality agent. It defines 8 workflows. Each workflow audits one quality dimension. Each workflow produces a report and a remediation plan. This skill does not change code or specs. It only analyzes and plans.

## When to Use

- Use this skill when you invoke `@code-quality` with any workflow name.
- Use this skill when you run `/code-quality` or any `/code-quality-*` command.
- Use this skill when the user asks for coverage gaps, logging audits, duplication checks, boundary reviews, spec drift, spec gaps, architecture drift, or performance audits (dependencies, binary bloat, compile times).

## Prerequisites

Do these checks before any workflow:

1. Run `cargo check --quiet`. Stop and report if the check fails.
2. Confirm the CodeGraph index is current. Run `codegraph explore "<scope> symbols"` for the scope.
3. Load `SPEC.md`, the relevant `src/*/SPEC.md` files, `doc/technical-context/ARCHITECTURE_C4.md`, and `.specify/memory/constitution.md`.

## How to Route

Parse user input as `<workflow> [scope]`.

- `workflow` is one of: `coverage`, `logging`, `duplication`, `boundaries`, `spec-drift`, `spec-gaps`, `arch-drift`, `perf`, `all`.
- `scope` is a path glob, for example `src/agent` or `src/app/workspace`.
- If the user omits workflow, use `all`.
- If the user omits scope, use the full workspace (`src/` and `doc/technical-context/`).

Then run the matching workflow below. For `all`, run workflows W1 to W8 in order and aggregate the report.

## Workflow W1 — coverage

**Goal:** Close gaps in unit test coverage for all code paths, error paths, and boundary conditions.

**Steps:**

1. List all public functions and methods in the scope with CodeGraph.
2. For each function, list all code paths. Include `match`, `if let`, `Result`, `Option`, and validation branches.
3. For each function, list all error paths. Include `?`, `Err`, and custom error types.
4. For each function, list all boundary conditions. Include empty input, max size, zero, and `..` traversal.
5. Map each path to existing tests. Use `cargo nextest` inventory and `*_tests.rs` sidecars.
6. Mark paths with no test as gaps. Flag inline test blocks that exceed 150 lines (RUST-056).
7. Flag tests that touch real user paths or production state (RUST-006).

**Exit criteria:** Every public path has a gap row or a test link. The report states the command to run, for example `cargo nextest run -p fastmd-agent`.

## Workflow W2 — logging

**Goal:** Audit logging for external API calls and error paths. Produce a remediation plan.

**External boundaries:** `src/agent/lib/*`, `src/agent/llm_client.rs`, `src/agent/tools/*`, `src/app/background/*`, `src/app/integrations/discord`.

**Steps:**

1. List all external calls and all error branches in the scope.
2. For each call and each error branch, check that the code emits an `ERROR` log (NFR-001).
3. Check that each `ERROR` log includes a unique error code (NFR-004).
4. Check that each log uses structured JSON with `timestamp`, `level`, `service_name`, `correlation_id` (NFR-006).
5. Check that each log captures the stack trace (NFR-005) and redacts PII and tokens (NFR-009).
6. Check that telemetry is non-blocking (NFR-002) and that backend failure does not crash the app (NFR-003).
7. Check that trace context propagates (NFR-007) and that spans cover external calls (NFR-008).

**Output:** Use a matrix `External Call x Error Path -> Has ERROR log?`. Each gap includes file, line, and a proposed error code such as `JMAP-4001`.

## Workflow W3 — duplication

**Goal:** Identify code duplication and consolidate it into shared, static helper functions.

**Steps:**

1. Find clone clusters. Use CodeGraph and grep to find similar functions and blocks.
2. For each cluster, count occurrences, lines, and files.
3. Propose one static helper per cluster. Choose the owning subsystem by concern (RUST-051). Keep the helper pure and honest (RUST-020).
4. Order fixes: extract the helper first, then update callers (RUST-055). Reduce nesting, do not add branches to complex code (RUST-023).

**Output:** Clone table with helper signature and target file.

## Workflow W4 — boundaries

**Goal:** Examine module boundaries and align code with domain boundaries.

**Steps:**

1. Check each import for cross-boundary violations. Flag `app/` code that imports `egui` (RUST-058).
2. Check that each subsystem exposes its API through `mod.rs` (RUST-050) and that `lib.rs` is facade-only (RUST-054).
3. Check file size. Flag files that exceed 4096 lines (RUST-053).
4. Check that placement follows concern, not type (RUST-051).

**Output:** Violation table and a realignment plan with target directory and `mod.rs` re-export.

## Workflow W5 — spec-drift

**Goal:** Detect drift where `spec.md` files do not represent what the code implements.

**Steps:**

1. Load requirements (REQ-xxx, UI-xxx, TOOL-xxx, etc.) from `SPEC.md` and `src/*/SPEC.md`.
2. For each requirement, find the code that implements it with CodeGraph.
3. Compare spec text to code behavior. Mark each requirement as `aligned`, `drifted`, or `obsolete`.
4. Do not change any spec file (RUST-043). Only report drift.

**Output:** Drift table `Spec Clause -> Code Location -> Divergence`.

## Workflow W6 — spec-gaps

**Goal:** Identify significant behavior in the code that has no spec.

**Steps:**

1. List public APIs, tool registrations, bus events, and config fields in the scope.
2. Check each item for a traceable requirement. Look for `REQ-xxx` cites in `//!` or `///` (RUST-040).
3. Mark items with no requirement as gaps. Rank gaps by user impact.
4. Propose new requirements in high-level, goal-oriented language (RUST-042).

**Output:** Gap table and proposed spec additions with target `SPEC.md` file.

## Workflow W7 — arch-drift

**Goal:** Detect drift between `doc/technical-context` and the implementation.

**Steps:**

1. Load `ARCHITECTURE_C4.md` Context, Container, and Component diagrams and the Folder Layout block.
2. Verify that every C4 node maps to a real directory or module under `src/`.
3. Verify that bus types (`FileEvent`, `AgentEvent`, `BackgroundEvent`, `ConfigArrived`) and worker topology match the diagrams.
4. Verify that the Folder Layout block matches `Get-ChildItem -Recurse src`.

**Output:** Drift table and a patch for `ARCHITECTURE_C4.md` prose and mermaid diagrams.

## Workflow W8 — perf

**Goal:** Audit dependency footprints, binary size bloat, and compile-time configuration. Produce an actionable remediation plan.

**Tooling & Scripts:**
- Automation runner: `.opencode/skills/code-quality/scripts/audit-perf.py`
- Dependency analyzer: `.opencode/skills/code-quality/scripts/analyze-deps.py`
- Binary bloat analyzer: `.opencode/skills/code-quality/scripts/analyze-bloat.py`
- Reference best practices: `doc/distill/perf.md`

**Steps:**

1. Run `.opencode/skills/code-quality/scripts/analyze-deps.py` on the target package.
2. Identify heavy dependencies where exclusive crate count exceeds 50 packages. Check for unneeded default features.
3. Check for duplicate crate versions using `cargo tree -d` or `analyze-deps.py`.
4. Run `.opencode/skills/code-quality/scripts/analyze-bloat.py`. Audit release profile settings in `Cargo.toml` and `.cargo/config.toml` (`strip = true`, `lto`, `codegen-units = 1`, `opt-level = "z"`, `panic = "abort"`).
5. Audit static embedded assets (e.g. `typst-kit-embed-fonts`) that inflate executable sizes.
6. Verify linker configuration in `.cargo/config.toml` (e.g. `lld-link` on Windows MSVC, `mold` on Linux).
7. Flag dev profile settings that unnecessarily generate heavy debuginfo (`debug = "line-tables-only"` recommended).

**Output:** Performance audit table with findings, metrics, and remediation plan.

## Report Format

Produce one report for the requested workflow(s). Use this structure:

```markdown
## Code Quality Report — <workflow> — <scope>

### Findings
| ID | Severity | Location | Summary | Evidence | Recommendation |
|----|----------|----------|---------|----------|----------------|
| W1-01 | HIGH | src/app/foo.rs:42 | Missing error-path test | fn foo Err branch | Add test foo_returns_err_on_empty |

### Metrics
- Total items checked: N
- Findings by severity: CRITICAL x, HIGH y, MEDIUM z, LOW w
- Alignment or coverage %: N%

### Remediation Plan
Ordered tasks. Each task states: objective, affected file(s), steps, and test impact.

### Next Actions
- State if you must fix CRITICAL items first or can proceed.
- Ask: "Do you approve the plan? Reply yes to generate patches."
```

Rules for the report:

- Limit findings to 50 rows. Aggregate excess in an overflow summary.
- Use severity: CRITICAL (blocks function or violates MUST), HIGH (duplicate, conflict, or untestable), MEDIUM (drift or missing non-functional), LOW (style or minor improvement).
- Reruns without code changes must produce the same IDs and counts.
- Keep sentences short. Limit instruction sentences to 30 words. Limit descriptive sentences to 45 words.
- Use active voice and imperative mood for steps.

## Quality Gate

After you produce a remediation plan, remind the user of the quality gate. The gate requires:

- `cargo check --quiet` — no errors or warnings
- `cargo nextest run --status-level fail --show-progress none` — all tests pass
- `cargo clippy -- -D warnings` — no lint warnings
- `cargo fmt --check` — format is correct
- `cargo doc --no-deps --quiet` — docs build without warnings

Do not mark a task complete until all gates pass.

## Constraints

- Do not edit `SPEC.md` or `ARCHITECTURE_C4.md` without explicit approval.
- Do not invent requirements. Cite the file and line that supports each finding.
- Keep the skill read-only. Patch generation requires a separate build-mode agent.

