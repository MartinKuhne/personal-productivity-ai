//! The range-control cells of the `UIF-VER-004` matrix (§3.5,
//! `UIF-ATOM-005`).
//!
//! Split out of [`gallery`] because that file is already past the
//! `RUST-053` line limit. These cells are built through `gallery`'s own
//! vocabulary and [`gallery::matrix`] calls [`range_cells`] back, so the
//! matrix is still one list in one fixed order — the split moved the code,
//! not the artifact.
//!
//! # Thirty-one cells
//!
//! * the slider in two tick configurations across five states — 10 cells;
//! * the slider's drag tier — `Live`, `Commit`, `Cancel`, and a release
//!   with the pointer outside the track — 4 cells;
//! * the number box in both stepper shapes across five states — 10 cells;
//! * the number box's step tier — one keyboard step each way, a page each
//!   way, `Home`, `End`, and a press-and-hold — 7 cells.
//!
//! # The tier cells record a transcript, not a picture
//!
//! A tier is a statement about a *sequence* of events, so a still of the
//! control cannot show it. Each tier cell runs its script against a real
//! [`SliderHost`] or [`NumberBoxHost`] and records the tiers that came
//! out, in order, alongside the live/commit/cancel counts and the value
//! the gesture reached. The picture is the control at that final value;
//! the transcript is the thing under test.
//!
//! The `release-outside-track` cell is the one that matters most. A drag
//! whose pointer leaves the control must still settle rather than leak an
//! open gesture, and the recorded transcript shows exactly one `commit`
//! with the value clamped to the end the pointer left through.
//!
//! # The thumb is audited through whichever channel is carrying it
//!
//! The thumb's silhouette is the union of its face and its ring, and which
//! one is doing the work changes with the state: a resting thumb over the
//! filled rail is carried by its white face (`4.8020:1`), a hovered one by
//! its ring (`4.8020:1` again, from the other direction), a disabled one
//! by its raised face against the de-accented rail (`4.2379:1`). Each cell
//! therefore names the channel it measures after [`slider::thumb_carrier`]
//! rather than asserting a fixed `slider.thumb.ring` pair that would be
//! `CoincidentWithFill` — and so silently exempt — on the inverted states.
//!
//! The filled rail against the unfilled rail is **not** an audit pair. It
//! measures `2.9768:1`, below the chrome floor, and both rails are
//! interior to one control rather than a perimeter against the world. The
//! number is recorded as a resolved entry so it is in the artifact; see
//! [`slider::filled_on_unfilled`] for the decision and its reason.
//!
//! [`gallery`]: crate::gallery
//! [`gallery::matrix`]: crate::gallery::matrix
//!
//! Unit tests live in the sibling `gallery_range_tests.rs` sidecar.

use crate::gallery::{
    AuditPair, Cell, CellKind, ContrastRole, Namer, StyleEntry, atom_sample_at, paint_frame,
};
use crate::state::{BaseState, VisualState};
use crate::token::color::Palette;
use crate::token::size;
use crate::widget::button_widget::Layer;
use crate::widget::number_box::{self, NumberBoxStyle, StepperShape};
use crate::widget::number_box_host::{NumberBoxEmission, NumberBoxHost, NumberBoxOutcome};
use crate::widget::range_model::{RangeKey, RangeModel};
use crate::widget::slider::{self, SliderStyle};
use crate::widget::slider_host::{SliderEmission, SliderHost};
use iced::{Point, Rectangle};

/// The five states every range cell is sampled at.
///
/// `RestFocused` is its own entry rather than a second axis because the
/// focus ring is the one channel that is orthogonal to the pointer states
/// and has to be seen together with a resting control; sampling it against
/// `Pressed` as well would double the cell count to say one more thing
/// §3.5 does not add.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CellState {
    /// Nothing happening.
    Rest,
    /// Pointer over the control.
    Hovered,
    /// Button or thumb held down.
    Pressed,
    /// Resting, with the keyboard ring on.
    RestFocused,
    /// Inert.
    Disabled,
}

impl CellState {
    /// The [`VisualState`] this cell resolves.
    #[must_use]
    pub const fn visual(self) -> VisualState {
        match self {
            Self::Rest => VisualState {
                base: BaseState::Rest,
                focused: false,
            },
            Self::Hovered => VisualState {
                base: BaseState::Hovered,
                focused: false,
            },
            Self::Pressed => VisualState {
                base: BaseState::Pressed,
                focused: false,
            },
            Self::RestFocused => VisualState {
                base: BaseState::Rest,
                focused: true,
            },
            Self::Disabled => VisualState {
                base: BaseState::Disabled,
                focused: false,
            },
        }
    }

    /// This state's cell-id slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Rest => "rest",
            Self::Hovered => "hovered",
            Self::Pressed => "pressed",
            Self::RestFocused => "rest+focused",
            Self::Disabled => "disabled",
        }
    }

    /// Every state, in cell order.
    pub const ALL: [Self; 5] = [
        Self::Rest,
        Self::Hovered,
        Self::Pressed,
        Self::RestFocused,
        Self::Disabled,
    ];
}

/// The fraction every slider cell is sampled at.
///
/// Mid-track rather than at an end, so a single picture shows the filled
/// portion, the unfilled portion and the thumb all at once. At `0.0` the
/// fill is entirely hidden under the thumb and the cell cannot show the
/// boundary it exists to show.
const SAMPLE_FRACTION: f32 = 0.4;

/// The value every number box cell is sampled at.
const SAMPLE_VALUE: f64 = 45.0;

/// The range every cell is built on.
const RANGE: (f64, f64) = (0.0, 100.0);

/// The step every stepped cell is built on.
const SAMPLE_STEP: f64 = 5.0;

/// The unit suffix every number box cell carries.
const SAMPLE_UNIT: &str = "px";

/// The slider's sample box — tall enough that the thumb's full `24px` hit
/// target, and the focus ring drawn on it, are inside the recorded frame.
fn slider_sample() -> Rectangle {
    atom_sample_at(size::HIT_TARGET_MIN)
}

/// The slider's rail, inset so the `24px` hit box at either end stays
/// inside the sample rather than hanging past `x = 0`.
///
/// The inset box is passed through [`slider::track_rect`] rather than used
/// directly: the rail is the `4px` capsule centred in the sample, not the
/// sample itself, and a resolver that was handed the full-height box would
/// paint a `24px` groove and put the thumb above it.
fn slider_track(sample: Rectangle) -> Rectangle {
    let inset = slider::THUMB_HIT_SIZE / 2.0;
    slider::track_rect(Rectangle::new(
        Point::new(sample.x + inset, sample.y),
        iced::Size::new((sample.width - 2.0 * inset).max(0.0), sample.height),
    ))
}

/// A model at the sample value, stepped, with the sample unit suffix.
fn sample_model() -> RangeModel {
    RangeModel::new(SAMPLE_VALUE, RANGE.0, RANGE.1)
        .with_step(Some(SAMPLE_STEP))
        .with_unit_suffix(SAMPLE_UNIT)
}

/// The resolved style entries a slider cell carries.
fn slider_entries(style: &SliderStyle, fraction: f32, ticks: usize) -> Vec<StyleEntry> {
    let mut entries = vec![
        StyleEntry::colour("rail.unfilled", style.rail_unfilled),
        StyleEntry::colour("rail.border", style.rail_border),
        StyleEntry::colour("rail.filled", style.rail_filled),
        StyleEntry::colour("thumb.face", style.thumb_fill),
        StyleEntry::colour("thumb.ring", style.thumb_border),
        StyleEntry::number("thumb.ring.width", style.thumb_border_width),
        StyleEntry::number("thumb.size", slider::THUMB_SIZE),
        StyleEntry::number("thumb.hit", slider::THUMB_HIT_SIZE),
        StyleEntry::number("track.height", slider::TRACK_HEIGHT),
        StyleEntry::number("fraction", fraction),
        StyleEntry::number("ticks.count", ticks as f32),
        StyleEntry::colour("tick", style.tick),
        StyleEntry::number(
            "fill-on-unfill.ratio",
            f64::from(slider::filled_on_unfilled(&Palette::dark())) as f32,
        ),
    ];
    if style.focus_ring_width > 0.0 {
        entries.push(StyleEntry::colour("focus.ring", style.focus_ring_color));
        entries.push(StyleEntry::number(
            "focus.ring.width",
            style.focus_ring_width,
        ));
    }
    entries
}

/// The audit pair that proves the thumb reads where it is sitting.
///
/// The channel is named after whichever of face and ring actually clears
/// the floor against the filled rail, so the cell never reports an exempt
/// coincident pair in place of the measurement that matters.
fn thumb_audit(style: &SliderStyle) -> AuditPair {
    let (channel, foreground) = match slider::thumb_carrier(style, style.rail_filled) {
        slider::ThumbCarrier::Face | slider::ThumbCarrier::Both => {
            ("slider.thumb.face-on-fill", style.thumb_fill)
        }
        slider::ThumbCarrier::Ring => ("slider.thumb.ring-on-fill", style.thumb_border),
        slider::ThumbCarrier::Neither => {
            // Not reachable against the snapshot palette; the resolver's
            // own sidecar pins every state's carrier. Audited anyway so a
            // palette edit that produced it fails here rather than
            // disappearing into an unmatched arm.
            ("slider.thumb.face-on-fill", style.thumb_fill)
        }
    };
    AuditPair {
        channel,
        foreground,
        background: style.rail_filled,
        role: ContrastRole::Chrome,
    }
}

/// One slider cell: the groove, the fill, the ticks, the thumb and the ring.
#[must_use]
pub fn slider_cell(ticks: bool, state: CellState) -> Cell {
    let palette = Palette::dark();
    let style = slider::resolve_style(state.visual(), palette.surface_overlay, &palette);
    let sample = slider_sample();
    let track = slider_track(sample);
    let tick_count = if ticks { 11 } else { 0 };

    let mut layers: Vec<(&'static str, Layer)> = vec![("rail", slider::rail_layer(track, &style))];
    if let Some(filled) = slider::filled_layer(track, &style, SAMPLE_FRACTION) {
        layers.push(("fill", filled));
    }
    layers.extend(
        slider::tick_layers(track, &style, tick_count)
            .into_iter()
            .map(|layer| ("tick", layer)),
    );
    layers.push(("thumb", slider::thumb_layer(track, &style, SAMPLE_FRACTION)));
    if let Some(ring) = slider::focus_ring_layer(track, &style, SAMPLE_FRACTION) {
        layers.push(("focus-ring", ring));
    }

    let mut audit = vec![
        thumb_audit(&style),
        AuditPair {
            channel: "slider.fill-on-mount",
            foreground: style.rail_filled,
            background: palette.surface_overlay,
            role: ContrastRole::Chrome,
        },
    ];
    if style.focus_ring_width > 0.0 {
        audit.push(AuditPair {
            channel: "slider.focus-ring-on-mount",
            foreground: style.focus_ring_color,
            background: palette.surface_overlay,
            role: ContrastRole::Chrome,
        });
    }

    Cell {
        id: format!(
            "slider/{}-{}",
            if ticks { "ticks" } else { "plain" },
            state.slug()
        ),
        kind: CellKind::Atom,
        subject: "slider",
        spec: "UIF-ATOM-005",
        namer: Namer::Standard,
        resolved: slider_entries(&style, SAMPLE_FRACTION, tick_count),
        frame: paint_frame(Namer::Standard, sample, &layers),
        audit,
        note: None,
    }
}

/// Count the `Live` frames in a slider transcript.
fn slider_live(tiers: &[&'static str]) -> usize {
    tiers.iter().filter(|tier| **tier == "live").count()
}

/// A slider tier cell carrying the transcript its script produced.
fn slider_tier_cell(
    slug: &'static str,
    fraction: f32,
    tiers: &[&'static str],
    extra: Vec<StyleEntry>,
    note: String,
) -> Cell {
    let palette = Palette::dark();
    let style = slider::resolve_style(CellState::Rest.visual(), palette.surface_overlay, &palette);
    let sample = slider_sample();
    let track = slider_track(sample);

    let mut layers: Vec<(&'static str, Layer)> = vec![("rail", slider::rail_layer(track, &style))];
    if let Some(filled) = slider::filled_layer(track, &style, fraction) {
        layers.push(("fill", filled));
    }
    layers.push(("thumb", slider::thumb_layer(track, &style, fraction)));

    let mut resolved = vec![
        StyleEntry::text("transcript", tiers.join(" → ")),
        StyleEntry::number("transcript.live", slider_live(tiers) as f32),
        StyleEntry::number(
            "transcript.commit",
            tiers.iter().filter(|t| **t == "commit").count() as f32,
        ),
        StyleEntry::number(
            "transcript.cancel",
            tiers.iter().filter(|t| **t == "cancel").count() as f32,
        ),
        StyleEntry::number("fraction", fraction),
    ];
    resolved.extend(extra);

    Cell {
        id: format!("slider-tier/{slug}"),
        kind: CellKind::Atom,
        subject: "slider-tier",
        spec: "UIF-ATOM-005",
        namer: Namer::Standard,
        resolved,
        frame: paint_frame(Namer::Standard, sample, &layers),
        audit: vec![thumb_audit(&style)],
        note: Some(note),
    }
}

/// The `Live` scrub cell: press and drag, no release yet.
#[must_use]
pub fn slider_live_cell() -> Cell {
    let model = sample_model();
    let mut host = SliderHost::new();
    let tiers = [
        tier_of(&host.press(&model, 0.2)),
        tier_of(&host.drag(&model, 0.35)),
        tier_of(&host.drag(&model, 0.5)),
    ];

    slider_tier_cell(
        "live",
        0.5,
        &tiers,
        vec![
            StyleEntry::number("value", host.current().unwrap_or(0.0) as f32),
            StyleEntry::number("captured", host.snapshot().unwrap_or(0.0) as f32),
            StyleEntry::text("gesture-open", host.is_open().to_string()),
            StyleEntry::text("settled", host.settled().to_string()),
        ],
        "mid-drag: every frame is Live and nothing has persisted. The captured \
         pre-drag value is still held, which is what a Cancel would restore."
            .to_string(),
    )
}

/// The settled cell: press, drag, release — exactly one `Commit`.
#[must_use]
pub fn slider_committed_cell() -> Cell {
    let model = sample_model();
    let mut host = SliderHost::new();
    let mut tiers = Vec::new();
    tiers.push(tier_of(&host.press(&model, 0.2)));
    tiers.push(tier_of(&host.drag(&model, 0.35)));
    tiers.push(tier_of(&host.drag(&model, 0.5)));
    // Read the reached value before the release: releasing is what clears
    // the host, and the point of the cell is what the release carried.
    let reached = host.current().unwrap_or(0.0);
    let out = host.release(&model);
    tiers.push(tier_of(&out));
    let committed = match out.emissions.as_slice() {
        [SliderEmission::Committed(commit)] => *commit.value(),
        _ => f64::NAN,
    };
    let commits = tiers.iter().filter(|t| **t == "commit").count();

    slider_tier_cell(
        "committed",
        0.5,
        &tiers,
        vec![
            StyleEntry::number("value-before-release", reached as f32),
            StyleEntry::number("committed", committed as f32),
            StyleEntry::number("commits", commits as f32),
            StyleEntry::text("settled", host.settled().to_string()),
            StyleEntry::text("gesture-open", host.is_open().to_string()),
        ],
        "release settles once. A second release over the same gesture emits \
         nothing, because the gesture slot has already given up its commit — \
         the exactly-once property is structural, not counted."
            .to_string(),
    )
}

/// The cancel cell: Escape restores the pre-drag value.
#[must_use]
pub fn slider_cancelled_cell() -> Cell {
    let model = sample_model();
    let mut host = SliderHost::new();
    let mut tiers = Vec::new();
    tiers.push(tier_of(&host.press(&model, 0.2)));
    tiers.push(tier_of(&host.drag(&model, 0.5)));
    let dragged_to = host.current().unwrap_or(0.0);
    let out = host.cancel();
    tiers.push(tier_of(&out));
    let restored = match out.emissions.as_slice() {
        [SliderEmission::Cancelled(cancel)] => *cancel.restored(),
        _ => f64::NAN,
    };

    slider_tier_cell(
        "cancelled",
        0.2,
        &tiers,
        vec![
            StyleEntry::number("restored", restored as f32),
            StyleEntry::number("dragged-to", dragged_to as f32),
            StyleEntry::text("settled", host.settled().to_string()),
            StyleEntry::text("gesture-open", host.is_open().to_string()),
        ],
        "Escape abandons the drag and hands back the value the control showed \
         before the press, not the value the drag reached and not one step back."
            .to_string(),
    )
}

/// The release-outside-the-track cell: the gesture still settles.
#[must_use]
pub fn slider_release_outside_cell() -> Cell {
    let model = sample_model();
    let mut host = SliderHost::new();
    let sample = slider_sample();
    let track = slider_track(sample);
    // A pointer that has left the right edge of the control entirely.
    let outside_x = track.x + track.width + 500.0;
    let clamped = slider::fraction_at_x(track, outside_x);

    let mut tiers = Vec::new();
    tiers.push(tier_of(&host.press(&model, 0.2)));
    tiers.push(tier_of(&host.drag(&model, clamped)));
    let reached = host.current().unwrap_or(0.0);
    let capturing = host.is_capturing();
    tiers.push(tier_of(&host.release(&model)));

    slider_tier_cell(
        "release-outside-track",
        clamped,
        &tiers,
        vec![
            StyleEntry::number("pointer.x-outside-track", outside_x - track.x),
            StyleEntry::number("clamped-fraction", clamped),
            StyleEntry::number("value", reached as f32),
            StyleEntry::text("capturing-before-release", capturing.to_string()),
            StyleEntry::text("gesture-open", host.is_open().to_string()),
            StyleEntry::text("capturing", host.is_capturing().to_string()),
        ],
        "the pointer left the track mid-drag and the release still settled: \
         one commit at the clamped end, no leaked open gesture, capture \
         released. A NaN pointer x reads as the start rather than as a \
         position, which is the same finiteness rule the model applies \
         before it clamps."
            .to_string(),
    )
}

/// The number box sample box — the `32px` control rung.
fn number_box_sample() -> Rectangle {
    atom_sample_at(size::CONTROL_MD)
}

/// The resolved style entries a number box cell carries.
fn number_box_entries(style: &NumberBoxStyle, shape: StepperShape) -> Vec<StyleEntry> {
    vec![
        StyleEntry::colour("entry.background", style.entry.background),
        StyleEntry::colour("entry.text", style.entry.text),
        StyleEntry::colour("entry.border", style.entry.border_color),
        StyleEntry::number("entry.border.width", style.entry.border_width),
        StyleEntry::colour("stepper.background", style.stepper_background),
        StyleEntry::colour("stepper.border", style.stepper_border),
        StyleEntry::colour("stepper.glyph", style.stepper_glyph),
        StyleEntry::number("stepper.width", number_box::stepper_width(shape)),
        StyleEntry::number("step.button", number_box::STEP_BUTTON_SIZE),
        StyleEntry::text("stepper.shape", slug_shape(shape)),
        StyleEntry::text(
            "glyph.decrement",
            number_box::StepDirection::Decrement
                .glyph(shape)
                .to_string(),
        ),
        StyleEntry::text(
            "glyph.increment",
            number_box::StepDirection::Increment
                .glyph(shape)
                .to_string(),
        ),
        StyleEntry::text("value", sample_model().display()),
    ]
}

/// A [`StepperShape`] as a cell-id slug.
fn slug_shape(shape: StepperShape) -> &'static str {
    match shape {
        StepperShape::Stacked => "stacked",
        StepperShape::SideBySide => "side-by-side",
    }
}

/// One number box cell: the delegated entry, the block, the two buttons.
#[must_use]
pub fn number_box_cell(shape: StepperShape, state: CellState) -> Cell {
    let palette = Palette::dark();
    let style = number_box::resolve_style(state.visual(), &palette);
    let sample = number_box_sample();

    let mut layers: Vec<(&'static str, Layer)> = Vec::new();
    if let Some(entry) = number_box::entry_layer_for(shape, sample, &style) {
        layers.push(("entry", entry));
    }
    if let Some(ring) = number_box::entry_focus_ring_layer_for(shape, sample, &style) {
        layers.push(("entry-focus-ring", ring));
    }
    layers.push((
        "stepper",
        number_box::stepper_layer_for(shape, sample, &style),
    ));
    layers.extend(
        number_box::step_button_layers(sample, &style, shape)
            .into_iter()
            .map(|layer| ("step-button", layer)),
    );

    let mut audit = vec![
        AuditPair {
            channel: "number-box.glyph-on-stepper",
            foreground: style.stepper_glyph,
            background: style.stepper_background,
            role: ContrastRole::Text,
        },
        AuditPair {
            channel: "number-box.digits-on-entry",
            foreground: style.entry.text,
            background: style.entry.background,
            role: ContrastRole::Text,
        },
    ];
    if style.entry.border_width > 0.0 {
        audit.push(AuditPair {
            channel: "number-box.entry-border-on-mount",
            foreground: style.entry.border_color,
            background: palette.surface_overlay,
            role: ContrastRole::Chrome,
        });
    }

    Cell {
        id: format!("number-box/{}-{}", slug_shape(shape), state.slug()),
        kind: CellKind::Atom,
        subject: "number-box",
        spec: "UIF-ATOM-005",
        namer: Namer::Standard,
        resolved: number_box_entries(&style, shape),
        frame: paint_frame(Namer::Standard, sample, &layers),
        audit,
        note: None,
    }
}

/// A number box tier cell carrying the transcript its script produced.
fn number_box_tier_cell(
    slug: &'static str,
    tiers: &[&'static str],
    extra: Vec<StyleEntry>,
    note: String,
) -> Cell {
    let palette = Palette::dark();
    let style = number_box::resolve_style(CellState::Rest.visual(), &palette);
    let sample = number_box_sample();

    let mut layers: Vec<(&'static str, Layer)> = Vec::new();
    if let Some(entry) = number_box::entry_layer_for(StepperShape::Stacked, sample, &style) {
        layers.push(("entry", entry));
    }
    layers.push((
        "stepper",
        number_box::stepper_layer_for(StepperShape::Stacked, sample, &style),
    ));

    let mut resolved = vec![
        StyleEntry::text("transcript", tiers.join(" → ")),
        StyleEntry::number(
            "transcript.live",
            tiers.iter().filter(|t| **t == "live").count() as f32,
        ),
        StyleEntry::number(
            "transcript.commit",
            tiers.iter().filter(|t| **t == "commit").count() as f32,
        ),
        StyleEntry::number(
            "transcript.cancel",
            tiers.iter().filter(|t| **t == "cancel").count() as f32,
        ),
    ];
    resolved.extend(extra);

    Cell {
        id: format!("number-box-tier/{slug}"),
        kind: CellKind::Atom,
        subject: "number-box-tier",
        spec: "UIF-ATOM-005",
        namer: Namer::Standard,
        resolved,
        frame: paint_frame(Namer::Standard, sample, &layers),
        audit: vec![AuditPair {
            channel: "number-box.glyph-on-stepper",
            foreground: style.stepper_glyph,
            background: style.stepper_background,
            role: ContrastRole::Text,
        }],
        note: Some(note),
    }
}

/// The keyboard-step tier cells: every keyed move is a `Commit`, never a
/// `Live` frame, because §3.5 lists the keys under `on_commit`.
#[must_use]
pub fn number_box_key_cells() -> Vec<Cell> {
    let keys = [
        ("step-up", RangeKey::Increase, "one step up, committed"),
        ("step-down", RangeKey::Decrease, "one step down, committed"),
        ("page-up", RangeKey::PageUp, "ten steps up, committed"),
        ("page-down", RangeKey::PageDown, "ten steps down, committed"),
        ("home", RangeKey::Home, "the minimum, committed"),
        ("end", RangeKey::End, "the maximum, committed"),
    ];
    let mut cells = Vec::new();
    for (slug, key, what) in keys {
        let mut host = NumberBoxHost::new();
        let mut tiers = Vec::new();
        let mut reached = String::new();
        for step in 0..2 {
            let model = if step == 0 {
                sample_model()
            } else {
                model_at(reached.parse::<f64>().unwrap_or(SAMPLE_VALUE))
            };
            let out = host.key(&model, key);
            tiers.push(tier_of_box(&out));
            if let Some(value) = committed_value(&out) {
                reached = value.to_string();
            }
        }
        cells.push(number_box_tier_cell(
            slug,
            &tiers,
            vec![
                StyleEntry::text("key", what),
                StyleEntry::text("value-after", reached.clone()),
                StyleEntry::number(
                    "page.multiplier",
                    crate::widget::range_model::PAGE_STEP_MULTIPLIER as f32,
                ),
            ],
            "a held arrow key is a run of commits, not a scrub with one settle \
             at the end. The model is re-seeded between presses because that \
             is the contract a controlled host runs under."
                .to_string(),
        ));
    }
    cells
}

/// The press-and-hold cell: `Live` frames while held, one `Commit` on
/// release.
#[must_use]
pub fn number_box_held_cell() -> Cell {
    let model = sample_model();
    let mut host = NumberBoxHost::new();
    let mut tiers = Vec::new();
    tiers.push(tier_of_box(
        &host.press_stepper(&model, number_box::StepDirection::Increment),
    ));
    for tick in 1..=4 {
        tiers.push(tier_of_box(&host.hold_stepper(
            &model,
            number_box::StepDirection::Increment,
            tick * 100,
            100,
        )));
    }
    tiers.push(tier_of_box(&host.release_stepper(&model)));

    number_box_tier_cell(
        "held",
        &tiers,
        vec![
            StyleEntry::number("steps", host.steps() as f32),
            StyleEntry::number("repeat.interval-ms", 100.0),
            StyleEntry::text("settled", host.settled().to_string()),
        ],
        "the hold's cadence enters as two values — elapsed and interval — \
         because the crate never reads a clock. A repeat interval of zero \
         emits nothing rather than dividing by it."
            .to_string(),
    )
}

/// The same sample model resting at `value`.
fn model_at(value: f64) -> RangeModel {
    RangeModel::new(value, RANGE.0, RANGE.1)
        .with_step(Some(SAMPLE_STEP))
        .with_unit_suffix(SAMPLE_UNIT)
}

/// The tier a slider outcome settled at, as a slug.
fn tier_of(outcome: &crate::widget::slider_host::SliderOutcome) -> &'static str {
    match outcome.emissions.as_slice() {
        [SliderEmission::Changed(_)] => "live",
        [SliderEmission::Committed(_)] => "commit",
        [SliderEmission::Cancelled(_)] => "cancel",
        [] => "none",
        _ => "mixed",
    }
}

/// The tier a number box outcome settled at, as a slug.
fn tier_of_box(outcome: &NumberBoxOutcome) -> &'static str {
    match outcome.emissions.as_slice() {
        [NumberBoxEmission::Changed(_)] => "live",
        [NumberBoxEmission::Committed(_)] => "commit",
        [NumberBoxEmission::Cancelled(_)] => "cancel",
        [] => "none",
        _ => "mixed",
    }
}

/// The value a number box `Commit` carries, if the outcome is one.
fn committed_value(outcome: &NumberBoxOutcome) -> Option<f64> {
    match outcome.emissions.as_slice() {
        [NumberBoxEmission::Committed(commit)] => Some(*commit.value()),
        _ => None,
    }
}

/// Every range cell: ten slider cells, four slider tier cells, ten number
/// box cells and seven number box tier cells, in that order.
#[must_use]
pub fn range_cells() -> Vec<Cell> {
    let mut cells = Vec::new();

    for ticks in [false, true] {
        for state in CellState::ALL {
            cells.push(slider_cell(ticks, state));
        }
    }

    cells.push(slider_live_cell());
    cells.push(slider_committed_cell());
    cells.push(slider_cancelled_cell());
    cells.push(slider_release_outside_cell());

    for shape in [StepperShape::Stacked, StepperShape::SideBySide] {
        for state in CellState::ALL {
            cells.push(number_box_cell(shape, state));
        }
    }

    cells.extend(number_box_key_cells());
    cells.push(number_box_held_cell());

    cells
}

#[cfg(test)]
#[path = "gallery_range_tests.rs"]
mod gallery_range_tests;
