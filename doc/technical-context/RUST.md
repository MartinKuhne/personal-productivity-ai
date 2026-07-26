# Coding guidelines

AI code agents MUST download and follow https://microsoft.github.io/rust-guidelines/agents/all.txt

# Rust Style Guide — Actionable Reference

*Distilled from [The Rust Style Guide](https://doc.rust-lang.org/style-guide/)*

---

## Formatting (enforced by `rustfmt`)

### Indentation & Layout
- **Indent:** 4 spaces (no tabs)
- **Line width:** 100 characters max
- **Indent style:** Block indent (not visual indent)
  ```rust
  // ✓ Block indent
  fn_call(
      arg1,
      arg2,
  );
  // ✗ Visual indent
  fn_call(arg1,
          arg2);
  ```
- **Trailing commas:** Always when followed by newline
  ```rust
  let arr = [a, b, c,];
  fn_call(a, b,);
  ```
- **Blank lines:** 0 or 1 blank line between items/statements
- **No trailing whitespace** (including in comments/strings)

### Imports & Attributes
- **Sort imports** with version sort (e.g., `u8` < `u16` < `u128`)
- **Attributes:** one per line, indented to item level
  ```rust
  #[repr(C)]
  #[derive(Debug, Clone)]
  struct Foo;
  ```
- **`derive`:** Single attribute, comma-separated: `#[derive(Debug, Clone)]`
- **Attribute args:** space around `=`: `#[foo = 42]`

### Comments
- **Prefer `//`** over `/* */`
- **Space after sigil:** `// comment`
- **Doc comments:** Prefer outer `///` over inner `//!` or block `/** */`
- **Doc comments before attributes**
- **Line comments:** Max 80 chars (excl. indent) or line width, whichever is smaller
- **Inline block comment:** space inside: `/* comment */`

### Small Items (single-line when small)
Tools decide "small" by size/complexity. Prefer single-line for simple structs/enums:
```rust
// ✓ Small
Foo { a: 1, b: 2 }
Foo { a, b }
// ✗ Large — block form
Foo {
    a: very_long_expression(),
    b: another_long_expression(),
}
```

---

## Non-Formatting Conventions (human-enforced)

### Naming
| Kind | Convention |
|------|------------|
| Crates, modules, packages | `snake_case` |
| Types (structs, enums, traits, type params) | `UpperCamelCase` |
| Functions, methods, variables, modules | `snake_case` |
| Constants, statics | `SCREAMING_SNAKE_CASE` |
| Macros | `snake_case!` |
| Lifetimes | `'short` or `'a`, `'b`... |
| Features (Cargo) | `snake-case` (kebab-case) |

### Cargo.toml
- `name` = package name in `kebab-case`
- `description` = one sentence, no trailing period
- `license` = SPDX identifier
- `repository` = public repo URL
- `categories`, `keywords` = lowercase, kebab-case
- Dependencies sorted alphabetically
- Versions: prefer `^1.0` (caret), avoid `*` or exact `=`

### Code Organization
- **Modules:** `mod foo;` + `foo/` dir or `foo.rs` (not both)
- **Prelude:** Re-export commonly used items in `prelude` module
- **Prelude imports:** `use crate::prelude::*;` in module root
- **Re-exports:** Use `pub use` in parent module

### Safety & Correctness
- **`unsafe`**: Document invariants in `// SAFETY:` comment above block
- **`unsafe` blocks:** Minimize scope; one operation per block when possible
- **`unsafe` functions:** Document preconditions in doc comment
- **`unwrap()`/`expect()`:** Avoid in library code; use `?` or `Result`
- **Panic messages:** Use `expect("context")` with context, not `unwrap()`

### API Design
- **Builders:** Prefer builder pattern for complex construction
- **Builders:** `Default` impl when sensible
- **Into/From:** Implement `From` (not `Into`) for conversions
- **Iterators:** Return `impl Iterator` when possible; `IntoIterator` for collections
- **Errors:** Use `thiserror`/`anyhow`; implement `std::error::Error`
- **Async:** Return `impl Future` or boxed dyn; avoid `async fn` in public traits (object safety)

### Testing
- **Unit tests:** `#[cfg(test)] mod tests { ... }` in same file
- **Integration tests:** `tests/*.rs` (separate crate)
- **Doc tests:** `/// ```rust ... ```` in doc comments
- **`#[should_panic]`** for expected panics

---

## Tooling
- **Format:** `cargo fmt` (uses `rustfmt` with default style)
- **Lint:** `cargo clippy -- -D warnings`
- **Check:** `cargo check` (fast type-check)
- **Test:** `cargo test`
- **Docs:** `cargo doc --no-deps`

---

## Quick Reference Card

| Rule | Action |
|------|--------|
| Indent | 4 spaces |
| Line length | ≤ 100 chars |
| Trailing comma | Yes, when multiline |
| Block indent | Yes (not visual) |
| Blank lines | 0–1 between items |
| Trailing whitespace | Never |
| Imports | Version-sorted |
| `derive` | Single attribute |
| Attributes | One per line |
| Comments | `// ` with space |
| Doc comments | `///` outer |
| `unsafe` | Minimize scope, document |
| `unwrap` | Avoid in libs |

---

## Sources
- [The Rust Style Guide](https://doc.rust-lang.org/style-guide/) (official, accessed 2026-07-17)
- [rustfmt Configuration](https://github.com/rust-lang/rustfmt/blob/master/Configurations.md)
- [API Guidelines](https://rust-lang.github.io/api-guidelines/) (for API design)
- [Microsoft Pragmatic Rust Guidelines](https://microsoft.github.io/rust-guidelines/) (v2026.6)
  - **Full reference:** `https://microsoft.github.io/rust-guidelines/agents/all.txt` (concatenated guidelines for LLM consumption)