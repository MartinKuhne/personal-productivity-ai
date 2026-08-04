---
name: create-pr
description: Create a pull request with a comprehensive description including objective, changes, rationale, and quality evidence. Use when the user asks to create a PR, push changes as a PR, or open a pull request for completed work.
---

# Create PR Skill

When the user asks to create a pull request, follow this workflow **in order**.

## 1. Gather Context

Before writing anything, understand what was changed:

- Run `git status` to see working tree state
- Run `git diff --staged` to see staged changes
- Run `git diff` to see unstaged changes
- Run `git log --oneline -5` to see recent commits
- If there are multiple commits, run `git log --oneline <base-branch>..HEAD` to see all commits on this branch
- Read the relevant files that were changed to understand the scope

If the working tree is clean and there are no commits yet, check:
- `git log --oneline -5` to see if commits exist
- `git branch --show-current` to confirm the current branch
- Ask the user if they want to commit first or if changes are elsewhere

## 2. Identify the Base Branch

Determine where the PR should target:
- Check `git remote -v` to find the remote
- Run `gh pr list --head <current-branch> --state open` to check if a PR already exists
- If no base is specified, default to `main` or `master` (check `git symbolic-ref refs/remotes/origin/HEAD` or inspect the repo)

## 3. Determine the Objective

From the changes and any user input, articulate:
- **What problem** was being solved or what feature was being built
- **What goal** this change achieves

If the user provided context about the objective, use theirs. Otherwise, infer from:
- Commit messages
- File names and paths changed
- Code patterns (bug fix vs feature vs refactor)
- Any issue/PR references in commit messages (e.g., "Fixes #123")

## 4. Analyze the Changes

For each significant change, determine:
- **What** was changed (files, functions, behaviors)
- **Why** it was changed (the rationale — reference requirements, bugs, or improvements)
- **How** it addresses the objective (implementation approach)

Group changes logically:
- New features / capabilities
- Bug fixes
- Refactoring / cleanup
- Configuration / infrastructure changes
- Test changes

## 5. Verify Quality Gates

Before creating the PR, verify that quality standards are met. Document evidence for each:

### Build / Compilation
- Run the project's build command (check README or package.json/Cargo.toml/go.mod for the appropriate command)
- Examples: `cargo check`, `npm run build`, `go build ./...`, `dotnet build`
- **Evidence:** Copy the output showing success, or note any warnings/errors

### Linting / Formatting
- Run the project's lint command (check package.json, Makefile, or CI config)
- Examples: `cargo clippy`, `npm run lint`, `eslint`, `ruff check`
- **Evidence:** Copy the output showing clean linting

### Tests
- Run the project's test command
- Examples: `cargo test`, `npm test`, `pytest`, `go test ./...`
- **Evidence:** Copy the output showing all tests pass, including test counts if available

### Static Analysis
- If the project has additional analysis tools (mypy, deadcode, etc.), run them
- **Evidence:** Copy output showing no issues

### Security / Secrets Scan
- If secrets scanning is available (e.g., `gitleaks`, `trufflehog`), run it on changed files
- **Evidence:** Copy output confirming no secrets detected

### Compilation Warnings
- Verify no new warnings were introduced
- **Evidence:** Note that build/clippy/lint ran cleanly with no warnings

If quality gates **fail**:
- Fix the issues if they are straightforward (typos, missing imports, obvious bugs)
- If fixes require user input or are non-trivial, **report the failures** and ask the user whether to proceed anyway

## 6. Check for Existing PRs

Before creating:
```bash
gh pr list --head <current-branch> --state open
```

If a PR already exists:
- Inform the user and show the existing PR URL
- Ask if they want to push new commits to update it instead (`git push origin <branch>`)

## 7. Create the Pull Request

Construct the PR body with this structure:

```markdown
## Objective

[Clear statement of what this change achieves and why]

## Changes

### [Category: e.g., New Feature, Bug Fix, Refactoring]
- **[File/path]:** [What changed and why]
- **[File/path]:** [What changed and why]

### [Category: ...]
- **[File/path]:** [What changed and why]

## How

[Brief explanation of the implementation approach and key design decisions]

## Why

[Context: what problem motivated this change? Reference issues/bugs if applicable]

## Quality Gates

- [x] Build: `[build command]` — `[evidence, e.g., "succeeded with 0 warnings"]`
- [x] Lint: `[lint command]` — `[evidence, e.g., "clean, no issues found"]`
- [x] Tests: `[test command]` — `[evidence, e.g., "all 42 tests passed"]`
- [ ] Static Analysis: `[command]` — `[evidence or "not configured"]`
- [ ] Secrets Scan: `[command]` — `[evidence or "not configured"]`
```

If a quality gate could not be verified, mark it as unchecked with a note explaining why.

Then create the PR:

```bash
gh pr create \
  --base <base-branch> \
  --head <current-branch> \
  --title "<concise title>" \
  --body "<PR body>"
```

If the PR body exceeds terminal limits, write it to a temporary file and use `--body-file`:
```bash
gh pr create \
  --base <base-branch> \
  --head <current-branch> \
  --title "<title>" \
  --body-file <temp-file>
```

## 8. Report Results

Tell the user:
- The PR was created successfully
- The PR URL
- A summary of what was included in the PR description
- Any quality gates that were skipped or failed
- Whether they should review the PR description before it is finalized

## Important Rules

1. **Never create a PR without verifying quality gates first** — always run at minimum build and tests
2. **Always document evidence** — copy actual command output, don't just say "passed"
3. **Never commit or push without user confirmation** — the user may have uncommitted changes they want to handle differently
4. **Check for existing PRs** to avoid duplicates
5. **Be honest about what you verified** — if a command failed, report it clearly
6. **Keep the PR title concise** (under 72 characters, imperative mood)
7. **If the branch has no commits yet**, inform the user that commits are needed before a PR can be created
