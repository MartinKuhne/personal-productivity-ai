//! Tests for `ui/attributions.rs`

use super::*;

/// Repo root — the `fastmd` package's manifest is the workspace root (`[lib]
/// path = "src/app/lib.rs"`), so `CARGO_MANIFEST_DIR` is the root and
/// `Cargo.lock` sits next to it.
fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// The text of the committed `Cargo.lock` at the repo root.
fn committed_cargo_lock() -> String {
    let path = repo_root().join("Cargo.lock");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
}

/// The framework crate whose own manifest is deliberately not in this repo.
///
/// It is excluded from the third-party attribution catalog because it is our
/// own framework, while the crates it pulls in still ship in the binary.
const FRAMEWORK_PACKAGE: &str = "hoarfrost";

/// Dependency names declared by the `[[package]]` block called `package` in the
/// `Cargo.lock` text `lock`.
///
/// # Why the lock file is the authority
///
/// `hoarfrost` stopped being a workspace member on 2026-11-01, so its
/// `Cargo.toml` is not in this repo and cannot be scanned like the workspace
/// manifests (see `doc/planning/hoarfrost-external-crate-plan.md`). Its direct
/// dependencies still compile into the shipped binary and must stay attributed.
///
/// A hand-copied list of them cannot enforce that: cargo rewrites `Cargo.lock`
/// whenever the framework version moves, so a dependency added or dropped
/// upstream changes this set in the very commit that changes what ships. A
/// restated list only fires when a human remembers to edit it, and passes
/// silently when upstream adds a crate nobody copied — exactly the way an
/// unattributed crate ends up in the binary. Deriving from the lock closes that
/// hole in both directions: added deps appear here automatically, dropped ones
/// disappear.
///
/// The rule keys on the package **name**, never on its `source` scheme. The
/// framework was fetched from a `git+` rev until 2026-11-01 and from the
/// crates.io registry after; the attribution obligation is identical either
/// way, and a source-shaped rule broke the moment the source changed.
///
/// Parsing follows this file's existing style: plain line scanning, no TOML
/// crate. A `[[package]]` header starts a block, `name = "…"` selects it, and
/// its `dependencies = [ … ]` entries are `"name"`, `"name version"` or
/// `"name version (source)"` — the name is everything before the first space.
fn external_framework_dependency_names(
    lock: &str,
    package: &str,
) -> std::collections::BTreeSet<String> {
    let mut names = std::collections::BTreeSet::new();
    // Whether the block currently being scanned is the framework's, and whether
    // the line reader is inside that block's dependency list.
    let mut block_is_framework = false;
    let mut in_framework_deps = false;

    for line in lock.lines() {
        let trimmed = line.trim();

        if trimmed == "[[package]]" {
            block_is_framework = false;
            in_framework_deps = false;
            continue;
        }

        if in_framework_deps {
            if trimmed == "]" {
                in_framework_deps = false;
            } else if let Some(entry) = trimmed.split('"').nth(1) {
                let name = entry.split(' ').next().unwrap_or(entry);
                if !name.is_empty() {
                    names.insert(name.to_string());
                }
            }
            continue;
        }

        if let Some(value) = trimmed.strip_prefix("name = ") {
            block_is_framework = value.trim().trim_matches('"') == package;
        } else if trimmed == "dependencies = [" && block_is_framework {
            in_framework_deps = true;
        }
    }

    names
}

#[test]
fn all_entries_have_valid_fields() {
    assert_eq!(
        DIRECT_DEPENDENCIES.len(),
        61,
        "expected 61 direct dependencies, got {}",
        DIRECT_DEPENDENCIES.len()
    );
    for attr in DIRECT_DEPENDENCIES {
        assert!(!attr.name.is_empty(), "empty name in {attr:?}");
        assert!(!attr.authors.is_empty(), "empty authors for {}", attr.name);
        assert!(
            !attr.github_url.is_empty(),
            "empty github_url for {}",
            attr.name
        );
        assert!(
            attr.github_url.starts_with("https://github.com/"),
            "github_url for {} must start with https://github.com/, got {}",
            attr.name,
            attr.github_url
        );
    }
}

#[test]
fn slice_is_sorted_and_unique() {
    let mut seen = std::collections::HashSet::new();
    let mut prev: Option<&str> = None;
    for attr in DIRECT_DEPENDENCIES {
        assert!(seen.insert(attr.name), "duplicate crate name {}", attr.name);
        if let Some(p) = prev {
            assert!(
                p < attr.name,
                "slice not sorted: {p:?} should be < {:?}",
                attr.name
            );
        }
        prev = Some(attr.name);
    }
}

#[test]
fn completeness_against_cargo_manifests() {
    // Collect expected direct third-party deps from workspace manifests.
    // Workspace members to scan (relative to repo root).
    let manifest_paths = [
        "Cargo.toml",
        "src/agent/Cargo.toml",
        "src/md2pdf/Cargo.toml",
        "src/fastmd-tool-macros/Cargo.toml",
    ];

    // Crates of our own are excluded from the third-party attribution catalog
    // by design (see `attributions.rs` module doc). `hoarfrost` stays in this
    // set although it left the workspace on 2026-11-01: it is the project's own
    // framework, whether it is vendored in-tree or fetched from git, so it must
    // never appear in the About dialog's third-party list.
    let internal_crates: std::collections::HashSet<String> = [
        "fastmd",
        "fastmd-agent",
        "fastmd-pdf",
        "fastmd-tool-macros",
        "hoarfrost",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let mut expected = std::collections::BTreeSet::new();

    for rel in manifest_paths {
        let content =
            std::fs::read_to_string(rel).unwrap_or_else(|e| panic!("failed to read {rel}: {e}"));
        // Very small TOML parser: look for [dependencies] and [target.*.dependencies] sections.
        // We collect lines of form `name = ...` until next section header.
        let mut in_deps = false;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                // Section header.
                // Treat [dependencies], [dev-dependencies] is skipped, [build-dependencies] skipped,
                // and [target.*.dependencies] as dependencies.
                let is_deps = trimmed == "[dependencies]"
                    || (trimmed.starts_with("[target.") && trimmed.ends_with("dependencies]"));
                let is_dev_or_build = trimmed == "[dev-dependencies]"
                    || trimmed == "[build-dependencies]"
                    || (trimmed.starts_with("[target.") && trimmed.contains("dev-dependencies"))
                    || (trimmed.starts_with("[target.") && trimmed.contains("build-dependencies"));
                if is_deps {
                    in_deps = true;
                } else if is_dev_or_build {
                    in_deps = false;
                } else if trimmed.starts_with('[') {
                    // Any other section ends deps.
                    in_deps = false;
                }
                continue;
            }
            if !in_deps {
                continue;
            }
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            // Dependency line: `crate-name = "version"` or `crate-name = { ... }`
            if let Some(eq_pos) = trimmed.find('=') {
                let name = trimmed[..eq_pos].trim().trim_matches('"').trim();
                // Skip empty or comment-like.
                if name.is_empty() || name.starts_with('#') {
                    continue;
                }
                // Filter our own crates.
                if internal_crates.contains(name) {
                    continue;
                }
                // Skip known non-crate entries like `fastmd-pdf` optional internal path dep already filtered,
                // but also filter by presence of version/path shape.
                expected.insert(name.to_string());
            }
        }
    }

    // `hoarfrost`'s own manifest is not in this repo, so its direct deps
    // cannot be parsed above; they still ship in the binary, so they are read
    // out of the committed lock file, which cargo rewrites on every framework
    // bump, whichever source the framework comes from.
    expected.extend(external_framework_dependency_names(
        &committed_cargo_lock(),
        FRAMEWORK_PACKAGE,
    ));

    // The manifest set is authoritative; ensure our parser found at least that many.
    // Some manifests include optional deps (e.g., rfd) which are direct deps but optional.
    // They should still be counted as direct dependencies per spec (all direct across all crates).
    // Verify that DIRECT_DEPENDENCIES covers every expected name.
    let actual: std::collections::BTreeSet<String> = DIRECT_DEPENDENCIES
        .iter()
        .map(|a| a.name.to_string())
        .collect();

    let missing: Vec<_> = expected.difference(&actual).cloned().collect();
    let extra: Vec<_> = actual.difference(&expected).cloned().collect();

    assert!(
        missing.is_empty(),
        "DIRECT_DEPENDENCIES missing crates found in Cargo.toml manifests: {missing:?}; expected={expected:?} actual={actual:?}"
    );
    assert!(
        extra.is_empty(),
        "DIRECT_DEPENDENCIES contains extra crates not in Cargo.toml manifests: {extra:?}; expected={expected:?} actual={actual:?}"
    );
}

// ---------------------------------------------------------------------------
// Anti-vacuity tests for the lock-file derivation
// ---------------------------------------------------------------------------

/// A lock file in which the framework package is **registry**-sourced — the
/// shape since 2026-11-01 — and carries two dependencies, next to two other
/// packages whose own dependency lists must never be reported.
const SYNTHETIC_LOCK: &str = r#"# This file is automatically @generated by Cargo.
# It is not intended for manual editing.
version = 4

[[package]]
name = "hoarfrost"
version = "0.14.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "59a9500d50caf85de4354380b5bb0e0cabf74e765c180fd6f1e01520b0fe145a"
dependencies = [
 "alpha",
 "beta 1.4.2 (registry+https://github.com/rust-lang/crates.io-index)",
]

[[package]]
name = "gamma"
version = "0.1.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "0000000000000000000000000000000000000000000000000000000000000000"

[[package]]
name = "widget"
version = "2.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "1111111111111111111111111111111111111111111111111111111111111111"
dependencies = [
 "delta",
 "gamma",
]
"#;

#[test]
fn lock_derivation_finds_the_framework_in_the_committed_lock() {
    // The real lock must produce a non-empty set: an empty result would make
    // the completeness check silently stop covering the framework's deps.
    let names = external_framework_dependency_names(&committed_cargo_lock(), FRAMEWORK_PACKAGE);
    assert!(
        !names.is_empty(),
        "deriving {FRAMEWORK_PACKAGE}'s dependencies from the committed Cargo.lock produced nothing"
    );
}

#[test]
fn lock_derivation_finds_the_frameworks_shipped_dependencies() {
    let names = external_framework_dependency_names(&committed_cargo_lock(), FRAMEWORK_PACKAGE);

    // These are the crates `hoarfrost` hands to the compiler. If upstream adds
    // one it must show up here without this test being edited; if it drops one
    // it must disappear.
    for expected in ["iced", "iced_futures", "iced_renderer", "iced_runtime"] {
        assert!(
            names.contains(expected),
            "Cargo.lock's {FRAMEWORK_PACKAGE} package does not yield {expected:?}; got {names:?}"
        );
    }

    // `serde` reaches the binary through other packages only. Reporting it
    // would mean the scan swept up dependencies the framework never declared.
    assert!(
        !names.contains("serde"),
        "the lock scan reported a dependency the framework does not declare: {names:?}"
    );
}

#[test]
fn lock_derivation_reports_only_the_framework_packages_dependencies() {
    let names = external_framework_dependency_names(SYNTHETIC_LOCK, "hoarfrost");
    let expected: std::collections::BTreeSet<String> =
        ["alpha", "beta"].iter().map(|s| (*s).to_string()).collect();

    assert_eq!(names, expected, "unexpected framework dependency set");
}

/// The rule must survive the framework moving between sources: the same package
/// fetched from a `git+` rev has to yield the same set as the registry one.
/// This is the regression the source-shaped rule could not survive.
#[test]
fn lock_derivation_ignores_the_source_scheme() {
    let from_git = SYNTHETIC_LOCK.replace(
        "source = \"registry+https://github.com/rust-lang/crates.io-index\"\nchecksum = \"59a9500d50caf85de4354380b5bb0e0cabf74e765c180fd6f1e01520b0fe145a\"",
        "source = \"git+https://github.com/MartinKuhne/hoarfrost?rev=dd7af16dc0d1031adcd5b90e8c789b6bf6af2900#dd7af16dc0d1031adcd5b90e8c789b6bf6af2900\"",
    );
    assert_ne!(
        from_git, SYNTHETIC_LOCK,
        "the synthetic lock no longer has the shape this test rewrites"
    );

    assert_eq!(
        external_framework_dependency_names(&from_git, "hoarfrost"),
        external_framework_dependency_names(SYNTHETIC_LOCK, "hoarfrost"),
        "the framework's dependencies must not depend on how it was fetched"
    );
}

#[test]
fn lock_derivation_is_empty_without_the_framework_package() {
    let lock = SYNTHETIC_LOCK.replace("name = \"hoarfrost\"", "name = \"someone-elses-crate\"");
    assert!(
        external_framework_dependency_names(&lock, "hoarfrost").is_empty(),
        "a lock file with no framework package must yield no dependencies"
    );
    assert!(
        external_framework_dependency_names("", "hoarfrost").is_empty(),
        "an empty lock file must yield no dependencies"
    );
}
