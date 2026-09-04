//! Tests for `ui/attributions.rs`

use super::*;

#[test]
fn all_entries_have_valid_fields() {
    assert_eq!(
        DIRECT_DEPENDENCIES.len(),
        52,
        "expected 52 direct dependencies, got {}",
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

    let workspace_members: std::collections::HashSet<String> =
        ["fastmd", "fastmd-agent", "fastmd-pdf", "fastmd-tool-macros"]
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
                // Filter workspace members.
                if workspace_members.contains(name) {
                    continue;
                }
                // Skip known non-crate entries like `fastmd-pdf` optional internal path dep already filtered,
                // but also filter by presence of version/path shape.
                expected.insert(name.to_string());
            }
        }
    }

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
