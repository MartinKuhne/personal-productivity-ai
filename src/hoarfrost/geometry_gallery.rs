//! Headless geometry gallery — every framework widget rendered **alone** in a
//! pinned box, so a layout or alignment defect can be pinned to the framework
//! rather than to whatever demo happens to compose it.
//!
//! # Why this exists
//!
//! When a consuming application reports that a control "sits two pixels low",
//! there are two candidate causes and the composed screen cannot tell them
//! apart: the widget's own `layout` may disagree with the height its module
//! declares, or the widget may be correct and the composition around it wrong.
//! This module removes the second candidate by laying each widget out on its
//! own, over iced's real renderer, inside a box larger than anything the
//! widget needs.
//!
//! # What "measured" means here
//!
//! [`measure_raw`] calls `Widget::layout` with
//! `layout::Limits::new(Size::ZERO, PIN)` — byte-for-byte what
//! `iced_runtime::UserInterface::build` does for the root widget
//! (`iced_runtime-0.14.0/src/user_interface.rs:104-108`). It is the only
//! route that reports a widget's own decision even when the widget registers
//! no hit candidate at its own bounds.
//!
//! [`candidate_report`] is the secondary diagnostic: it runs a recording
//! [`Operation`] over the same layout and reports every rectangle the widget
//! published through iced's registration hooks, which is how a widget that
//! lays out at 32px but hit-tests at 24px gets caught.
//!
//! # What cannot be measured
//!
//! A handful of things named in the design spec have no element builder at
//! all — no `Widget` implementation, only pure geometry and paint helpers.
//! Those are listed verbatim in [`NOT_RENDERABLE`] rather than quietly
//! skipped, because "the framework cannot render this alone" is itself the
//! finding.
//!
//! Unit tests live in the sibling `geometry_gallery_tests.rs` sidecar.

use std::any::Any;
use std::cell::RefCell;

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::Tree;
use iced::advanced::widget::operation::{self, Operation};
use iced::{Element, Rectangle, Size};
use iced_test::core::widget::Id;
use iced_test::renderer::Renderer as HeadlessRenderer;

use crate::binding::controlled::Controlled;
use crate::composite::form_field::{
    self, CONTROL_SLOT_HEIGHT, STACKED_HEIGHT, resolve_caption, resolve_header, stacked,
};
use crate::composite::navbar::{
    NAVBAR_HEIGHT_COMPACT, NAVBAR_HEIGHT_STANDARD, NavBar, NavBarMenuTrigger, NavBarModel,
};
use crate::composite::settings_row::{
    self, MIN_HEIGHT, horizontal, resolve_style, row_height, text_column,
};
use crate::focus::FocusId;
use crate::layout::density::ControlDensity;
use crate::test_helpers::{self, Blank, Theme};
use crate::token::color::Palette;
use crate::token::{size, typography};
use crate::widget::button::{
    ButtonVariant, HEIGHT_COMPACT as BUTTON_HEIGHT_COMPACT,
    HEIGHT_STANDARD as BUTTON_HEIGHT_STANDARD,
};
use crate::widget::button_model::ButtonDensity;
use crate::widget::button_widget::Button;
use crate::widget::hyperlink::{self, Hyperlink};
use crate::widget::rating::RatingModel;
use crate::widget::segmented::SegmentedButton;
use crate::widget::segmented_model::{SegmentItem, SegmentedModel};
use crate::widget::text_input::{
    HEIGHT_COMPACT as INPUT_HEIGHT_COMPACT, HEIGHT_STANDARD as INPUT_HEIGHT_STANDARD, InputKind,
    TextInput,
};
use crate::widget::toggle_model::{CheckState, RadioGroupModel, RadioOption};
use crate::widget::{
    Checkbox, RadioGroup, RangeSliderModel, ToggleSwitch, range_slider, rating_control,
};

/// Width of the pinned box every probe is laid out inside.
///
/// Wide enough that a `Length::Fill` control saturates against the box rather
/// than against its content, so a measured width is a statement about the
/// pin, not about a label.
pub const PIN_WIDTH: f32 = 400.0;

/// Height of the pinned box — generous, so no control is height-starved.
pub const PIN_HEIGHT: f32 = 100.0;

/// The pinned box itself.
pub const PIN: Size = Size::new(PIN_WIDTH, PIN_HEIGHT);

/// How far a measured height may sit from its declared height before the
/// gallery calls it a defect.
///
/// Half a pixel: sub-pixel disagreement is rounding, anything at or above it
/// is a layout decision that will show on screen.
pub const TOLERANCE: f32 = 0.5;

/// The element type every probe produces.
type GalleryEl = Element<'static, (), Theme, HeadlessRenderer>;

/// What one widget decided when it was laid out on its own.
///
/// Not `Copy`: the label is owned, see [`Self::widget`].
#[derive(Clone, Debug, PartialEq)]
pub struct Measured {
    /// What the gallery calls this widget, and what the conformance report
    /// names when it complains.
    ///
    /// Owned rather than `&'static str` because the variant and density
    /// probes build theirs at run time (`Button::primary (standard)`). A
    /// report label is produced and consumed inside one measurement, so it
    /// is owned by the row that carries it and reclaimed with it.
    pub widget: String,
    /// The height the widget's own module declares for this configuration,
    /// read from the token or constant the module itself uses.
    pub declared_height: f32,
    /// The height the widget actually laid out to inside [`PIN`].
    pub measured_height: f32,
    /// The width the widget actually laid out to inside [`PIN`].
    pub measured_width: f32,
    /// Whether [`Self::measured_height`] names a `ControlDensity` rung
    /// (`size.control.sm` / `size.control.md`).
    pub on_rung: bool,
}

impl Measured {
    /// Signed gap between what was measured and what was declared.
    #[must_use]
    pub fn delta(&self) -> f32 {
        self.measured_height - self.declared_height
    }

    /// Whether the measurement lands within [`TOLERANCE`] of the declaration.
    #[must_use]
    pub fn matches(&self) -> bool {
        self.delta().abs() <= TOLERANCE
    }
}

impl std::fmt::Display for Measured {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:<48} declared {:>6.1}  measured {:>6.1}  delta {:+6.2}  width {:>6.1}  rung {}",
            self.widget,
            self.declared_height,
            self.measured_height,
            self.delta(),
            self.measured_width,
            if self.on_rung { "yes" } else { "no" },
        )
    }
}

/// One rectangle a widget published through iced's operation hooks while it
/// stood alone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    /// Which hook published it: `container`, `focusable`, `text_input` or
    /// `custom`.
    pub kind: &'static str,
    /// The rectangle the widget claimed.
    pub bounds: Rectangle,
}

/// A widget, alone, plus what its own module claims about its height.
///
/// `'a` is the lifetime of whatever the probe borrows — see
/// [`GalleryModels`]. A probe that owns everything it renders is generic
/// over it for free.
struct Probe<'a> {
    /// Report label, owned for the same reason as [`Measured::widget`]: the
    /// variant and density probes format theirs inside a loop.
    label: String,
    /// The height the module declares.
    declared: f32,
    /// Whether the module claims the result lands on the `ControlDensity`
    /// ladder. Toggles claim the hit-target floor instead, so they are
    /// `false` here and are not accused of missing a rung they never
    /// claimed.
    claims_ladder: bool,
    /// Build the widget as an element borrowing from [`GalleryModels`].
    build: Box<dyn Fn() -> ProbeEl<'a> + 'a>,
}

/// The element type a probe builds while borrowing its host models.
type ProbeEl<'a> = Element<'a, (), Theme, HeadlessRenderer>;

/// The models the controlled probes borrow.
///
/// The probe list borrows them rather than each probe owning a copy: built
/// once per run, read by every probe that needs one, and dropped with the
/// measurement rather than kept for the life of the process.
struct GalleryModels {
    radio: RadioGroupModel<String>,
    segmented_standard: SegmentedModel<String>,
    segmented_compact: SegmentedModel<String>,
    navbar: NavBarModel,
}

impl GalleryModels {
    /// Build every model the probe table borrows.
    fn new() -> Self {
        Self {
            radio: RadioGroupModel::<String>::new(vec![RadioOption::new(
                "a".to_owned(),
                "Only option",
            )])
            .with_selected("a".to_owned()),
            segmented_standard: segmented_model(ButtonDensity::Standard),
            segmented_compact: segmented_model(ButtonDensity::Compact),
            navbar: navbar_model(),
        }
    }
}

/// The three-segment model the segmented probes borrow, at a given density.
fn segmented_model(density: ButtonDensity) -> SegmentedModel<String> {
    SegmentedModel::new(vec![
        SegmentItem::new("left".to_owned(), "Left"),
        SegmentItem::new("middle".to_owned(), "Middle"),
        SegmentItem::new("right".to_owned(), "Right"),
    ])
    .with_selected("middle".to_owned())
    .with_density(density)
}

/// The two-trigger model the nav bar probes borrow.
fn navbar_model() -> NavBarModel {
    let mut model = NavBarModel::new().with_brand("Workspace");
    model = model.with_trigger(NavBarMenuTrigger::new("file", "File"));
    model = model.with_trigger(NavBarMenuTrigger::new("edit", "Edit"));
    model
}

/// The gallery's headless renderer — iced's real renderer, no window.
///
/// Delegates to [`crate::test_helpers::headless_renderer`] so the text
/// metrics here are the metrics every other hoarfrost probe already trusts,
/// rather than a second copy of the same construction.
///
/// Renderer construction is the gallery's dominant cost — every probe is a
/// layout call — so each entry point builds one and reuses it across the
/// whole table rather than minting a renderer per probe.
///
/// # Panics
/// Panics when iced cannot build a headless renderer, which means the test
/// environment cannot measure anything.
#[must_use]
pub fn gallery_renderer() -> HeadlessRenderer {
    test_helpers::headless_renderer()
}

/// Build one probe, diff a fresh [`Tree`] against it, and lay it out inside
/// the pinned window.
///
/// Both measurements the gallery takes start from the same three steps —
/// they mirror `UserInterface::build` — so they share them rather than each
/// carrying its own copy.
fn laid_out<'a, F>(renderer: &HeadlessRenderer, build: &F) -> (ProbeEl<'a>, Tree, layout::Node)
where
    F: Fn() -> ProbeEl<'a> + ?Sized,
{
    let mut element = build();
    let mut tree = Tree::empty();
    tree.diff(element.as_widget());

    let limits = layout::Limits::new(Size::ZERO, PIN);
    let node = element.as_widget_mut().layout(&mut tree, renderer, &limits);

    (element, tree, node)
}

/// Lay one widget out alone inside [`PIN`] and report the size it chose.
///
/// Returns `(height, width)`.
#[must_use]
pub fn measure_raw<'a, F>(renderer: &HeadlessRenderer, build: &F) -> (f32, f32)
where
    F: Fn() -> ProbeEl<'a> + ?Sized,
{
    let (_, _, node) = laid_out(renderer, build);

    let size = node.size();
    (size.height, size.width)
}

/// Every widget the gallery could not render alone, and why.
///
/// These are not skipped measurements — they are framework gaps. Something
/// the spec names that has no `Widget` implementation cannot be laid out
/// alone, so no standalone geometry claim can be made about it at all.
pub const NOT_RENDERABLE: &[(&str, &str)] = &[
    (
        "Slider (single-thumb)",
        "widget/slider.rs carries SliderStyle plus pure paint/geometry helpers, and \
         widget::slider_host::SliderHost is the interaction engine, but neither \
         implements iced's Widget trait. There is no single-thumb slider to lay out; \
         RangeSlider is the only slider that renders.",
    ),
    (
        "list_tile",
        "composite/list_tile.rs exposes only pure geometry (slots, slots_in, \
         trailing_targets, hit_target) and layer helpers (focus_outline_layer, \
         row_layer). No element builder, so no tile can be rendered alone.",
    ),
    (
        "table_cell",
        "composite/table_cell.rs is pure geometry and colour resolution (cell_rect, \
         content_rect, bottom_separator, trailing_divider). No element builder.",
    ),
    (
        "table_column",
        "composite/table_column.rs is the column solver (resolve_widths, bounds, \
         place). No element builder.",
    ),
    (
        "table_header",
        "composite/table_header.rs is pure geometry and colour resolution \
         (header_cell_rect, resize_handle_rect, sort_indicator_rect). \
         No element builder.",
    ),
];

/// Measure every probe in the gallery.
#[must_use]
pub fn measure_all() -> Vec<Measured> {
    let models = GalleryModels::new();
    let renderer = gallery_renderer();

    probes(&models)
        .iter()
        .map(|probe| {
            let (height, width) = measure_raw(&renderer, probe.build.as_ref());
            Measured {
                widget: probe.label.clone(),
                declared_height: probe.declared,
                measured_height: height,
                measured_width: width,
                on_rung: ControlDensity::from_height(height).is_some(),
            }
        })
        .collect()
}

/// Every standalone geometry claim the framework cannot keep.
///
/// One line per problem: a height that disagrees with its declaration by more
/// than [`TOLERANCE`], or a height that misses the `ControlDensity` ladder a
/// module claims to sit on.
#[must_use]
pub fn framework_geometry_conformance() -> Vec<String> {
    let models = GalleryModels::new();
    let renderer = gallery_renderer();
    let mut problems = Vec::new();

    for probe in probes(&models) {
        let (height, width) = measure_raw(&renderer, probe.build.as_ref());
        let delta = height - probe.declared;

        if delta.abs() > TOLERANCE {
            problems.push(format!(
                "{} declares {:.1}px but lays out at {:.1}px (delta {:+.2}px, width {:.1}px)",
                probe.label, probe.declared, height, delta, width
            ));
        }

        if probe.claims_ladder && ControlDensity::from_height(height).is_none() {
            problems.push(format!(
                "{} lays out at {:.1}px, which is not a ControlDensity rung ({} / {})",
                probe.label,
                height,
                size::CONTROL_SM,
                size::CONTROL_MD
            ));
        }
    }

    problems
}

/// Report every hit candidate each widget registers while standing alone.
///
/// The secondary diagnostic: a widget may lay out at one size and publish a
/// candidate at another, which is how a control looks right and clicks wrong.
#[must_use]
pub fn candidate_report() -> Vec<(String, Vec<Candidate>)> {
    let models = GalleryModels::new();
    let renderer = gallery_renderer();

    probes(&models)
        .iter()
        .map(|probe| {
            let (mut element, mut tree, node) = laid_out(&renderer, probe.build.as_ref());

            let mut recorder = CandidateProbe::default();
            element.as_widget_mut().operate(
                &mut tree,
                Layout::new(&node),
                &renderer,
                &mut recorder,
            );

            (probe.label.clone(), recorder.finish_candidates())
        })
        .collect()
}

/// Render the whole gallery as an aligned table.
#[must_use]
pub fn format_table(rows: &[Measured]) -> String {
    let mut out = String::new();
    for row in rows {
        out.push_str(&format!("{row}\n"));
    }
    out
}

/// The settings-row floor the module prints (§3B.1), exposed so a report can
/// name the number rather than a literal.
#[must_use]
pub fn settings_row_floor() -> f32 {
    MIN_HEIGHT
}

// ---------------------------------------------------------------------------
// Probe table
// ---------------------------------------------------------------------------

/// Every probe in the gallery, in report order.
///
/// The controlled probes borrow their models from `models`, so the same
/// four models back every run of the table and are reclaimed when the
/// table is dropped. Each probe's own label is an owned `String`, built
/// where it varies and `to_owned()` where it does not.
///
/// Each builder is annotated `-> ProbeEl<'a>` on purpose. `Probe`'s build
/// field is a `dyn Fn` object, and a trait object is *invariant* in its
/// associated `Output` type — a `fn() -> Element<'static>` is therefore
/// not a `dyn Fn() -> Element<'a>`. Naming the lifetime at every builder
/// is what lets the borrowed and the self-contained probes sit in one
/// table.
fn probes<'a>(models: &'a GalleryModels) -> Vec<Probe<'a>> {
    let mut probes: Vec<Probe<'a>> = Vec::new();

    // -- Button: 4 variants x 2 densities ---------------------------------
    for variant in [
        ButtonVariant::Primary,
        ButtonVariant::Secondary,
        ButtonVariant::Ghost,
        ButtonVariant::Danger,
    ] {
        let name = variant_slug(variant);
        probes.push(Probe {
            label: format!("Button::{name} (standard)"),
            declared: BUTTON_HEIGHT_STANDARD,
            claims_ladder: true,
            build: Box::new(move || -> ProbeEl<'a> { button_element(variant, false) }),
        });
        probes.push(Probe {
            label: format!("Button::{name} (compact)"),
            declared: BUTTON_HEIGHT_COMPACT,
            claims_ladder: true,
            build: Box::new(move || -> ProbeEl<'a> { button_element(variant, true) }),
        });
    }

    // -- TextInput: every kind x 2 densities ------------------------------
    for kind in [
        InputKind::TextEntry,
        InputKind::Search,
        InputKind::DropdownTrigger,
        InputKind::Password,
        InputKind::TextArea,
    ] {
        let name = kind_slug(kind);
        probes.push(Probe {
            label: format!("TextInput::{name} (standard)"),
            declared: INPUT_HEIGHT_STANDARD,
            claims_ladder: true,
            build: Box::new(move || -> ProbeEl<'a> { text_input_element(kind, false) }),
        });
        probes.push(Probe {
            label: format!("TextInput::{name} (compact)"),
            declared: INPUT_HEIGHT_COMPACT,
            claims_ladder: true,
            build: Box::new(move || -> ProbeEl<'a> { text_input_element(kind, true) }),
        });
    }

    // -- Toggles: the hit-target floor, not a density rung ----------------
    probes.push(Probe {
        label: "Checkbox (standalone)".to_owned(),
        declared: size::HIT_TARGET_MIN,
        claims_ladder: false,
        build: Box::new(|| -> ProbeEl<'a> {
            Checkbox::standalone("Enable", CheckState::Unchecked, "geo-checkbox", |_| None).into()
        }),
    });
    probes.push(Probe {
        label: "ToggleSwitch (standalone)".to_owned(),
        declared: size::HIT_TARGET_MIN,
        claims_ladder: false,
        build: Box::new(|| -> ProbeEl<'a> {
            ToggleSwitch::standalone("Enable", false, "geo-switch", |_| None).into()
        }),
    });
    probes.push(Probe {
        label: "RadioGroup (one option, vertical)".to_owned(),
        declared: size::HIT_TARGET_MIN,
        claims_ladder: false,
        build: Box::new(|| -> ProbeEl<'a> {
            RadioGroup::controlled(Controlled::new(&models.radio), "geo-radio", |_| None).into()
        }),
    });

    // -- Range slider ------------------------------------------------------
    probes.push(Probe {
        label: "RangeSlider".to_owned(),
        declared: size::HIT_TARGET_MIN,
        claims_ladder: false,
        build: Box::new(|| -> ProbeEl<'a> {
            range_slider(RangeSliderModel::new(0.0, 100.0, 20.0, 80.0)).into()
        }),
    });

    // -- Rating ------------------------------------------------------------
    probes.push(Probe {
        label: "Rating (5)".to_owned(),
        declared: size::HIT_TARGET_MIN,
        claims_ladder: false,
        build: Box::new(|| -> ProbeEl<'a> {
            rating_control(RatingModel::new(5).with_value(3.0)).into()
        }),
    });

    // -- Hyperlink ----------------------------------------------------------
    probes.push(Probe {
        label: "Hyperlink (standalone)".to_owned(),
        declared: hyperlink::MIN_HEIGHT,
        claims_ladder: false,
        build: Box::new(|| -> ProbeEl<'a> {
            Hyperlink::standalone("Open", "https://example.test", "geo-link", |_| None).into()
        }),
    });

    // -- Segmented ----------------------------------------------------------
    probes.push(Probe {
        label: "Segmented (standard)".to_owned(),
        declared: BUTTON_HEIGHT_STANDARD,
        claims_ladder: true,
        build: Box::new(|| -> ProbeEl<'a> {
            SegmentedButton::controlled(
                Controlled::new(&models.segmented_standard),
                "geo-segmented",
                |_| None,
            )
            .into()
        }),
    });
    probes.push(Probe {
        label: "Segmented (compact)".to_owned(),
        declared: BUTTON_HEIGHT_COMPACT,
        claims_ladder: true,
        build: Box::new(|| -> ProbeEl<'a> {
            SegmentedButton::controlled(
                Controlled::new(&models.segmented_compact),
                "geo-segmented",
                |_| None,
            )
            .into()
        }),
    });

    // -- Composites ---------------------------------------------------------
    probes.push(Probe {
        label: "form_field::stacked (header+control+caption)".to_owned(),
        declared: STACKED_HEIGHT,
        claims_ladder: false,
        build: Box::new(|| -> ProbeEl<'a> {
            let palette = Palette::dark();
            stacked(
                resolve_header("Name", true, Some("hint"), &palette),
                Element::new(Blank::new(Size::new(200.0, CONTROL_SLOT_HEIGHT))),
                resolve_caption(true, Some("helper text"), None, &palette),
            )
        }),
    });
    probes.push(Probe {
        label: "form_field::stacked (header+control, empty caption)".to_owned(),
        declared: form_field::HEADER_HEIGHT
            + form_field::BAND_GAP
            + CONTROL_SLOT_HEIGHT
            + form_field::BAND_GAP
            + form_field::CAPTION_SLOT_HEIGHT,
        claims_ladder: false,
        build: Box::new(|| -> ProbeEl<'a> {
            let palette = Palette::dark();
            stacked(
                resolve_header("Name", false, None, &palette),
                Element::new(Blank::new(Size::new(200.0, CONTROL_SLOT_HEIGHT))),
                resolve_caption(true, None, None, &palette),
            )
        }),
    });
    probes.push(Probe {
        label: "settings_row::horizontal (title only, 32px control)".to_owned(),
        declared: row_height(false, size::CONTROL_MD),
        claims_ladder: false,
        build: Box::new(|| -> ProbeEl<'a> { settings_row_title_only() }),
    });
    probes.push(Probe {
        label: "settings_row::horizontal (title+description, 32px control)".to_owned(),
        declared: row_height(true, size::CONTROL_MD),
        claims_ladder: false,
        build: Box::new(|| -> ProbeEl<'a> { settings_row_with_description() }),
    });
    probes.push(Probe {
        label: "settings_row::text_column (title+description, no card padding)".to_owned(),
        declared: settings_row::text_column_height(true),
        claims_ladder: false,
        build: Box::new(|| -> ProbeEl<'a> { settings_row_text_column_only() }),
    });
    probes.push(Probe {
        label: "NavBar (standard)".to_owned(),
        declared: NAVBAR_HEIGHT_STANDARD,
        claims_ladder: false,
        build: Box::new(|| -> ProbeEl<'a> { NavBar::new(&models.navbar).into() }),
    });
    probes.push(Probe {
        label: "NavBar (compact)".to_owned(),
        declared: NAVBAR_HEIGHT_COMPACT,
        claims_ladder: false,
        build: Box::new(|| -> ProbeEl<'a> { NavBar::new(&models.navbar).compact().into() }),
    });

    probes
}

// ---------------------------------------------------------------------------
// Probe builders
// ---------------------------------------------------------------------------

/// A button, alone.
///
/// The label is deliberately wide: `Button::outer_size` carries a
/// `debug_assert!` that a content box of 28x28 or less with no tooltip is a
/// framework misuse, and a probe that tripped it would panic instead of
/// reporting.
fn button_element(variant: ButtonVariant, compact: bool) -> GalleryEl {
    let label = iced::widget::text("Save changes").size(typography::BODY.size);
    let button = Button::new(variant, label, (), FocusId::new("geo-button"));

    if compact {
        button.compact().into()
    } else {
        button.standard().into()
    }
}

/// A text input, alone.
fn text_input_element(kind: InputKind, compact: bool) -> GalleryEl {
    let input = TextInput::new(
        kind,
        "placeholder",
        "value",
        |_| (),
        FocusId::new("geo-input"),
    );

    if compact {
        input.compact().into()
    } else {
        input.standard().into()
    }
}

/// A settings row showing a title and nothing else.
fn settings_row_title_only() -> GalleryEl {
    let style = resolve_style(false, &Palette::dark());

    horizontal(
        "Sync on startup",
        None,
        Element::new(Blank::new(Size::new(size::CONTROL_MD, size::CONTROL_MD))),
        &style,
    )
}

/// A settings row showing a title and a description.
fn settings_row_with_description() -> GalleryEl {
    let style = resolve_style(false, &Palette::dark());

    horizontal(
        "Sync on startup",
        Some("Pull the workspace before the first frame"),
        Element::new(Blank::new(Size::new(size::CONTROL_MD, size::CONTROL_MD))),
        &style,
    )
}

/// The settings row's text column on its own, so the height that comes
/// back is the text column's and nothing else.
///
/// `text_column_height(true)` models title and description as two stacked
/// lines (`20 + 16 = 36`). Probing the column directly rather than a
/// whole row around it is what makes the number mean the stacking: a
/// `horizontal` that laid the two out side by side would return `20`
/// here, and the card's own padding cannot inflate the answer.
fn settings_row_text_column_only() -> GalleryEl {
    let style = resolve_style(false, &Palette::dark());

    text_column(
        "Sync on startup",
        Some("Pull the workspace before the first frame"),
        &style,
    )
    .into()
}

// ---------------------------------------------------------------------------
// Candidate recording
// ---------------------------------------------------------------------------

/// An [`Operation`] that records every rectangle a widget registers.
///
/// Same shape as [`crate::test_helpers::RecordFocusables`], widened to every
/// registration hook so a candidate published through `text_input` or
/// `custom` is visible too.
#[derive(Default)]
pub struct CandidateProbe {
    seen: RefCell<Vec<Candidate>>,
}

impl CandidateProbe {
    /// What has been recorded so far.
    #[must_use]
    pub fn finish_candidates(&self) -> Vec<Candidate> {
        self.seen.borrow().clone()
    }

    fn record(&mut self, kind: &'static str, bounds: Rectangle) {
        self.seen.borrow_mut().push(Candidate { kind, bounds });
    }
}

impl Operation for CandidateProbe {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
        operate(self);
    }

    fn container(&mut self, _id: Option<&Id>, bounds: Rectangle) {
        self.record("container", bounds);
    }

    fn focusable(
        &mut self,
        _id: Option<&Id>,
        bounds: Rectangle,
        _state: &mut dyn operation::Focusable,
    ) {
        self.record("focusable", bounds);
    }

    fn text_input(
        &mut self,
        _id: Option<&Id>,
        bounds: Rectangle,
        _state: &mut dyn operation::TextInput,
    ) {
        self.record("text_input", bounds);
    }

    fn custom(&mut self, _id: Option<&Id>, bounds: Rectangle, _state: &mut dyn Any) {
        self.record("custom", bounds);
    }
}

// ---------------------------------------------------------------------------
// Slugs
// ---------------------------------------------------------------------------

/// Report name for a button variant.
fn variant_slug(variant: ButtonVariant) -> &'static str {
    match variant {
        ButtonVariant::Primary => "primary",
        ButtonVariant::Secondary => "secondary",
        ButtonVariant::Ghost => "ghost",
        ButtonVariant::Danger => "danger",
    }
}

/// Report name for an input kind.
fn kind_slug(kind: InputKind) -> &'static str {
    match kind {
        InputKind::TextEntry => "text",
        InputKind::Search => "search",
        InputKind::DropdownTrigger => "dropdown",
        InputKind::Password => "password",
        InputKind::TextArea => "textarea",
    }
}

#[cfg(test)]
#[path = "geometry_gallery_tests.rs"]
mod geometry_gallery_tests;
