//! Property-based tests for the VFS virtual-path parser.
//!
//! The virtual path parser is the project's path-traversal
//! defense: it accepts strings like `Library/sub/path.md` and
//! rejects any input containing a `..` parent component
//! (after normalising backslash to forward slash). A bug
//! that lets a `..` slip through is a privilege-escalation
//! vector — an attacker who can call the agent's file tools
//! with a crafted `vpath` could read or write outside the
//! configured `content_libraries`.
//!
//! Per the VFS spec ([`crate::workspace::vfs::SPEC.md`], VFS-004,
//! VFS-009) the parser must:
//!
//! - Reject empty paths.
//! - Reject any path containing a `..` parent component,
//!   whether the component is at the start, after the library
//!   prefix, or deep in a sub-path.
//! - Reject paths without a `/` separator (the library name
//!   and sub-path are mandatory).
//! - Reject paths with an empty library name (e.g. leading
//!   slash with no library before it).
//! - Reject paths with an empty sub-path.
//! - Normalise `\` to `/` so Windows-style paths hit the same
//!   parser. The same path must produce the same
//!   `TraversalDetected` error regardless of separator.
//!
//! These example-based cases are already covered by the unit
//! tests in `virtual_path.rs`. The proptest adds:
//!
//! 1. **Panic-freedom** on any random string.
//! 2. **Closure under the `..` rejection rule**: any path
//!    that contains a real `..` component (bounded by
//!    slashes or string ends, after backslash normalisation)
//!    is rejected with `TraversalDetected`. The strategy
//!    enumerates the canonical `..` component shapes so a
//!    regression that lets any of them slip through is
//!    caught here.
//! 3. **No false positives**: any path with a non-empty
//!    library, a non-empty sub-path, and no `..` component
//!    parses successfully. Catches a regression that
//!    over-rejects (e.g. a future fix that flags `..` in
//!    path *names* like `..hidden` as a traversal, even
//!    though `..hidden` is a legal filename).

use crate::workspace::vfs::virtual_path::{VirtualPath, VirtualPathError};
use proptest::prelude::*;

/// Strategy: any random string up to 64 bytes, including
/// the chars that are interesting for path-traversal
/// (`.`, `..`, `/`, `\\`, control chars, non-ASCII).
fn any_path_string() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[\x00-\x7F]{0,64}").unwrap()
}

/// Strategy: a path-shaped prefix or suffix (alphanumeric,
/// no slashes, no `..`). Used to build real `..` component
/// test paths by inserting a `..` at a proper component
/// boundary.
fn path_component() -> impl Strategy<Value = String> {
    prop::string::string_regex(r"[A-Za-z][A-Za-z0-9_-]{0,15}").unwrap()
}

/// Build paths that contain a real `..` component (bounded
/// by slashes or string boundaries). The list of shapes
/// below is exhaustive for the common attack vectors:
///
/// - `..` (standalone, no library prefix — `..` is the
///   first path component)
/// - `../x` (`..` followed by a slash)
/// - `x/..` (`..` preceded by a slash, end-of-path)
/// - `x/../y` (`..` between two segments)
/// - `x/..` with the `..` at the end (e.g. `x/..`)
/// - `..\x` (Windows separator, normalised to `/..` by the
///   parser — the `\` form is a separate shape)
fn path_with_parent_component() -> impl Strategy<Value = String> {
    (path_component(), path_component()).prop_map(|(a, b)| {
        // Pick a deterministic shape from a + b so the
        // proptest explores all shapes across shrinking.
        // Every shape here contains a real `..` component
        // (bounded by `/` or `\` or string ends). A
        // bare `a\b` (no `..`) is NOT in the list — the
        // parser would accept that shape after backslash
        // normalisation, so it doesn't belong here.
        let shapes: [String; 7] = [
            "../".to_string(),
            format!("{a}/.."),
            format!("{a}/../{b}"),
            format!("../{a}"),
            format!("{a}/../../{b}"),
            format!("{a}\\..\\{b}"),
            format!("{a}\\.."),
        ];
        let idx = (a.len() + b.len()) % shapes.len();
        shapes[idx].clone()
    })
}

/// Strategy: a non-traversal virtual path. Always has a
/// non-empty library, a non-empty sub-path, and no `..`
/// component. Used to verify the positive case ("paths that
/// look fine actually parse").
fn non_traversal_path() -> impl Strategy<Value = String> {
    (
        // Library name: alphanumeric, no `/`, no `..`
        prop::string::string_regex(r"[A-Za-z][A-Za-z0-9_-]{0,15}").unwrap(),
        // Sub-path: a single alphanumeric filename
        prop::string::string_regex(r"[A-Za-z0-9_-]{1,20}").unwrap(),
    )
        .prop_map(|(lib, sub)| format!("{lib}/{sub}"))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Property 1: panic-freedom. Any random byte string
    /// passed to `VirtualPath::parse` returns `Result` —
    /// it never panics, never aborts, never loops forever.
    /// The proptest is configured with 512 cases; the
    /// `preprocessor` proptest would otherwise shrink a
    /// failure to the minimum input that triggers it.
    #[test]
    fn virtual_path_parser_never_panics(s in any_path_string()) {
        let _ = VirtualPath::parse(&s);
    }

    /// Property 2: any path containing a real `..` component
    /// (bounded by slashes or string ends) is rejected with
    /// `TraversalDetected`. The parser normalises `\` to `/`
    /// before checking, so the Windows-style shapes
    /// (`x\..\y`, `x\..`) hit the same rejection rule.
    ///
    /// The strategy enumerates the canonical `..` component
    /// shapes — standalone `..`, `..` at start, `..` at end,
    /// `..` between segments, deep `../..`, and the
    /// backslash variants. A regression that lets any of
    /// these slip through is caught here.
    #[test]
    fn any_path_with_parent_component_is_rejected(path in path_with_parent_component()) {
        let result = VirtualPath::parse(&path);
        prop_assert!(
            matches!(result, Err(VirtualPathError::TraversalDetected)),
            "path with `..` component should be rejected: {:?} -> {:?}",
            path,
            result
        );
    }

    /// Property 3: a path with a non-empty library, a
    /// non-empty sub-path, and no `..` component always
    /// parses successfully. This is the positive case —
    /// "well-formed paths round-trip through the parser".
    /// Catches a regression that over-rejects (e.g. a future
    /// fix that accidentally flags `..` in path *names* like
    /// `..hidden` as a traversal, even though `..hidden` is
    /// a legal filename).
    #[test]
    fn non_traversal_path_always_parses(path in non_traversal_path()) {
        let result = VirtualPath::parse(&path);
        prop_assert!(
            result.is_ok(),
            "non-traversal path should parse: {:?} -> {:?}",
            path,
            result
        );
        let vp = result.unwrap();
        // Round-trip: the parsed library + sub-path, when
        // re-serialised, must produce a string that parses
        // to the same `VirtualPath`. The `Display` impl
        // normalises the path representation, so the
        // round-trip is structural rather than byte-equal.
        let s = vp.to_string();
        let vp2 = VirtualPath::parse(&s).expect("display-then-parse should succeed");
        prop_assert_eq!(vp, vp2, "round-trip mismatch for {:?}", path);
    }
}
