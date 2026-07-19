<!--
Sync Impact Report:
- Version change: 0.0.0 -> 1.0.0
- Modified principles: 
  - New: I. Testability
  - New: II. Security
  - New: III. Modularity
  - New: IV. Open Source Leverage
  - New: V. SDLC Best Practices
- Added sections: Governance
- Removed sections: N/A
- Templates requiring updates:
  - ✅ `.specify/templates/plan-template.md` (no changes needed)
  - ✅ `.specify/templates/spec-template.md` (no changes needed)
  - ✅ `.specify/templates/tasks-template.md` (no changes needed)
- Follow-up TODOs: N/A
-->
# Personal Productivity AI Constitution

## Core Principles

### I. Testability
Code MUST be testable. Write modular code with minimal side effects to enable unit and functional testing. Ensure all bugs are accompanied by a regression test and coverage is maintained or improved with changes.

### II. Security
Code MUST be secure. Validate all inputs, sanitize data, and follow secure coding guidelines to prevent vulnerabilities.

### III. Modularity
Features MUST be modular. Keep components focused, self-contained, and independently verifiable. Refrain from sweeping, massive refactors in a single pass; instead work in small, manageable iterations.

### IV. Open Source Leverage
The project SHOULD use open source libraries wherever applicable instead of reinventing the wheel, ensuring they meet the project's security and licensing constraints.

### V. SDLC Best Practices
Development MUST follow SDLC best practices. This includes test-driven changes, fixing all warnings before completion, keeping tests updated, maintaining a clean compilation step, and refusing tasks if requirements are unclear.

## Governance

Amendments to this constitution require documentation and a proposed revision.
All new features and fixes MUST verify compliance with the core principles.
Ensure that any new implementation strategy aligns with the provided templates (`spec-template.md`, `plan-template.md`, `tasks-template.md`).

**Version**: 1.0.0 | **Ratified**: 2026-07-19 | **Last Amended**: 2026-07-19
