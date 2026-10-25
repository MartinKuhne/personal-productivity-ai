//! Tests for the `UIF-*` spec-coverage ledger.
//!
//! Two groups. The first checks the ledger against itself: shape, no
//! duplicates, the three-way partition, and that a `Partial` or `NotOwned`
//! entry actually says something rather than gesturing.
//!
//! The second group is the reason the ledger exists. It reads
//! `spec.md` at this crate's root at test time, extracts every numbered
//! `UIF-*` id and asserts the ledger and the spec agree **in both
//! directions**, so an ID added to the spec without a classification here
//! fails the build, and a manifest entry the spec no longer contains fails
//! too. The spec file is in-repo and read-only: this test reads it and
//! writes nothing anywhere, which is what RUST-006 asks for.
//!
//! The injected-failure test (`test_audit_fires_on_an_unclassified_id`)
//! runs the real read and the real comparison over a doctored copy of the
//! spec text, so the guard is observed firing on every run rather than
//! trusted.

use std::path::{Path, PathBuf};

use crate::spec_coverage::SPEC_COVERAGE;
use crate::spec_coverage::{
    Coverage, EXPECTED_SPEC_ID_COUNT, Status, audit, extract_spec_ids, implemented, is_suffixed_id,
    manifest, not_owned, parent_id, partial, render, render_table, status_of, unimplemented,
};

/// The spec file, resolved from the crate directory.
///
/// The spec lives **inside** this crate as `spec.md`, so it resolves with no
/// traversal at all. That is a change of state, not a preference: the spec
/// used to sit at `doc/planning/ui-framework.md` and be read from two
/// directories up, and commit `edbe04a` moved it here. Nothing else in the
/// crate was repointed at the time, which is why this function is the only
/// functional breakage that move caused.
///
/// The file is read-only as far as this crate is concerned and in-repo — no
/// user path is touched (RUST-006).
fn spec_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("spec.md")
}

fn spec_text() -> String {
    let path = spec_path();
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read the spec at {}: {error}", path.display()))
}

/// A `Partial` reason must name what is missing, not gesture at it. These
/// are the markers the ledger's own convention admits.
const GAP_MARKERS: [&str; 4] = ["Missing", "Not covered", "Not cells", "deviation"];

/// Placeholders that must never stand in for a real justification.
const PLACEHOLDERS: [&str; 4] = ["TODO", "TBD", "FIXME", "XXX"];

fn ids_of(entries: &[Coverage]) -> Vec<&'static str> {
    entries.iter().map(|entry| entry.id).collect()
}

// ---------------------------------------------------------------------------
// Ledger shape
// ---------------------------------------------------------------------------

#[test]
fn test_manifest_carries_the_expected_total() {
    assert_eq!(
        manifest().len(),
        EXPECTED_SPEC_ID_COUNT,
        "the ledger must classify exactly the {} numbered UIF-* ids the spec carries",
        EXPECTED_SPEC_ID_COUNT,
    );
}

#[test]
fn test_manifest_has_no_duplicate_ids() {
    let ids = ids_of(&manifest());
    let unique: std::collections::BTreeSet<&str> = ids.iter().copied().collect();
    let mut duplicates: Vec<&str> = ids
        .iter()
        .copied()
        .filter(|id| ids.iter().filter(|other| *other == id).count() > 1)
        .collect();
    duplicates.sort_unstable();
    duplicates.dedup();
    assert!(
        duplicates.is_empty(),
        "duplicate ledger entries for {duplicates:?} ({} unique of {})",
        unique.len(),
        ids.len(),
    );
}

#[test]
fn test_every_entry_names_a_spec_section() {
    for entry in manifest() {
        assert!(
            !entry.section.trim().is_empty() && entry.section.starts_with('§'),
            "{} carries section {:?}; every entry must cite a spec section",
            entry.id,
            entry.section,
        );
    }
}

#[test]
fn test_every_status_has_a_label_and_is_one_of_three() {
    let all = [Status::Implemented, Status::Partial, Status::NotOwned];
    for status in all {
        assert!(!status.label().is_empty());
    }
    for entry in manifest() {
        assert!(
            all.contains(&entry.status),
            "{} has a status outside the taxonomy: {:?}",
            entry.id,
            entry.status,
        );
    }
}

#[test]
fn test_implemented_entries_carry_no_reason() {
    for entry in implemented() {
        assert!(
            entry.reason.is_none(),
            "{} is Implemented but carries a reason — a gap cannot live inside an \
             Implemented row; reclassify it as Partial",
            entry.id,
        );
    }
}

#[test]
fn test_partial_and_not_owned_carry_a_substantive_reason() {
    let open = unimplemented();
    assert!(!open.is_empty(), "the ledger must report open items");
    for entry in &open {
        let reason = entry
            .reason
            .unwrap_or_else(|| panic!("{} is {:?} with no reason at all", entry.id, entry.status));
        let trimmed = reason.trim();
        assert!(
            trimmed.chars().count() >= 80,
            "{}'s reason is {} chars — too short to be a finding: {reason}",
            entry.id,
            trimmed.chars().count(),
        );
        for placeholder in PLACEHOLDERS {
            assert!(
                !trimmed.contains(placeholder),
                "{}'s reason leans on a placeholder ({placeholder}): {reason}",
                entry.id,
            );
        }
    }
}

#[test]
fn test_every_partial_reason_names_what_is_missing() {
    for entry in partial() {
        let reason = entry.reason.unwrap_or_default();
        assert!(
            GAP_MARKERS.iter().any(|marker| reason.contains(marker)),
            "{} is Partial but its reason names no gap (expected one of \
             {GAP_MARKERS:?}): {reason}",
            entry.id,
        );
    }
}

#[test]
fn test_not_owned_entries_name_d2_as_the_reason_they_are_out_of_scope() {
    for entry in not_owned() {
        let reason = entry.reason.unwrap_or_default();
        assert!(
            reason.contains("D-2"),
            "{} is NotOwned but its reason does not cite the decision that excludes \
             it: {reason}",
            entry.id,
        );
    }
}

#[test]
fn test_the_three_statuses_partition_the_manifest() {
    let all = manifest().len();
    let done = implemented().len();
    let part = partial().len();
    let out = not_owned().len();
    assert_eq!(
        done + part + out,
        all,
        "implemented({done}) + partial({part}) + not_owned({out}) != manifest({all})",
    );

    let mut ids = ids_of(&implemented());
    ids.extend(ids_of(&partial()));
    ids.extend(ids_of(&not_owned()));
    let unique: std::collections::BTreeSet<&str> = ids.iter().copied().collect();
    assert_eq!(
        unique.len(),
        all,
        "the three status buckets overlap or drop entries: {} ids, {} unique",
        ids.len(),
        unique.len(),
    );
}

#[test]
fn test_unimplemented_is_exactly_partial_plus_not_owned() {
    let open: Vec<&str> = ids_of(&unimplemented());
    let mut expected = ids_of(&partial());
    expected.extend(ids_of(&not_owned()));
    expected.sort_unstable();
    let mut open = open;
    open.sort_unstable();
    assert_eq!(open, expected);
}

#[test]
fn test_status_of_agrees_with_the_manifest() {
    for entry in manifest() {
        assert_eq!(status_of(entry.id), Some(entry.status), "{}", entry.id);
    }
    assert_eq!(status_of("UIF-NOPE-000"), None);
    assert_eq!(status_of(""), None);
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

#[test]
fn test_render_is_deterministic() {
    let first = render();
    let second = render();
    assert_eq!(first, second, "the rendered ledger is not byte-stable");
    assert_eq!(
        first,
        render_table(&manifest()),
        "render() and render_table(manifest()) disagree"
    );
}

#[test]
fn test_rendered_table_lists_every_entry_with_its_status() {
    let table = render();
    for entry in manifest() {
        assert!(
            table.contains(entry.id),
            "{} is missing from the rendered table",
            entry.id
        );
        assert!(
            table.contains(entry.status.label()),
            "{}'s status label is missing from the rendered table",
            entry.id
        );
    }
    assert!(table.contains(&format!("entries: {}", EXPECTED_SPEC_ID_COUNT)));
}

// ---------------------------------------------------------------------------
// Id extraction
// ---------------------------------------------------------------------------

#[test]
fn test_extract_accepts_numbered_ids_and_rejects_family_wildcards() {
    let text = "heads: `UIF-TOK-*` and `UIF-ZORD-*`; ids: `UIF-TOK-001`, \
               `UIF-CMENU-003`, `UIF-ATOM-002c`, UIF-FAKE-999; noise: UIF-, \
               UIF-TOK-, uif-tok-001, UIF-TOK-001X, UIF-ATOM-002ab, preUIF-TOK-002";
    let ids = extract_spec_ids(text);
    assert_eq!(
        ids,
        vec![
            "UIF-TOK-001".to_string(),
            "UIF-CMENU-003".to_string(),
            "UIF-ATOM-002c".to_string(),
            "UIF-FAKE-999".to_string(),
        ],
        "wildcards, bare prefixes, lowercase, trailing junk and prefixed runs must all \
         be rejected, while a single lowercase letter suffix is a real id",
    );
}

#[test]
fn test_a_letter_suffix_extends_an_id_and_is_its_own_requirement() {
    // The whole point of the 2026-10-22 extractor fix: `002c` is a requirement
    // in its own right, distinct from `002`, and both must come out of one run.
    let text = "`UIF-ATOM-002` is the matrix; `UIF-ATOM-002c` is one row of it.";
    let ids = extract_spec_ids(text);
    assert_eq!(
        ids,
        vec!["UIF-ATOM-002".to_string(), "UIF-ATOM-002c".to_string()],
        "a suffixed id must extract alongside its parent, never be swallowed by it",
    );
    assert_ne!(ids[0], ids[1], "the suffix must not be normalised away",);
}

#[test]
fn test_only_a_single_trailing_lowercase_letter_is_a_suffix() {
    // Uppercase trailing junk and multi-letter runs are still noise, exactly as
    // before the fix — the suffix rule is narrow on purpose.
    for rejected in [
        "UIF-TOK-001X",
        "UIF-TOK-001XY",
        "UIF-ATOM-002ab",
        "UIF-ATOM-002cd",
    ] {
        let ids = extract_spec_ids(&format!("noise: {rejected} tail"));
        assert!(
            ids.is_empty(),
            "{rejected} must not extract as a requirement id, got {ids:?}",
        );
    }
    for accepted in ["UIF-ATOM-002a", "UIF-ATOM-002e", "UIF-COMP-007z"] {
        let ids = extract_spec_ids(&format!("real: `{accepted}` tail"));
        assert_eq!(
            ids,
            vec![accepted.to_string()],
            "{accepted} must extract as a requirement id",
        );
    }
}

#[test]
fn test_parent_id_and_is_suffixed_id_agree() {
    assert_eq!(parent_id("UIF-ATOM-002c"), Some("UIF-ATOM-002"));
    assert_eq!(parent_id("UIF-ATOM-000b"), Some("UIF-ATOM-000"));
    assert_eq!(parent_id("UIF-ATOM-002"), None);
    assert_eq!(parent_id("UIF-COMP-008"), None);
    assert!(is_suffixed_id("UIF-ATOM-002c"));
    assert!(is_suffixed_id("UIF-ATOM-000b"));
    assert!(!is_suffixed_id("UIF-ATOM-002"));
    assert!(!is_suffixed_id("UIF-TOK-001X"));
}

#[test]
fn test_a_suffixed_id_in_the_spec_must_be_classified_on_its_own() {
    // Mirrors the UIF-FAKE-999 pattern: an unclassified suffixed id has to fire
    // the audit by name, and must not be excused by its parent being classified.
    let mut ids = extract_spec_ids(&spec_text());
    assert!(
        !ids.iter().any(|id| id == "UIF-ATOM-002z"),
        "UIF-ATOM-002z must not already be a real requirement",
    );
    ids.push("UIF-ATOM-002z".to_string());
    let report = audit(&ids);
    assert_eq!(
        report.missing_in_manifest,
        vec!["UIF-ATOM-002z".to_string()],
        "a suffixed id with no entry of its own must be reported missing",
    );
    assert!(
        report.describe().contains("UIF-ATOM-002z"),
        "the report must name the suffixed id it could not classify",
    );
    assert_eq!(status_of("UIF-ATOM-002z"), None);
}

#[test]
fn test_no_suffixed_requirement_collapses_onto_its_parent_entry() {
    // The regression this guards: the extractor used to be blind to suffixes, so
    // `UIF-ATOM-002` was reported `Implemented` while `002c`, `002d` and `002e`
    // went unbuilt and unclassified. Every suffixed id the spec carries must have
    // its own entry, distinct from whatever its parent says.
    let spec_ids = extract_spec_ids(&spec_text());
    let suffixed: Vec<&str> = spec_ids
        .iter()
        .map(String::as_str)
        .filter(|id| is_suffixed_id(id))
        .collect();
    assert!(
        suffixed.len() >= 10,
        "expected at least ten letter-suffixed requirements in the spec, got \
         {suffixed:?}",
    );

    for id in suffixed {
        let parent = parent_id(id).expect("a suffixed id has a parent");
        let own = status_of(id).unwrap_or_else(|| panic!("{id} is unclassified"));
        let parent_status = status_of(parent);

        assert_ne!(
            id, parent,
            "a suffixed id must never resolve to its parent's id",
        );
        assert!(
            !parent.eq(id),
            "{id} must not be its own parent — the suffix has been normalised away",
        );
        // A parent may be Implemented only if the child independently is too;
        // anything else means the child's gap is being hidden behind it.
        if let Some(Status::Implemented) = parent_status {
            assert_eq!(
                own,
                Status::Implemented,
                "{parent} is `Implemented` while its own sub-requirement {id} is \
                 {own:?} — the parent is covering for unbuilt work",
            );
        }
    }
}

#[test]
fn test_base_and_suffixed_ids_are_two_separate_ledger_entries() {
    // `UIF-ATOM-002` and `UIF-ATOM-002c` must both appear in the manifest, as
    // two entries, never as one.
    let ids = ids_of(SPEC_COVERAGE);
    for (base, child) in [
        ("UIF-ATOM-000", "UIF-ATOM-000b"),
        ("UIF-ATOM-002", "UIF-ATOM-002c"),
    ] {
        assert!(ids.contains(&base), "{base} must be in the manifest",);
        assert!(
            ids.contains(&child),
            "{child} must be in the manifest in its own right",
        );
        assert_eq!(
            ids.iter().filter(|id| **id == child).count(),
            1,
            "{child} must appear exactly once",
        );
    }
}

#[test]
fn test_extract_deduplicates_across_a_repeated_id() {
    let text = "`UIF-MOD-002` here and `UIF-MOD-002` again";
    assert_eq!(extract_spec_ids(text), vec!["UIF-MOD-002".to_string()]);
}

#[test]
fn test_the_real_spec_carries_the_expected_number_of_ids() {
    let ids = extract_spec_ids(&spec_text());
    assert_eq!(
        ids.len(),
        EXPECTED_SPEC_ID_COUNT,
        "the spec no longer carries {EXPECTED_SPEC_ID_COUNT} numbered UIF-* ids; \
         extracted {} ids",
        ids.len(),
    );
}

#[test]
fn test_family_wildcards_in_the_spec_are_not_counted_as_requirements() {
    let text = spec_text();
    let ids = extract_spec_ids(&text);
    assert!(
        text.contains("UIF-ZORD-*"),
        "the spec is expected to head §5 with the UIF-ZORD-* family wildcard",
    );
    assert!(
        !ids.iter().any(|id| id.starts_with("UIF-ZORD-")),
        "§5 numbers no requirement, so no UIF-ZORD-nnn id may be counted: {ids:?}",
    );
}

// ---------------------------------------------------------------------------
// The audit itself
// ---------------------------------------------------------------------------

#[test]
fn test_the_ledger_is_clean_against_the_real_spec() {
    let ids = extract_spec_ids(&spec_text());
    let report = audit(&ids);
    assert!(
        report.is_clean(),
        "spec/ledger drift:\n{}",
        report.describe(),
    );
}

#[test]
fn test_audit_fires_on_an_unclassified_id() {
    // The real read, the real comparison, one injected requirement. This is
    // the guard observed firing: an ID in the spec that nothing classifies
    // must come back as missing, and the report must name it.
    let mut ids = extract_spec_ids(&spec_text());
    let before = ids.len();
    ids.push("UIF-FAKE-999".to_string());

    let report = audit(&ids);
    assert!(!report.is_clean(), "the injected id was not caught");
    assert_eq!(
        report.missing_in_manifest,
        vec!["UIF-FAKE-999".to_string()],
        "the injected id must be the only unclassified requirement",
    );
    assert_eq!(report.spec_total, before + 1);
    assert_eq!(report.stale_in_manifest, Vec::<String>::new());
    assert!(
        report.describe().contains("UIF-FAKE-999"),
        "the report must name the unclassified id",
    );
    assert_eq!(status_of("UIF-FAKE-999"), None);
}

#[test]
fn test_audit_fires_on_a_stale_ledger_entry() {
    // Drop one real requirement from the spec side: the ledger entry that
    // no longer corresponds to anything must surface as stale rather than
    // silently shrinking the ledger.
    let ids: Vec<String> = extract_spec_ids(&spec_text())
        .into_iter()
        .filter(|id| id != "UIF-TOK-006")
        .collect();

    let report = audit(&ids);
    assert!(!report.is_clean());
    assert_eq!(report.stale_in_manifest, vec!["UIF-TOK-006".to_string()]);
    assert!(report.missing_in_manifest.is_empty());
    assert_ne!(report.spec_total, report.expected_total);
}

#[test]
fn test_audit_is_clean_against_the_ledger_in_both_directions() {
    // Symmetry: feeding the audit the ledger's own ids must produce a clean
    // report only because the two sets are genuinely equal, not because
    // one direction was never checked.
    let as_spec: Vec<String> = manifest()
        .iter()
        .map(|entry| entry.id.to_string())
        .collect();
    let report = audit(&as_spec);
    assert!(report.is_clean(), "drift: {}", report.describe());

    let mut shuffled = as_spec.clone();
    shuffled.reverse();
    assert_eq!(audit(&shuffled), report, "audit must not depend on order");
}
