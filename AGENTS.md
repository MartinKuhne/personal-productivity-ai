# Agent Instructions

## Anti-Hallucination protocol

- Any statement that is not backed by factual evidence is considered unverified
- If you realize you made an unverified claim, immediately state:
> "Correction: My previous statement was unverified. I should have labeled it as [appropriate label]"

### MANDATORY LABELS (use at START of any unverified statement)
- [SPECULATION] - For logical guesses
- [INFERENCE] - For pattern-based conclusions  
- [UNVERIFIED] - For anything you cannot confirm
- [GENERALIZATION] - For broad statements about groups/categories

## Development process
- Break tasks into logical, contained and testable steps. Confirm tests pass between steps.
- When the current branch is 'main' you MUST create a branch before changing ANY files.
- You MUST name the branch feature/<change>, bugfix/<change> or chore/<change>.
- Compile and unit test MUST succeed before starting any work.
- Quality gates MUST pass to call a task complete.
- You SHOULD assess if the existing codebase is suitable for the modification you are being asked to make. You SHOULD suggest to the user any refactorings needed before implementation can start.
- You MUST refuse a task if the requirements or context are unclear. You MUST ask clarifying questions.

## Test-Driven development
- All changes MUST be covered by unit tests.
- All changes SHOULD be covered by integration tests.
- When asked to fix a bug, create a failing test first. The test MUST reproduce the issue. Then make the code change. Then prove the code change works because the test passes.

## Code
- You MUST Write modular code with minimal side effects. Functions SHOULD be pure and honest.
- You MUST use string constants for repeat strings or user-facing literals.
- You SHOULD use open source and well-maintained libraries over hand-coding equivalent functions.
- Prefer splitting large functions, extracting helpers, and reducing nesting over introducing additional branches into already-complex code.

## Component-specific rules

| Directory                    | Scope                                                                                  |
|------------------------------|----------------------------------------------------------------------------------------|
| [`src/desktop/AGENTS.md`](src/desktop/AGENTS.md)               | Rust `fastmd` crate: documentation, egui, quality gate, tool/UI contracts. |
| [`src/android/AGENTS.md`](src/android/AGENTS.md)               | Android Kotlin/Gradle companion app.                                                  |
| [`doc/technical-context/AGENTS.md`](doc/technical-context/AGENTS.md) | Maintenance of architecture documentation.                |
| [`doc/planning/AGENTS.md`](doc/planning/AGENTS.md)             | Planning / design-record documents.                                                   |
| [`src/test/wiki/AGENTS.md`](src/test/wiki/AGENTS.md)           | Test wiki fixtures.                                                                   |

<!-- CODEGRAPH_START -->
## CodeGraph

In repositories indexed by CodeGraph (a `.codegraph/` directory exists at the repo root), reach for it BEFORE grep/find or reading files when you need to understand or locate code:

- **MCP tool** (when available): `codegraph_explore` answers most code questions in one call — the relevant symbols' verbatim source plus the call paths between them, including dynamic-dispatch hops grep can't follow. Name a file or symbol in the query to read its current line-numbered source. If it's listed but deferred, load it by name via tool search.
- **Shell** (always works): `codegraph explore "<symbol names or question>"` prints the same output.

If there is no `.codegraph/` directory, skip CodeGraph entirely — indexing is the user's decision.
<!-- CODEGRAPH_END -->
