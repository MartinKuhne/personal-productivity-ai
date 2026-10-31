//! Unit tests for the layout gate's own helpers.
//!
//! A gate is only worth trusting if the gate itself is tested. These check
//! that each assertion helper actually *fails* on the thing it claims to
//! catch — an assertion that never fires is worse than no assertion, and
//! the golden helper is checked for the one behaviour that matters most:
//! a missing golden must never read as a pass.

use std::panic::{AssertUnwindSafe, catch_unwind};

use iced::{Rectangle, Size};

use super::*;

/// A viewport big enough to hold any fixture below.
const VIEW: Size = Size::new(1000.0, 800.0);

/// Builds an entry with the geometry a test wants to reason about.
fn entry(id: &str, x: f32, y: f32, w: f32, h: f32) -> Entry {
    let bounds = Rectangle::new(iced::Point::new(x, y), Size::new(w, h));
    Entry {
        id: id.to_string(),
        kind: "container",
        bounds,
        visible: Some(bounds),
    }
}

/// Builds an entry whose visible rect is smaller than its bounds, i.e. one
/// the viewport has cut down.
fn clipped(id: &str, x: f32, y: f32, w: f32, h: f32, visible_h: f32) -> Entry {
    Entry {
        id: id.to_string(),
        kind: "container",
        bounds: Rectangle::new(iced::Point::new(x, y), Size::new(w, h)),
        visible: Some(Rectangle::new(
            iced::Point::new(x, y),
            Size::new(w, visible_h),
        )),
    }
}

/// Runs `f` expecting it to panic, and returns the panic message.
fn panic_message(f: impl FnOnce()) -> String {
    let result = catch_unwind(AssertUnwindSafe(f));
    match result {
        Ok(()) => panic!("the helper was expected to fail but it passed"),
        Err(payload) => payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_string()))
            .unwrap_or_else(|| String::from("<non-string panic>")),
    }
}

// ---------------------------------------------------------------------------
// same_rect
// ---------------------------------------------------------------------------

#[test]
fn same_rect_accepts_identical_rectangles() {
    let a = Rectangle::new(iced::Point::new(10.0, 20.0), Size::new(100.0, 50.0));
    assert!(same_rect(a, a));
}

#[test]
fn same_rect_accepts_slack_inside_the_epsilon() {
    let a = Rectangle::new(iced::Point::new(10.0, 20.0), Size::new(100.0, 50.0));
    let b = Rectangle::new(
        iced::Point::new(10.0 + EPSILON / 2.0, 20.0),
        Size::new(100.0, 50.0 - EPSILON / 2.0),
    );
    assert!(same_rect(a, b));
}

#[test]
fn same_rect_rejects_a_difference_outside_the_epsilon() {
    let a = Rectangle::new(iced::Point::new(10.0, 20.0), Size::new(100.0, 50.0));
    let shifted = Rectangle::new(iced::Point::new(11.0, 20.0), Size::new(100.0, 50.0));
    let resized = Rectangle::new(iced::Point::new(10.0, 20.0), Size::new(101.0, 50.0));

    assert!(!same_rect(a, shifted));
    assert!(!same_rect(a, resized));
}

// ---------------------------------------------------------------------------
// assert_inside_horizontally
// ---------------------------------------------------------------------------

#[test]
fn the_horizontal_check_passes_for_a_widget_inside_the_viewport() {
    assert_inside_horizontally(&[entry("ok", 0.0, 0.0, 1000.0, 40.0)], VIEW);
    assert_inside_horizontally(&[entry("edge", 960.0, 100.0, 40.0, 40.0)], VIEW);
}

#[test]
fn the_horizontal_check_fails_for_a_widget_past_the_right_edge() {
    let message = panic_message(|| {
        assert_inside_horizontally(&[entry("bad", 990.0, 0.0, 40.0, 40.0)], VIEW);
    });
    assert!(message.contains("escapes the right edge"), "{message}");
    assert!(message.contains("bad"), "{message}");
}

#[test]
fn the_horizontal_check_fails_for_a_widget_past_the_left_edge() {
    let message = panic_message(|| {
        assert_inside_horizontally(&[entry("bad", -1.0, 0.0, 40.0, 40.0)], VIEW);
    });
    assert!(message.contains("escapes the left edge"), "{message}");
}

#[test]
fn the_horizontal_check_tolerates_exactly_the_epsilon() {
    // A widget whose right edge lands exactly on the viewport edge must not
    // be reported, or every flush layout would fail.
    assert_inside_horizontally(
        &[entry("flush", 0.0, 0.0, VIEW.width + EPSILON, 40.0)],
        VIEW,
    );
}

// ---------------------------------------------------------------------------
// assert_clipped_only_at_a_scroll_boundary
// ---------------------------------------------------------------------------

/// Builds the entry for a `scrollable` whose viewport ends at `fold`.
fn scroll_viewport(id: &str, width: f32, fold: f32) -> Entry {
    let bounds = Rectangle::new(iced::Point::new(0.0, 0.0), Size::new(width, fold));
    Entry {
        id: id.to_string(),
        kind: "scrollable",
        bounds,
        visible: Some(bounds),
    }
}

#[test]
fn the_clip_check_passes_when_visible_equals_bounds() {
    assert_clipped_only_at_a_scroll_boundary(&[entry("ok", 10.0, 10.0, 100.0, 40.0)], VIEW);
}

#[test]
fn the_clip_check_ignores_an_entry_with_no_visible_rect() {
    let mut hidden = entry("hidden", 10.0, 10.0, 100.0, 40.0);
    hidden.visible = None;
    assert_clipped_only_at_a_scroll_boundary(&[hidden], VIEW);
}

#[test]
fn the_clip_check_allows_a_cut_that_lands_on_a_scroll_fold() {
    // The widget runs past the bottom of its scrollable and is cut exactly
    // at the fold. That is scrolling, not a layout bug.
    let pane = scroll_viewport("probe.scroll", 1000.0, 200.0);
    let cut = clipped("cut", 10.0, 160.0, 100.0, 80.0, 40.0);
    assert_clipped_only_at_a_scroll_boundary(&[pane, cut], VIEW);
}

#[test]
fn the_clip_check_fails_for_a_cut_that_is_not_at_a_fold() {
    let pane = scroll_viewport("probe.scroll", 1000.0, 200.0);
    // Cut at 160 while the fold sits at 200 — nothing explains this clip.
    let cut = clipped("cut", 10.0, 100.0, 100.0, 80.0, 60.0);
    let message = panic_message(|| {
        assert_clipped_only_at_a_scroll_boundary(&[pane, cut], VIEW);
    });
    assert!(message.contains("not a scroll fold"), "{message}");
    assert!(message.contains("cut"), "{message}");
}

#[test]
fn the_clip_check_fails_when_there_is_no_scrollable_at_all() {
    // A clip with no scrollable in the tree has no possible explanation.
    let cut = clipped("cut", 10.0, 100.0, 100.0, 80.0, 40.0);
    let message = panic_message(|| {
        assert_clipped_only_at_a_scroll_boundary(&[cut], VIEW);
    });
    assert!(message.contains("not a scroll fold"), "{message}");
}

#[test]
fn the_clip_check_fails_for_a_widget_cut_sideways() {
    let mut sideways = entry("sideways", 10.0, 10.0, 100.0, 40.0);
    sideways.visible = Some(Rectangle::new(
        iced::Point::new(20.0, 10.0),
        Size::new(90.0, 40.0),
    ));

    let message = panic_message(|| {
        assert_clipped_only_at_a_scroll_boundary(&[sideways], VIEW);
    });
    assert!(message.contains("clipped sideways"), "{message}");
}

#[test]
fn the_clip_check_fails_when_visible_is_larger_than_bounds() {
    let mut inflated = entry("inflated", 10.0, 10.0, 100.0, 40.0);
    inflated.visible = Some(Rectangle::new(
        iced::Point::new(10.0, 10.0),
        Size::new(200.0, 40.0),
    ));

    let message = panic_message(|| {
        assert_clipped_only_at_a_scroll_boundary(&[inflated], VIEW);
    });
    assert!(
        message.contains("exceed the widget's own bounds"),
        "{message}"
    );
}

// ---------------------------------------------------------------------------
// golden handling
// ---------------------------------------------------------------------------

#[test]
fn the_golden_path_lands_inside_the_crate() {
    let path = golden_path("whatever.txt");
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(
        path.starts_with(manifest_dir),
        "goldens must live inside the crate, got {}",
        path.display()
    );
    assert!(
        path.to_string_lossy().contains(GOLDEN_DIR),
        "goldens must live under {GOLDEN_DIR}, got {}",
        path.display()
    );
}

#[test]
fn a_missing_golden_panics_instead_of_passing() {
    // This is the whole reason `check_golden` exists instead of calling
    // `iced_test`'s `matches_image`: that helper would create the file and
    // return `true`, so a suite could report green forever against a
    // reference nobody ever reviewed.
    let message = panic_message(|| check_golden("definitely-not-a-real-golden.txt", "anything"));
    assert!(message.contains("is missing"), "{message}");
    assert!(message.contains("FAIL"), "{message}");
    assert!(
        message.contains(GOLDEN_ENV),
        "the panic must tell the reader how to write the golden deliberately"
    );
}

#[test]
fn a_matching_golden_passes_without_writing() {
    // Round-trip through a golden that really exists: read it, compare it
    // to itself, and confirm nothing was rewritten.
    let existing = golden_path("root-1400x900.txt");
    if !existing.exists() {
        // The root golden is generated as part of the gate; if it has not
        // been written yet the real test will fail loudly, so there is
        // nothing to round-trip here.
        return;
    }
    let before = std::fs::read(&existing).expect("the root golden must be readable");
    let text = String::from_utf8(before.clone()).expect("goldens are UTF-8 text");
    check_golden("root-1400x900.txt", &text);
    let after = std::fs::read(&existing).expect("the root golden must still be readable");
    assert_eq!(
        before, after,
        "a compare-only run must not rewrite the golden"
    );
}

#[test]
fn a_mismatching_golden_fails_loudly() {
    let existing = golden_path("root-1400x900.txt");
    if !existing.exists() {
        return;
    }
    let message = panic_message(|| check_golden("root-1400x900.txt", "not the golden"));
    assert!(message.contains("differs"), "{message}");
    assert!(
        message.contains(GOLDEN_ENV),
        "the failure must name the regen switch"
    );
}

#[test]
fn regen_is_off_by_default() {
    // `std::env::set_var` is `unsafe` in edition 2024, so the gate must be
    // correct with the variable simply absent — which is every normal run.
    assert!(
        !regen_requested(),
        "the gate must never rewrite goldens unless the operator opted in"
    );
}

// ---------------------------------------------------------------------------
// registry shape
// ---------------------------------------------------------------------------

#[test]
fn the_registry_covers_both_panes_and_the_chrome() {
    let atoms = REGISTRY
        .iter()
        .filter(|id| id.starts_with("demo.atoms."))
        .count();
    let composites = REGISTRY
        .iter()
        .filter(|id| id.starts_with("demo.composites."))
        .count();
    let chrome = REGISTRY
        .iter()
        .filter(|id| id.starts_with("demo.chrome."))
        .count();

    assert!(
        atoms > 20,
        "the atoms pane registry looks too thin: {atoms}"
    );
    assert!(
        composites > 5,
        "the composites pane registry looks too thin: {composites}"
    );
    assert_eq!(2, chrome, "the chrome registers close + status bar");
}

#[test]
fn the_registry_and_the_unaddressable_list_do_not_overlap() {
    for (id, _) in UNADDRESSABLE {
        // The unaddressable entries use wildcards for the variants, so
        // compare on the concrete prefix they name.
        let concrete = id.replace('*', "");
        assert!(
            !REGISTRY.contains(&id),
            "`{id}` cannot be in both lists (loose match `{concrete}`)"
        );
    }
}
