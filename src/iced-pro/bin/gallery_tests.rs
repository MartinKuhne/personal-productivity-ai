//! Guard tests for the gallery binary's output-path refusal.
//!
//! These exercise the pure [`refusal`] predicate only. Nothing here touches
//! the filesystem, so no test can create or destroy a real user path
//! (**RUST-006**).
//!
//! The hole these tests exist to keep shut: the guard originally compared
//! the candidate path to each forbidden root for *equality*. Refusing
//! `%APPDATA%` while accepting `%APPDATA%\fastmd-snapshot` protects
//! nothing — a subdirectory of the config root is the config root for
//! these purposes. That bug let a full 175-cell snapshot land in
//! `%APPDATA%` while the guard was believed to be working.

use super::*;

/// The three roots the guard exists to keep writes out of.
const ROOT_VARS: [&str; 3] = FORBIDDEN_ROOT_VARS;

#[test]
fn refuses_each_root_exactly() {
    for var in ROOT_VARS {
        let Ok(value) = std::env::var(var) else {
            continue;
        };
        assert!(
            refusal(Path::new(&value)).is_some(),
            "${var} root ({value}) must be refused"
        );
    }
}

#[test]
fn refuses_a_subdirectory_of_each_root() {
    // The regression this whole module was written for.
    for var in ROOT_VARS {
        let Ok(value) = std::env::var(var) else {
            continue;
        };
        let nested = PathBuf::from(&value).join("iced-pro-snapshot");
        let reason = refusal(&nested);
        assert!(
            reason.is_some(),
            "${var} subdirectory must be refused, got None for {}",
            nested.display()
        );
        assert!(reason.unwrap().contains(var), "the reason must name ${var}");
    }
}

#[test]
fn refuses_a_deeply_nested_subdirectory_of_each_root() {
    for var in ROOT_VARS {
        let Ok(value) = std::env::var(var) else {
            continue;
        };
        let nested = PathBuf::from(&value).join("a").join("b").join("fastmd-out");
        assert!(
            refusal(&nested).is_some(),
            "a nested path under ${var} must be refused: {}",
            nested.display()
        );
    }
}

#[test]
fn refuses_a_fastmd_component_anywhere() {
    for candidate in [
        "/tmp/.fastmd-gallery",
        "/somewhere/.fastmdx/out",
        "/data/notes/.fastmd",
    ] {
        assert!(
            refusal(Path::new(candidate)).is_some(),
            "a {FASTMD_MARKER}* component must be refused: {candidate}"
        );
    }
}

#[test]
fn a_temp_subdirectory_is_allowed() {
    // The default output lives in the temp tree; the rule is about the
    // app's own config roots, not about temp.
    let temp = std::env::temp_dir().join("iced-pro-gallery");
    assert!(
        refusal(&temp).is_none(),
        "a temp subdirectory must be allowed: {}",
        temp.display()
    );
}

#[test]
fn matching_is_case_insensitive_and_separator_normalised() {
    let Ok(appdata) = std::env::var("APPDATA") else {
        return;
    };
    let upper = appdata.to_uppercase();
    assert!(
        refusal(Path::new(&upper)).is_some(),
        "case must not defeat the guard: {upper}"
    );
    let posix = appdata.replace('\\', "/");
    assert!(
        refusal(Path::new(&posix)).is_some(),
        "POSIX separators must not defeat the guard: {posix}"
    );
}

#[test]
fn a_root_with_a_trailing_separator_still_nests_correctly() {
    // A trailing slash on the env var must not turn `root//child` into a
    // non-match.
    let Ok(appdata) = std::env::var("APPDATA") else {
        return;
    };
    let nested = format!(
        "{}{}iced-pro-out",
        appdata.trim_end_matches(['/', '\\']),
        "/x"
    );
    assert!(
        refusal(Path::new(&nested)).is_some(),
        "a nested path must still be refused: {nested}"
    );
}

#[test]
fn containment_is_component_wise_not_string_wise() {
    // A sibling that merely shares a string prefix is not "below".
    assert!(
        !at_or_below("/tmp/iced-pro-gallery-evil", "/tmp/iced-pro-gallery"),
        "a string-prefix sibling must not count as nested"
    );
    assert!(at_or_below(
        "/tmp/iced-pro-gallery",
        "/tmp/iced-pro-gallery"
    ));
    assert!(at_or_below(
        "/tmp/iced-pro-gallery/cells",
        "/tmp/iced-pro-gallery"
    ));
    assert!(!at_or_below("/tmp", "/tmp/iced-pro-gallery"));
}
