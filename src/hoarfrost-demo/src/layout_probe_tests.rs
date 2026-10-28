//! Unit tests for the layout probe itself.
//!
//! The probe is the instrument every layout assertion reads through, so a
//! defect here silently corrupts all of them. These tests check the probe
//! against fixtures whose geometry is known by construction rather than
//! by inspection.

use iced::widget::Id;
use iced::widget::{Column, scrollable};
use iced::{Element, Rectangle, Size};

use hoarfrost::token::typography;

use super::*;
use crate::app::{DemoApp, Message};
use crate::section::{rung, section};

/// A viewport big enough that nothing in these fixtures is scrolled out.
const ROOMY: Size = Size::new(1200.0, 1000.0);

/// Builds a one-widget tree so a probe run has something named to find.
fn one_named_widget(id: &'static str) -> Element<'static, Message> {
    iced::widget::container(iced::widget::Space::new().width(120.0).height(40.0))
        .id(Id::new(id))
        .width(iced::Length::Fixed(120.0))
        .height(iced::Length::Fixed(40.0))
        .into()
}

/// Probes a single named box and returns its entry.
fn probe_one(id: &'static str) -> Entry {
    let probe = Probe::capture(ROOMY, one_named_widget(id));
    probe
        .find(id)
        .unwrap_or_else(|| panic!("`{id}` should be addressable"))
        .clone()
}

// ---------------------------------------------------------------------------
// stable_name
// ---------------------------------------------------------------------------

#[test]
fn a_named_id_is_stable() {
    assert_eq!(
        Some(String::from("demo.thing")),
        stable_name(&Id::new("demo.thing"))
    );
}

#[test]
fn an_owned_id_is_stable_too() {
    let owned = Id::from(String::from("demo.owned"));
    assert_eq!(Some(String::from("demo.owned")), stable_name(&owned));
}

#[test]
fn a_unique_id_is_rejected() {
    // `Id::unique()` draws from a process-global counter, so it changes
    // between runs and would make every golden flap.
    assert_eq!(None, stable_name(&Id::unique()));
    assert_eq!(None, stable_name(&Id::unique()));
}

#[test]
fn two_unique_ids_are_both_unstable() {
    let a = Id::unique();
    let b = Id::unique();
    assert_ne!(a, b, "the fixture assumes unique ids differ");
    assert_eq!(None, stable_name(&a));
    assert_eq!(None, stable_name(&b));
}

// ---------------------------------------------------------------------------
// capture + manifest
// ---------------------------------------------------------------------------

#[test]
fn a_named_box_is_found_at_the_geometry_it_was_given() {
    let entry = probe_one("probe.box");
    assert_eq!(120.0, entry.bounds.width);
    assert_eq!(40.0, entry.bounds.height);
    assert_eq!("container", entry.kind);
}

#[test]
fn the_viewport_is_recorded() {
    let probe = Probe::capture(ROOMY, one_named_widget("probe.box"));
    assert_eq!(ROOMY, probe.viewport());
}

#[test]
fn the_manifest_is_sorted_by_id() {
    let tree: Element<'_, Message> = Column::new()
        .push(one_named_widget("probe.zulu"))
        .push(one_named_widget("probe.alpha"))
        .push(one_named_widget("probe.mike"))
        .into();
    let probe = Probe::capture(ROOMY, tree);
    let manifest = probe.manifest();

    let lines: Vec<&str> = manifest
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect();
    assert_eq!(3, lines.len(), "{manifest}");
    assert!(lines[0].starts_with("probe.alpha"), "{lines:?}");
    assert!(lines[1].starts_with("probe.mike"), "{lines:?}");
    assert!(lines[2].starts_with("probe.zulu"), "{lines:?}");
}

#[test]
fn the_manifest_carries_a_viewport_header() {
    let manifest = Probe::capture(ROOMY, one_named_widget("probe.box")).manifest();
    assert!(
        manifest.starts_with("# viewport 1200.00 x 1000.00"),
        "{manifest}"
    );
    assert!(manifest.contains("# named 1 / anonymous 0"), "{manifest}");
}

#[test]
fn an_anonymous_candidate_is_counted_but_not_named() {
    let tree: Element<'_, Message> = Column::new()
        .push(one_named_widget("probe.named"))
        .push(
            iced::widget::container(iced::widget::Space::new().width(10.0).height(10.0))
                .width(iced::Length::Fixed(10.0))
                .height(iced::Length::Fixed(10.0)),
        )
        .into();
    let probe = Probe::capture(ROOMY, tree);

    // Exactly one named entry. The anonymous side is at least the unnamed
    // container; the surrounding `Column` may register a node of its own
    // too, and asserting an exact anonymous count here would pin iced's
    // internal traversal rather than anything the demo owns.
    assert_eq!(1, probe.entries().len(), "only the named box is listed");
    assert!(
        probe.anonymous_count() >= 1,
        "the anonymous box must be counted, got {}",
        probe.anonymous_count()
    );
    assert!(
        probe.all_entries().len() > probe.entries().len(),
        "all_entries must include the anonymous node"
    );
}

#[test]
fn anonymous_entries_get_a_synthetic_diagnostic_id() {
    let tree: Element<'_, Message> = Column::new()
        .push(
            iced::widget::container(iced::widget::Space::new().width(10.0).height(10.0))
                .width(iced::Length::Fixed(10.0))
                .height(iced::Length::Fixed(10.0)),
        )
        .into();
    let probe = Probe::capture(ROOMY, tree);
    let unnamed = probe
        .all_entries()
        .iter()
        .find(|e| e.id.starts_with('#'))
        .expect("the anonymous node must carry a synthetic id");
    assert!(unnamed.kind.len() > 1, "{unnamed:?}");
}

#[test]
fn with_prefix_selects_a_subtree_by_id() {
    let tree: Element<'_, Message> = Column::new()
        .push(one_named_widget("probe.a.one"))
        .push(one_named_widget("probe.a.two"))
        .push(one_named_widget("probe.b.one"))
        .into();
    let probe = Probe::capture(ROOMY, tree);

    assert_eq!(2, probe.with_prefix("probe.a.").len());
    assert_eq!(1, probe.with_prefix("probe.b.").len());
    assert_eq!(3, probe.with_prefix("probe.").len());
    assert_eq!(0, probe.with_prefix("nope.").len());
}

#[test]
fn the_finite_predicate_rejects_any_non_finite_component() {
    // A `Length::Fill` resolved inside an unbounded main axis yields
    // `f32::INFINITY`, which poisons every following sibling's `y`. The
    // predicate is tested directly rather than through a fixture because
    // iced's own `container` clamps `Fill` in an unbounded axis, so no
    // plain widget reliably reproduces the condition — the widgets that do
    // are hoarfrost's, and they are covered by the gate's
    // `no_widget_has_infinite_geometry` on the real demo.
    let finite = Rectangle::new(iced::Point::new(0.0, 0.0), Size::new(10.0, 10.0));
    assert!(is_finite_rect(&finite));

    for bad in [
        Rectangle::new(iced::Point::new(f32::INFINITY, 0.0), Size::new(10.0, 10.0)),
        Rectangle::new(
            iced::Point::new(0.0, f32::NEG_INFINITY),
            Size::new(10.0, 10.0),
        ),
        Rectangle::new(iced::Point::new(0.0, 0.0), Size::new(f32::INFINITY, 10.0)),
        Rectangle::new(iced::Point::new(0.0, 0.0), Size::new(10.0, f32::NAN)),
    ] {
        assert!(!is_finite_rect(&bad), "{bad:?} must not read as finite");
    }
}

#[test]
fn a_bounded_tree_reports_no_infinite_geometry() {
    let probe = Probe::capture(ROOMY, one_named_widget("probe.box"));
    assert!(probe.infinite_entries().is_empty());
}

// ---------------------------------------------------------------------------
// Entry::line
// ---------------------------------------------------------------------------

#[test]
fn a_manifest_line_has_three_pipe_separated_sections() {
    let entry = probe_one("probe.line");
    let line = entry.line();
    let parts: Vec<&str> = line.split(" | ").collect();
    assert_eq!(3, parts.len(), "{line}");
    assert_eq!("probe.line [container]", parts[0]);
    assert_eq!(4, parts[1].split(' ').count(), "{}", parts[1]);
    assert_eq!(4, parts[2].split(' ').count(), "{}", parts[2]);
}

#[test]
fn a_line_without_visible_bounds_prints_a_dash() {
    let entry = Entry {
        id: String::from("probe.hidden"),
        kind: "container",
        bounds: Rectangle::new(iced::Point::new(1.0, 2.0), Size::new(3.0, 4.0)),
        visible: None,
    };
    assert!(entry.line().ends_with("| -"), "{}", entry.line());
}

#[test]
fn coordinates_are_printed_to_the_declared_precision() {
    let entry = Entry {
        id: String::from("probe.prec"),
        kind: "container",
        bounds: Rectangle::new(iced::Point::new(1.23456, 2.0), Size::new(3.0, 4.0)),
        visible: Some(Rectangle::new(
            iced::Point::new(0.0, 0.0),
            Size::new(9.999, 8.005),
        )),
    };
    let line = entry.line();
    assert!(line.contains("1.23"), "{line}");
    assert!(!line.contains("1.234"), "{line}");
    assert!(line.contains("10.00"), "{line}");
}

// ---------------------------------------------------------------------------
// text_of
// ---------------------------------------------------------------------------

#[test]
fn text_of_collects_the_rendered_strings() {
    let tree: Element<'static, Message> = section(
        "Probe Section",
        0,
        Column::new()
            .push(rung("needle-alpha", typography::BODY, iced::Color::WHITE))
            .push(rung("needle-beta", typography::CAPTION, iced::Color::WHITE)),
    )
    .element;

    let text = text_of(tree);
    assert!(text.contains("needle-alpha"), "{text}");
    assert!(text.contains("needle-beta"), "{text}");
    assert!(text.contains("Probe Section"), "{text}");
}

#[test]
fn text_of_does_not_invent_strings_that_were_never_rendered() {
    let tree: Element<'static, Message> = section("Plain", 0, Column::new()).element;
    let text = text_of(tree);
    assert!(!text.contains("needle"), "{text}");
}

#[test]
fn text_of_sees_the_demo_chrome() {
    let app = DemoApp::boot().0;
    let text = text_of(app.view());
    assert!(
        text.contains("Close"),
        "the close button's label must be rendered text: {text}"
    );
}

// ---------------------------------------------------------------------------
// renderer
// ---------------------------------------------------------------------------

#[test]
fn the_headless_renderer_names_itself() {
    use iced::advanced::renderer::Headless as _;
    let name = headless_renderer().name();
    assert!(!name.is_empty(), "the renderer must report a name");
}

// ---------------------------------------------------------------------------
// Human review only
// ---------------------------------------------------------------------------

/// Renders the demo to a PNG in the temp dir and prints its SHA-256.
///
/// Deliberately `#[ignore]`d. A raster golden pins the renderer, the font
/// files and the hinting settings of whichever machine drew it, so it can
/// only ever be *reviewed*, never gated. The text manifest is what the
/// suite asserts on; this test exists so a human can look at a picture.
#[test]
#[ignore = "human review only: writes a PNG + SHA-256 to the temp dir"]
fn render_a_png_for_human_review() {
    let dir = std::env::temp_dir().join("hoarfrost-demo-review");
    std::fs::create_dir_all(&dir).expect("the review directory must be creatable");

    let stem = dir.join("root-1400x900");
    let png_path = stem.with_extension("png");
    let hash_path = stem.with_extension("sha256");

    // Start clean, so `matches_*` cannot silently compare against a stale
    // reference from an earlier run.
    let _ = std::fs::remove_file(&png_path);
    let _ = std::fs::remove_file(&hash_path);
    assert!(!png_path.exists(), "the review PNG must start absent");
    assert!(!hash_path.exists(), "the review hash must start absent");

    let app = DemoApp::boot().0;
    let mut simulator = iced_test::simulator(app.view());
    let snapshot = simulator
        .snapshot(&crate::theme::demo_theme())
        .expect("the demo must rasterise headless");

    // Both helpers create-on-missing and return `true`. That is precisely
    // the behaviour the gate refuses to trust, so here it is asserted as
    // *creation*, and the second hash call is the only real comparison.
    assert!(
        snapshot.matches_image(&stem).unwrap(),
        "the PNG must round-trip to the snapshot that wrote it"
    );
    assert!(png_path.exists(), "the review PNG must exist");

    assert!(
        snapshot.matches_hash(&stem).unwrap(),
        "hash must be written"
    );
    let hash = std::fs::read_to_string(&hash_path)
        .expect("the review hash must exist")
        .trim()
        .to_string();
    assert_eq!(64, hash.len(), "a SHA-256 hex digest is 64 chars: {hash}");

    // Now that the hash file exists this is a genuine comparison.
    assert!(
        snapshot.matches_hash(&stem).unwrap(),
        "the raster must reproduce its own hash within one process"
    );

    println!("PNG  {}", png_path.display());
    println!("SHA  {hash}");
    println!(
        "SIZE {} bytes",
        std::fs::metadata(&png_path)
            .expect("the PNG must be readable")
            .len()
    );
}

// ---------------------------------------------------------------------------
// BUG-01 canary
// ---------------------------------------------------------------------------

/// Builds the same tree the BUG-01 repro uses: one widget above a sibling,
/// both inside a scrollable whose main axis is unbounded.
fn stacked_in_scrollable(widget: Element<'static, Message>) -> Probe {
    let root: Element<'_, Message> = scrollable(
        Column::new()
            .push(widget)
            .push(iced::widget::button("sibling").width(iced::Length::Fixed(120.0))),
    )
    .height(iced::Length::Fill)
    .into();

    Probe::capture(Size::new(600.0, 900.0), root)
}

/// BUG-01: a full-height hoarfrost widget inside a scrollable propagates a
/// non-finite height, and the sibling below it is placed at `y = inf`.
///
/// This is a **canary**, not an endorsement. `BUGS.md` records the finding
/// and asks hoarfrost for a finite guard. Until one exists, this test
/// asserts the behaviour still occurs: if hoarfrost ever clamps
/// `limits.max()` (or stops declaring `Length::Fill` on an unbounded axis)
/// this test fails, and that is the signal to retire the note rather than
/// let it go stale.
///
/// Ignored by default because it documents a framework gap rather than
/// something the demo regressed on; run it with
/// `cargo nextest run --run-ignored only repro_bug01`.
#[test]
#[ignore = "BUG-01 canary: asserts a hoarfrost robustness gap still exists"]
fn repro_bug01_infinite_height_in_unbounded_axis() {
    use crate::pane_composites::CompositesState;

    let mut state = CompositesState::new();
    state.rail = crate::data::rail_model();
    state.sheet = crate::data::sheet_model();
    state.palette = crate::data::palette_model();

    let cases: [(&str, Element<'static, Message>); 3] = [
        (
            "NavigationRail",
            hoarfrost::composite::navigation_rail::NavigationRail::new(state.rail.clone()).into(),
        ),
        (
            "SideSheet",
            hoarfrost::composite::side_sheet::SideSheet::new(
                state.sheet.clone(),
                Column::new().push("body"),
            )
            .into(),
        ),
        (
            "CommandPalette",
            hoarfrost::composite::command_palette::CommandPalette::new(state.palette.clone())
                .into(),
        ),
    ];

    for (name, widget) in cases {
        let probe = stacked_in_scrollable(widget);
        let infinite = probe.infinite_entries();

        assert!(
            !infinite.is_empty(),
            "{name}: BUG-01 expected non-finite geometry inside an unbounded \
             scrollable but found none - if hoarfrost added a finite guard, \
             retire this canary and update BUGS.md"
        );

        // The widget's own node is non-finite, and the poison reaches the
        // sibling below it.
        let poisoned_sibling = infinite
            .iter()
            .any(|entry| !entry.bounds.y.is_finite() && entry.kind != "scrollable");
        assert!(
            poisoned_sibling,
            "{name}: expected the sibling below to be placed at a non-finite y, \
             got: {:?}",
            infinite.iter().map(|e| e.line()).collect::<Vec<_>>()
        );
    }
}
