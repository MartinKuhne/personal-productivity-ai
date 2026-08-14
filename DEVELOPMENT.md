# Development notes

## Setup

- `cargo install --locked cargo-nextest`
- `cargo install --git https://github.com/rerun-io/kittest_inspector`

## Regular code inspections

- Audit compliance with the [Microsoft Rust guidelines](https://microsoft.github.io/rust-guidelines/agents/all.txt)
- Audit compliance with know good software engineering patterns
  - SOLID
  - Fluent interfaces for creating delightful APIs
  - Builder pattern to encapsulate data and perform initialization
  - Immutable data structures
  - Functional programming patterns
- Audit compliance with rust specific patterns
- Module separation and cross-cutting concerns
- Refactor tests into module_tests.rs
- Remove tactical notes from code comments and ensure public functions have comments
- Audit Unit, functional, integration, fuzz test coverage
- Spec drift
- NFR: Logging, tracing


