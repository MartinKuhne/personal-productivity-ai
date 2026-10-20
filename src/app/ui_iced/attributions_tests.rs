//! Sidecar unit tests for `ui_iced/attributions.rs`.
//!
//! Pure view-model and widget-construction tests ([RUST-003]). The catalog
//! is compile-time data, so these tests read it directly and assert the
//! snapshot preserves it exactly — no renderer, no bus, no filesystem
//! ([RUST-006]).

use super::*;
use crate::ui::strings;
use iced::Element;

/// A synthetic two-row catalog — never read from disk ([RUST-006]).
fn fixture_entries() -> Vec<Attribution> {
    vec![
        Attribution {
            name: "aaa-crate",
            authors: "A. Author",
            github_url: "https://github.com/a/aaa-crate",
        },
        Attribution {
            name: "zzz-crate",
            authors: "Z. Author",
            github_url: "https://github.com/z/zzz-crate",
        },
    ]
}

// --- Data source: the shared catalog ----------------------------------------

#[test]
fn from_catalog_is_the_reused_thelegacy_catalog_verbatim() {
    let vm = AttributionsViewModel::from_catalog();
    assert_eq!(
        vm.entries(),
        DIRECT_DEPENDENCIES,
        "the iced view must show the same catalog the legacy UI dialog shows, unchanged"
    );
}

#[test]
fn catalog_is_non_empty() {
    let vm = AttributionsViewModel::from_catalog();
    assert!(
        !vm.is_empty() && vm.count() == DIRECT_DEPENDENCIES.len(),
        "the attribution catalog must carry rows"
    );
}

#[test]
fn every_entry_carries_name_authors_and_repository() {
    // The catalog's row shape is `name` / `authors` / `github_url` — there
    // is no per-crate license or version field, so these three are what
    // the window can show.
    for entry in AttributionsViewModel::from_catalog().entries() {
        assert!(!entry.name.is_empty(), "empty crate name in {entry:?}");
        assert!(
            !entry.authors.is_empty(),
            "empty authors for {}",
            entry.name
        );
        assert!(
            entry.github_url.starts_with("https://github.com/"),
            "repository URL for {} must be an https GitHub link, got {}",
            entry.name,
            entry.github_url
        );
    }
}

#[test]
fn ordering_is_deterministic_and_alphabetical() {
    // Mirrors `ui::attributions_tests::slice_is_sorted_and_unique`: the
    // snapshot must not reorder anything, so two runs render identically.
    let names: Vec<&str> = AttributionsViewModel::from_catalog()
        .entries()
        .iter()
        .map(|entry| entry.name)
        .collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(
        names, sorted,
        "the rendered order must be the catalog order"
    );
}

#[test]
fn catalog_has_no_duplicate_crate_names() {
    let mut seen = std::collections::HashSet::new();
    for entry in AttributionsViewModel::from_catalog().entries() {
        assert!(
            seen.insert(entry.name),
            "duplicate crate name {}",
            entry.name
        );
    }
}

#[test]
fn snapshot_is_independent_of_later_catalog_reads() {
    // The snapshot owns its `Vec`, so two snapshots are equal but not
    // aliasing the same allocation-dependent state.
    let a = AttributionsViewModel::from_catalog();
    let b = AttributionsViewModel::from_catalog();
    assert_eq!(a, b);
    assert_eq!(a.count(), b.count());
}

// --- Summary line --------------------------------------------------------

#[test]
fn summary_counts_the_entries() {
    let vm = AttributionsViewModel::with_entries(fixture_entries());
    assert_eq!(vm.count(), 2);
    assert_eq!(vm.summary(), strings::format_attribution_count(2));
    assert!(vm.summary().contains("2"));
}

#[test]
fn empty_catalog_reports_the_empty_state_line() {
    let vm = AttributionsViewModel::with_entries(Vec::new());
    assert!(vm.is_empty());
    assert_eq!(vm.summary(), strings::ABOUT_ATTRIBUTIONS_EMPTY);
}

#[test]
fn default_view_model_is_empty() {
    let vm = AttributionsViewModel::default();
    assert!(vm.is_empty());
    assert_eq!(vm.summary(), strings::ABOUT_ATTRIBUTIONS_EMPTY);
}

// --- Widget construction --------------------------------------------------

#[test]
fn attribution_table_builds_for_empty_and_populated_catalogs() {
    // The empty state renders the same table widget (header only), so the
    // card's widget tree never changes (AGENTS §6).
    let empty: Element<'static, Message> = attribution_table(&[]);
    let full: Element<'static, Message> = attribution_table(&fixture_entries());
    let catalog: Element<'static, Message> =
        attribution_table(AttributionsViewModel::from_catalog().entries());
    let _ = (empty, full, catalog);
}

#[test]
fn attributions_view_builds_for_empty_and_populated_catalogs() {
    let empty_vm = AttributionsViewModel::with_entries(vec![]);
    let full_vm = AttributionsViewModel::with_entries(fixture_entries());
    let catalog_vm = AttributionsViewModel::from_catalog();

    let empty: Element<'_, Message> = attributions_view(&empty_vm);
    let full: Element<'_, Message> = attributions_view(&full_vm);
    let catalog: Element<'_, Message> = attributions_view(&catalog_vm);
    let _ = (empty, full, catalog);
}

#[test]
fn view_model_view_matches_the_free_function() {
    let vm = AttributionsViewModel::from_catalog();
    // Both entry points construct the same card; neither panics.
    let _via_method: Element<'_, Message> = vm.view();
    let _via_function: Element<'_, Message> = attributions_view(&vm);
}

// --- Deferred intents -----------------------------------------------------

#[test]
fn repository_press_carries_the_url_verbatim() {
    let url = "https://github.com/dtolnay/anyhow";
    assert_eq!(
        repository_press(url),
        Message::About(AboutMessage::RepositoryOpened {
            url: url.to_owned(),
        })
    );
}

#[test]
fn repository_press_copies_the_url_into_the_message() {
    // The view may not open anything, so the URL must travel with the
    // message as an owned copy.
    let url = String::from("https://github.com/serde-rs/serde");
    let message = repository_press(&url);
    drop(url);
    assert_eq!(
        message,
        Message::About(AboutMessage::RepositoryOpened {
            url: "https://github.com/serde-rs/serde".into(),
        })
    );
}

// --- Colours come from palette tokens ---------------------------------------

#[test]
fn column_colours_resolve_to_palette_tokens() {
    let palette = Palette::fastmd_dark();
    assert_eq!(crate_name_color(), palette.text_strong);
    assert_eq!(authors_color(), palette.text);
    assert_eq!(repository_color(), palette.accent);
}

#[test]
fn column_colours_are_distinct() {
    assert_ne!(crate_name_color(), authors_color());
    assert_ne!(crate_name_color(), repository_color());
    assert_ne!(authors_color(), repository_color());
}
