//! The layout gate: nine deterministic assertions over the rendered demo.
//!
//! Every assertion here is a claim that a *human would otherwise have to
//! eyeball*. Each one is machine-checkable and byte-stable, so a layout
//! regression fails a test rather than shipping a bad screenshot.
//!
//! Goldens live in `tests/golden/` and are **compare-only by default**. A
//! missing golden is a hard failure with instructions, never a silent pass
//! — `iced_test`'s own `Snapshot::matches_image`/`matches_hash` create the
//! reference file and return `true` when it is absent, which is exactly
//! the failure mode where a suite looks green while proving nothing. The
//! only way to write a golden is the explicit `HOARFROST_DEMO_GOLDEN=1`
//! opt-in, and the resulting diff is meant to be reviewed.
//!
//! These tests live inside the crate rather than in `tests/` because the
//! demo is a binary crate: an integration test cannot reach `main.rs`'s
//! modules, so the probe and its assertions have to share the binary's
//! test build. See `README.md` KNOWN GAPS.
//!
//! Unit tests live in the sibling `layout_gate_tests.rs` sidecar.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use iced::widget::scrollable;
use iced::{Element, Length, Rectangle, Size};

use crate::app::{DemoApp, Message};
use crate::layout_probe::{Entry, Probe};
use crate::pane_atoms::{self, AtomsState};
use crate::pane_composites::{self, CompositesState};

/// Setting this to `1` rewrites the goldens instead of comparing them.
const GOLDEN_ENV: &str = "HOARFROST_DEMO_GOLDEN";
/// Where the goldens live, relative to the crate root.
const GOLDEN_DIR: &str = "tests/golden";

/// The demo window's reference viewport.
const ROOT: Size = Size::new(1400.0, 900.0);
/// A single pane at the reference height.
const PANE: Size = Size::new(600.0, 900.0);

/// Widths the responsive sweep runs at.
const SWEEP_WIDTHS: [f32; 4] = [900.0, 1200.0, 1600.0, 2000.0];
/// Heights the responsive sweep runs at.
const SWEEP_HEIGHTS: [f32; 3] = [600.0, 900.0, 1200.0];

/// Slack allowed when comparing floats read out of a layout pass.
const EPSILON: f32 = 0.01;
/// Anything narrower or shorter than this is treated as collapsed.
///
/// Eight pixels is well below the smallest thing the demo legitimately
/// draws (a 24px checkbox row) and well above zero, so it catches a
/// widget that collapsed without also catching a deliberate hairline.
const MIN_WIDGET_EDGE: f32 = 8.0;

/// Every widget id the layout tests are allowed to address, sorted.
///
/// The registry is checked **both ways**: an id in this list that the
/// probe cannot find is dead weight, and an id the probe finds that is not
/// in this list is an unregistered widget that no test is watching.
const REGISTRY: [&str; 158] = [
    "demo.atoms.button.Danger.busy",
    "demo.atoms.button.Danger.live",
    "demo.atoms.button.Ghost.busy",
    "demo.atoms.button.Ghost.live",
    "demo.atoms.button.Primary.busy",
    "demo.atoms.button.Primary.live",
    "demo.atoms.button.Secondary.busy",
    "demo.atoms.button.Secondary.live",
    "demo.atoms.buttons.danger.demo-label",
    "demo.atoms.buttons.ghost.demo-label",
    "demo.atoms.buttons.primary.demo-label",
    "demo.atoms.buttons.secondary.demo-label",
    "demo.atoms.checkbox.0",
    "demo.atoms.checkbox.0.label",
    "demo.atoms.checkbox.1",
    "demo.atoms.checkbox.1.label",
    "demo.atoms.checkbox.2",
    "demo.atoms.checkbox.2.label",
    "demo.atoms.checkbox.3",
    "demo.atoms.checkbox.3.label",
    "demo.atoms.link.external",
    "demo.atoms.link.internal",
    "demo.atoms.password.disabled",
    "demo.atoms.password.live",
    "demo.atoms.radio",
    "demo.atoms.radio.label-0",
    "demo.atoms.radio.label-1",
    "demo.atoms.radio.label-2",
    "demo.atoms.range.live.lower",
    "demo.atoms.range.live.upper",
    "demo.atoms.segmented",
    "demo.atoms.split.control",
    "demo.atoms.switch.busy",
    "demo.atoms.switch.busy.label",
    "demo.atoms.switch.disabled",
    "demo.atoms.switch.disabled.label",
    "demo.atoms.switch.live",
    "demo.atoms.switch.live.label",
    "demo.atoms.text.controlled",
    "demo.atoms.text.disabled",
    "demo.atoms.text.invalid",
    "demo.atoms.text.live",
    "demo.atoms.text.readonly",
    "demo.chrome.close",
    "demo.chrome.status_bar",
    "demo.composites.form.valid",
    "demo.composites.form.valid.label",
    "demo.composites.menu.m-dark",
    "demo.composites.menu.m-dark.label",
    "demo.composites.menu.m-light",
    "demo.composites.menu.m-light.label",
    "demo.composites.menu.m-new",
    "demo.composites.menu.m-new.label",
    "demo.composites.menu.m-save",
    "demo.composites.menu.m-save.label",
    "demo.composites.menu.m-wrap",
    "demo.composites.menu.m-wrap.label",
    "demo.composites.scroll",
    "demo.composites.setting-row.0.row",
    "demo.composites.setting-row.1.row",
    "demo.composites.setting-row.2.row",
    "demo.composites.setting.0",
    "demo.composites.setting.0.label",
    "demo.composites.setting.1",
    "demo.composites.setting.1.label",
    "demo.composites.setting.2",
    "demo.composites.setting.2.label",
    "demo.pane.atoms.scroll",
    "demo.section.avatar",
    "demo.section.avatar.caption",
    "demo.section.badge-resolver-uif-atom-008",
    "demo.section.badge-resolver-uif-atom-008.caption",
    "demo.section.breadcrumb-resolver-uif-comp-004",
    "demo.section.breadcrumb-resolver-uif-comp-004.caption",
    "demo.section.buttons",
    "demo.section.buttons.caption",
    "demo.section.card-resolver-uif-comp-005",
    "demo.section.card-resolver-uif-comp-005.caption",
    "demo.section.checkbox",
    "demo.section.checkbox.caption",
    "demo.section.chip-resolver-uif-atom-006",
    "demo.section.chip-resolver-uif-atom-006.caption",
    "demo.section.code-block-uif-comp-015",
    "demo.section.code-block-uif-comp-015.caption",
    "demo.section.colour-picker",
    "demo.section.colour-picker.caption",
    "demo.section.combo-box",
    "demo.section.combo-box.caption",
    "demo.section.command-palette-uif-comp-010",
    "demo.section.command-palette-uif-comp-010.caption",
    "demo.section.date-picker",
    "demo.section.date-picker.caption",
    "demo.section.diff-viewer-uif-comp-015",
    "demo.section.diff-viewer-uif-comp-015.caption",
    "demo.section.empty-states-uif-comp-014",
    "demo.section.empty-states-uif-comp-014.caption",
    "demo.section.form-field-uif-comp-016",
    "demo.section.form-field-uif-comp-016.caption",
    "demo.section.grid-view-uif-comp-013",
    "demo.section.grid-view-uif-comp-013.caption",
    "demo.section.hyperlink",
    "demo.section.hyperlink.caption",
    "demo.section.keybind-recorder-uif-comp-015",
    "demo.section.keybind-recorder-uif-comp-015.caption",
    "demo.section.list-tile-resolver-uif-comp-006",
    "demo.section.list-tile-resolver-uif-comp-006.caption",
    "demo.section.menu-rows-uif-fmenu-uif-cmenu",
    "demo.section.menu-rows-uif-fmenu-uif-cmenu.caption",
    "demo.section.navigation-rail-uif-comp-009",
    "demo.section.navigation-rail-uif-comp-009.caption",
    "demo.section.number-box-resolver-uif-atom-011",
    "demo.section.number-box-resolver-uif-atom-011.caption",
    "demo.section.overflow-toolbar-resolver-uif-comp-008",
    "demo.section.overflow-toolbar-resolver-uif-comp-008.caption",
    "demo.section.pagination-bar-uif-comp-012",
    "demo.section.pagination-bar-uif-comp-012.caption",
    "demo.section.password",
    "demo.section.password.caption",
    "demo.section.pips-pager-uif-comp-013",
    "demo.section.pips-pager-uif-comp-013.caption",
    "demo.section.progress-resolver-uif-atom-009",
    "demo.section.progress-resolver-uif-atom-009.caption",
    "demo.section.radio-group",
    "demo.section.radio-group.caption",
    "demo.section.range-slider",
    "demo.section.range-slider.caption",
    "demo.section.rating",
    "demo.section.rating.caption",
    "demo.section.search-box",
    "demo.section.search-box.caption",
    "demo.section.segmented",
    "demo.section.segmented.caption",
    "demo.section.settings-rows-uif-comp-017",
    "demo.section.settings-rows-uif-comp-017.caption",
    "demo.section.side-sheet-uif-comp-012",
    "demo.section.side-sheet-uif-comp-012.caption",
    "demo.section.skeleton-uif-comp-015",
    "demo.section.skeleton-uif-comp-015.caption",
    "demo.section.slider-resolver-uif-atom-010",
    "demo.section.slider-resolver-uif-atom-010.caption",
    "demo.section.split-button",
    "demo.section.split-button.caption",
    "demo.section.stepper-wizard-footer-uif-comp-011",
    "demo.section.stepper-wizard-footer-uif-comp-011.caption",
    "demo.section.tab-strip-uif-tab-003",
    "demo.section.tab-strip-uif-tab-003.caption",
    "demo.section.table-resolver-uif-comp-011",
    "demo.section.table-resolver-uif-comp-011.caption",
    "demo.section.text-input",
    "demo.section.text-input.caption",
    "demo.section.time-picker",
    "demo.section.time-picker.caption",
    "demo.section.toast-notification-resolver-uif-comp-009",
    "demo.section.toast-notification-resolver-uif-comp-009.caption",
    "demo.section.toggle-switch",
    "demo.section.toggle-switch.caption",
    "demo.section.tooltip",
    "demo.section.tooltip.caption",
];

/// Ids the demo declares that the selector deliberately cannot see.
///
/// These are asserted **absent** rather than ignored. If a future hoarfrost
/// change makes one of them appear, this test fails and the id moves into
/// [`REGISTRY`] — the list cannot quietly drift in either direction.
const UNADDRESSABLE: [(&str, &str); 4] = [
    (
        "demo.atoms.button.Primary.disabled",
        "a disabled hoarfrost `Button` registers no focusable candidate, so the \
         operation tree never carries its id (iced's own disabled handling, not a \
         hoarfrost defect)",
    ),
    (
        "demo.atoms.link.disabled",
        "same for a disabled `Hyperlink`",
    ),
    (
        "demo.atoms.range.disabled",
        "same for a disabled `RangeSlider`; the live one registers its two thumbs as \
         `<id>.lower` / `<id>.upper` rather than under the bare id",
    ),
    (
        "demo.atoms.split",
        "the `SplitDivider` is not focusable and exposes no candidate kind, so only \
         `demo.atoms.split.control` (the button inside it) is addressable",
    ),
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Path to a golden file inside the crate.
fn golden_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(GOLDEN_DIR)
        .join(name)
}

/// Whether the goldens should be rewritten rather than compared.
fn regen_requested() -> bool {
    std::env::var(GOLDEN_ENV).is_ok_and(|value| value == "1")
}

/// Compares `actual` against the golden `name`, or rewrites it on request.
///
/// A missing golden panics. That is deliberate: `iced_test`'s snapshot
/// helpers would happily create the file and report success, turning a
/// never-reviewed reference into a permanent green.
fn check_golden(name: &str, actual: &str) {
    let path = golden_path(name);

    if regen_requested() {
        let dir = path
            .parent()
            .expect("a golden path always has a parent directory");
        std::fs::create_dir_all(dir).expect("the golden directory must be creatable");
        std::fs::write(&path, actual).expect("the golden must be writable");
        return;
    }

    match std::fs::read_to_string(&path) {
        Ok(expected) => assert_eq!(
            expected,
            actual,
            "golden `{}` differs. If the change is intended, re-run with \
             `$env:HOARFROST_DEMO_GOLDEN='1'` and review the diff.",
            path.display()
        ),
        Err(error) => panic!(
            "golden `{}` is missing ({error}). A missing golden is a FAIL, never \
             an auto-pass. Write it deliberately with \
             `$env:HOARFROST_DEMO_GOLDEN='1'; cargo nextest run {name}` and \
             review what it captured.",
            path.display()
        ),
    }
}

/// Captures the whole demo root at `size`.
fn capture_root(size: Size) -> Probe {
    let app = DemoApp::boot().0;
    Probe::capture(size, app.view())
}

/// Captures one pane on its own, wrapped in a scrollable the way the real
/// pane wraps it.
///
/// The state has to outlive the capture, so the pane is built and probed
/// inside one function rather than handed back as a tuple.
fn capture_pane(
    sections: iced::widget::Column<'_, Message>,
    scroll_id: &'static str,
    size: Size,
) -> Probe {
    let root: Element<'_, Message> = scrollable(sections)
        .id(iced::widget::Id::new(scroll_id))
        .width(Length::Fill)
        .height(Length::Fill)
        .into();
    Probe::capture(size, root)
}

/// Captures the atoms pane body at `size`.
fn capture_atoms_pane(size: Size) -> Probe {
    let state = AtomsState::new();
    let body = pane_atoms::atoms_pane(&state);
    capture_pane(body.sections, pane_atoms::PANE_SCROLL_ID, size)
}

/// Captures the composites pane body at `size`.
fn capture_composites_pane(size: Size) -> Probe {
    let state = CompositesState::new();
    let body = pane_composites::composites_pane(&state);
    capture_pane(body.sections, pane_composites::PANE_SCROLL_ID, size)
}

/// True when two rectangles agree to within [`EPSILON`].
fn same_rect(a: Rectangle, b: Rectangle) -> bool {
    (a.x - b.x).abs() <= EPSILON
        && (a.y - b.y).abs() <= EPSILON
        && (a.width - b.width).abs() <= EPSILON
        && (a.height - b.height).abs() <= EPSILON
}

/// Asserts no named widget leaves the viewport sideways.
///
/// Vertical overflow is what the scrollable exists for; horizontal
/// overflow is a layout bug that shows up as a clipped control or a
/// stray horizontal scrollbar.
fn assert_inside_horizontally(entries: &[Entry], viewport: Size) {
    for entry in entries {
        assert!(
            entry.bounds.x >= -EPSILON,
            "escapes the left edge at {:.0}x{:.0}: {}",
            viewport.width,
            viewport.height,
            entry.line()
        );
        assert!(
            entry.bounds.x + entry.bounds.width <= viewport.width + EPSILON,
            "escapes the right edge at {:.0}x{:.0}: {}",
            viewport.width,
            viewport.height,
            entry.line()
        );
    }
}

/// Asserts every clip lands exactly on a scrollable's edge.
///
/// A widget inside a `scrollable` is legitimately cut off where that
/// scrollable ends — that is the scroll fold, not a layout bug. What is
/// *not* legitimate is being cut anywhere else, being cut sideways (no
/// pane scrolls horizontally), or a `visible` rect larger than the
/// widget's own bounds.
fn assert_clipped_only_at_a_scroll_boundary(entries: &[Entry], viewport: Size) {
    let folds: Vec<f32> = entries
        .iter()
        .filter(|entry| entry.kind == "scrollable")
        .map(|entry| entry.bounds.y + entry.bounds.height)
        .collect();

    for entry in entries {
        let Some(visible) = entry.visible else {
            continue;
        };

        assert!(
            visible.width <= entry.bounds.width + EPSILON
                && visible.height <= entry.bounds.height + EPSILON,
            "visible bounds exceed the widget's own bounds at {:.0}x{:.0}: {}",
            viewport.width,
            viewport.height,
            entry.line()
        );
        assert!(
            (visible.x - entry.bounds.x).abs() <= EPSILON
                && (visible.width - entry.bounds.width).abs() <= EPSILON,
            "clipped sideways at {:.0}x{:.0} (no pane scrolls horizontally): {}",
            viewport.width,
            viewport.height,
            entry.line()
        );

        if same_rect(visible, entry.bounds) {
            continue;
        }

        let clipped_bottom = visible.y + visible.height;
        let at_a_fold = folds
            .iter()
            .any(|fold| (clipped_bottom - fold).abs() <= EPSILON);
        assert!(
            at_a_fold,
            "clipped somewhere that is not a scroll fold at {:.0}x{:.0} \
             (fold candidates: {folds:?}): {}",
            viewport.width,
            viewport.height,
            entry.line()
        );
    }
}

/// The entry with `id`, or a panic naming the viewport it was looked for in.
fn need<'p>(probe: &'p Probe, id: &str, viewport: Size) -> &'p Entry {
    probe.find(id).unwrap_or_else(|| {
        panic!(
            "`{id}` is missing at {:.0}x{:.0}",
            viewport.width, viewport.height
        )
    })
}

// ---------------------------------------------------------------------------
// 1. Determinism
// ---------------------------------------------------------------------------

#[test]
fn the_same_viewport_produces_the_same_manifest_twice() {
    let app = DemoApp::boot().0;
    let first = Probe::capture(ROOT, app.view()).manifest();
    let second = Probe::capture(ROOT, app.view()).manifest();

    assert_eq!(
        first, second,
        "two captures of the same view at the same viewport must be byte-identical; \
         anything else means the manifest depends on run-order, not layout"
    );
}

// ---------------------------------------------------------------------------
// 2. Goldens
// ---------------------------------------------------------------------------

#[test]
fn the_root_manifest_matches_its_golden() {
    check_golden("root-1400x900.txt", &capture_root(ROOT).manifest());
}

#[test]
fn the_atoms_pane_manifest_matches_its_golden() {
    check_golden(
        "atoms-pane-600x900.txt",
        &capture_atoms_pane(PANE).manifest(),
    );
}

#[test]
fn the_composites_pane_manifest_matches_its_golden() {
    check_golden(
        "composites-pane-600x900.txt",
        &capture_composites_pane(PANE).manifest(),
    );
}

// ---------------------------------------------------------------------------
// 3. Nothing collapsed
// ---------------------------------------------------------------------------

#[test]
fn no_named_widget_is_collapsed() {
    let probe = capture_root(ROOT);
    assert!(
        !probe.entries().is_empty(),
        "the probe found no named widgets at all, which would make this assertion vacuous"
    );

    for entry in probe.entries() {
        assert!(
            entry.bounds.width >= MIN_WIDGET_EDGE && entry.bounds.height >= MIN_WIDGET_EDGE,
            "collapsed widget: {}",
            entry.line()
        );
    }
}

#[test]
fn no_widget_has_infinite_geometry() {
    let probe = capture_root(ROOT);
    let infinite: Vec<String> = probe
        .infinite_entries()
        .iter()
        .map(|entry| entry.line())
        .collect();

    assert!(
        infinite.is_empty(),
        "non-finite layout bounds poison every following sibling's y: {infinite:?}"
    );
}

// ---------------------------------------------------------------------------
// 4. Nothing escapes the viewport
// ---------------------------------------------------------------------------

#[test]
fn nothing_escapes_the_viewport_horizontally() {
    let probe = capture_root(ROOT);
    assert_inside_horizontally(probe.entries(), ROOT);
}

// ---------------------------------------------------------------------------
// 5. Nothing clipped
// ---------------------------------------------------------------------------

#[test]
fn nothing_is_clipped_except_at_a_scroll_fold() {
    let probe = capture_root(ROOT);
    assert_clipped_only_at_a_scroll_boundary(probe.entries(), ROOT);
}

// ---------------------------------------------------------------------------
// 6. Band ordering
// ---------------------------------------------------------------------------

#[test]
fn the_three_bands_are_ordered_top_to_bottom() {
    let probe = capture_root(ROOT);
    let close = need(&probe, "demo.chrome.close", ROOT);
    let atoms = need(&probe, "demo.pane.atoms.scroll", ROOT);
    let status = need(&probe, "demo.chrome.status_bar", ROOT);

    assert!(
        close.bounds.y + close.bounds.height <= atoms.bounds.y + EPSILON,
        "the navbar must sit above the panes: close {} / atoms {}",
        close.line(),
        atoms.line()
    );
    assert!(
        atoms.bounds.y + atoms.bounds.height <= status.bounds.y + EPSILON,
        "the panes must sit above the status bar: atoms {} / status {}",
        atoms.line(),
        status.line()
    );
    assert!(
        status.bounds.y + status.bounds.height <= ROOT.height + EPSILON,
        "the status bar must fit inside the window: {}",
        status.line()
    );
}

#[test]
fn the_two_panes_are_side_by_side() {
    let probe = capture_root(ROOT);
    let atoms = need(&probe, "demo.pane.atoms.scroll", ROOT);
    let composites = need(&probe, "demo.composites.scroll", ROOT);

    assert!(
        atoms.bounds.x < composites.bounds.x,
        "the atoms pane must be left of the composites pane: atoms {} / composites {}",
        atoms.line(),
        composites.line()
    );
    assert!(
        atoms.bounds.x + atoms.bounds.width <= composites.bounds.x + EPSILON,
        "the panes must not overlap: atoms {} / composites {}",
        atoms.line(),
        composites.line()
    );
}

// ---------------------------------------------------------------------------
// 7. Registry completeness
// ---------------------------------------------------------------------------

#[test]
fn every_addressable_widget_is_registered_and_every_registered_widget_is_found() {
    let probe = capture_root(ROOT);
    let found: BTreeSet<&str> = probe.entries().iter().map(|e| e.id.as_str()).collect();
    let declared: BTreeSet<&str> = REGISTRY.into_iter().collect();

    let dead: Vec<&&str> = declared.iter().filter(|id| !found.contains(*id)).collect();
    let orphans: Vec<&&str> = found.iter().filter(|id| !declared.contains(*id)).collect();

    assert!(
        dead.is_empty(),
        "registered ids the probe could not find: {dead:?}"
    );
    assert!(
        orphans.is_empty(),
        "widgets the probe found that no test is registered against: {orphans:?}"
    );
}

#[test]
fn the_unaddressable_ids_are_still_absent() {
    let probe = capture_root(ROOT);
    let found: BTreeSet<&str> = probe.entries().iter().map(|e| e.id.as_str()).collect();

    for (id, why) in UNADDRESSABLE {
        assert!(
            !found.contains(id),
            "`{id}` unexpectedly registered a candidate. {why}. If that changed, \
             move it into REGISTRY and delete this entry."
        );
    }
}

#[test]
fn the_registry_is_sorted_and_free_of_duplicates() {
    // The manifest is sorted by id, so an unsorted or duplicated registry
    // would make the golden diff unreadable rather than wrong.
    let set: BTreeSet<&str> = REGISTRY.into_iter().collect();
    assert_eq!(
        REGISTRY.len(),
        set.len(),
        "REGISTRY contains a duplicate id"
    );

    let mut sorted: Vec<&str> = REGISTRY.to_vec();
    sorted.sort_unstable();
    assert_eq!(
        REGISTRY.to_vec(),
        sorted,
        "REGISTRY must be kept in sorted order so a diff stays readable"
    );
}

// ---------------------------------------------------------------------------
// 8. Responsive sweep
// ---------------------------------------------------------------------------

#[test]
fn the_layout_survives_the_responsive_sweep() {
    for width in SWEEP_WIDTHS {
        for height in SWEEP_HEIGHTS {
            let viewport = Size::new(width, height);
            let probe = capture_root(viewport);

            let infinite: Vec<String> = probe
                .infinite_entries()
                .iter()
                .map(|entry| entry.line())
                .collect();
            assert!(
                infinite.is_empty(),
                "infinite geometry at {:.0}x{:.0}: {infinite:?}",
                width,
                height
            );

            let atoms = need(&probe, "demo.pane.atoms.scroll", viewport);
            let composites = need(&probe, "demo.composites.scroll", viewport);
            let status = need(&probe, "demo.chrome.status_bar", viewport);
            let close = need(&probe, "demo.chrome.close", viewport);

            assert!(
                atoms.bounds.x < composites.bounds.x,
                "panes not side by side at {:.0}x{:.0}: atoms {} / composites {}",
                width,
                height,
                atoms.line(),
                composites.line()
            );
            assert!(
                atoms.bounds.x + atoms.bounds.width <= composites.bounds.x + EPSILON,
                "panes overlap at {:.0}x{:.0}: atoms {} / composites {}",
                width,
                height,
                atoms.line(),
                composites.line()
            );
            assert!(
                (status.bounds.width - width).abs() <= EPSILON,
                "the status bar must span the window at {:.0}x{:.0}: {}",
                width,
                height,
                status.line()
            );
            assert!(
                close.bounds.x + close.bounds.width <= width + EPSILON,
                "the close button left the window at {:.0}x{:.0}: {}",
                width,
                height,
                close.line()
            );

            assert_inside_horizontally(probe.entries(), viewport);
            assert_clipped_only_at_a_scroll_boundary(probe.entries(), viewport);
        }
    }
}

// ---------------------------------------------------------------------------
// 9. No rebuild drift
// ---------------------------------------------------------------------------

#[test]
fn a_state_change_does_not_move_the_chrome_bands() {
    let mut app = DemoApp::boot().0;
    let before = Probe::capture(ROOT, app.view());

    let _ = app.update(Message::Atoms(pane_atoms::AtomsMessage::SwitchToggled(
        true,
    )));
    let after = Probe::capture(ROOT, app.view());

    for id in [
        "demo.chrome.close",
        "demo.chrome.status_bar",
        "demo.pane.atoms.scroll",
        "demo.composites.scroll",
    ] {
        assert_eq!(
            before.find(id).map(|entry| entry.line()),
            after.find(id).map(|entry| entry.line()),
            "`{id}` moved after a state change that should not have touched layout"
        );
    }
}

#[test]
fn a_layout_affecting_change_actually_moves_the_pane() {
    // The mirror image of the band test. If *nothing* ever moved, the
    // probe would be reporting a frozen tree and every assertion above
    // would be vacuously true. A committed split is the demo's one
    // message guaranteed to change geometry.
    let mut app = DemoApp::boot().0;
    let before = Probe::capture(ROOT, app.view());
    let before_pane = before
        .find("demo.composites.scroll")
        .map(|entry| entry.line())
        .expect("the composites pane must be addressable");

    let ratio = crate::data::SPLIT_RATIO_DEFAULT + 0.15;
    let _ = app.update(Message::SplitCommitted(ratio));
    let after = Probe::capture(ROOT, app.view());
    let after_pane = after
        .find("demo.composites.scroll")
        .map(|entry| entry.line())
        .expect("the composites pane must be addressable");

    assert_ne!(
        before_pane, after_pane,
        "committing a new split ratio did not move the composites pane - the \
         probe cannot see state"
    );
}

#[cfg(test)]
#[path = "layout_gate_tests.rs"]
mod tests;
