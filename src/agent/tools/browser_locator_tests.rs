use super::*;
use std::collections::HashSet;
use std::path::PathBuf;

#[test]
fn test_browser_kind_display() {
    assert_eq!(format!("{}", BrowserKind::GoogleChrome), "Google Chrome");
    assert_eq!(format!("{}", BrowserKind::Chromium), "Chromium");
    assert_eq!(format!("{}", BrowserKind::MicrosoftEdge), "Microsoft Edge");
    assert_eq!(format!("{}", BrowserKind::Brave), "Brave");
    assert_eq!(format!("{}", BrowserKind::Custom), "Custom Browser");
}

#[test]
fn test_custom_path_takes_precedence() {
    let custom = PathBuf::from("/custom/bin/my-chrome");
    let locator = SystemBrowserLocator::new(Some(custom.clone()));

    let mut existing = HashSet::new();
    existing.insert(custom.clone());

    let found = locator
        .locate_with_checker(|p| existing.contains(p))
        .unwrap();
    assert_eq!(found.kind, BrowserKind::Custom);
    assert_eq!(found.executable_path, custom);
}

#[test]
fn test_custom_path_nonexistent_falls_through() {
    let custom = PathBuf::from("/nonexistent/chrome");
    let locator = SystemBrowserLocator::new(Some(custom));

    let candidates = SystemBrowserLocator::standard_candidates();
    if let Some((first_kind, first_path)) = candidates.first() {
        let mut existing = HashSet::new();
        existing.insert(first_path.clone());

        let found = locator
            .locate_with_checker(|p| existing.contains(p))
            .unwrap();
        assert_eq!(found.kind, *first_kind);
        assert_eq!(found.executable_path, *first_path);
    }
}

#[test]
fn test_none_returned_when_no_browser_installed() {
    let locator = SystemBrowserLocator::new(None);
    let found = locator.locate_with_checker(|_| false);
    assert!(found.is_none());
}

#[test]
fn test_edge_selected_when_chrome_absent() {
    let locator = SystemBrowserLocator::new(None);
    let candidates = SystemBrowserLocator::standard_candidates();

    // Find the first Edge candidate
    let edge_candidate = candidates
        .iter()
        .find(|(kind, _)| *kind == BrowserKind::MicrosoftEdge);

    if let Some((kind, path)) = edge_candidate {
        let mut existing = HashSet::new();
        existing.insert(path.clone());

        let found = locator
            .locate_with_checker(|p| existing.contains(p))
            .unwrap();
        assert_eq!(found.kind, *kind);
        assert_eq!(found.executable_path, *path);
    }
}
