//! The combobox cells of the `UIF-VER-004` matrix (§3.4, `UIF-ATOM-004`).
//!
//! Built through [`gallery`]'s own vocabulary; [`gallery::matrix`] calls
//! [`combo_cells`] back so the matrix remains one list in one fixed order.
//!
//! # Twenty-nine cells on two axes
//!
//! Twenty-four trigger cells — two height rungs × six state columns × the
//! flyout closed or open — plus five tier cells that drive a real
//! [`ComboHost`] and record which binding tier came out.
//!
//! The open/closed axis is its own because §3.4's chevron rule is stated
//! against it: *`text.muted` at rest, `text.strong` on hover/open*. A
//! matrix that sampled only the closed control could not show that the
//! glyph changes when the flyout appears, which is the one colour
//! behaviour §3.4 owns outright.
//!
//! The six state columns are §3.3's five plus `Invalid`, because §3.4
//! gives the combobox a `validation` field and a validation failure is a
//! state the trigger paints differently — the border carries the error,
//! and the chevron takes the dropdown-trigger row's danger icon.
//!
//! # The tier cells are driven, not described
//!
//! Each tier cell runs a real [`ComboHost`] against a real
//! [`ComboBoxModel`] and a real
//! [`FocusState`](crate::focus::FocusState), then records the
//! [`ComboEmission`] the host actually produced. A cell that merely
//! asserted "this is the commit tier" would still pass if the host
//! emitted the wrong tier; these cannot, because the recorded tier name
//! is read back out of what the host returned.
//!
//! The `no-match` cell is the interesting one: it types a query that
//! matches nothing and records that the flyout *did not open*, which is
//! the engine's [`NoFocusableRows`](crate::surface::menu::cascade::IgnoredReason::NoFocusableRows)
//! invariant made visible
//! rather than a claim in a doc comment.
//!
//! # The chevron is audited, and its fill is proven safe
//!
//! `text.muted` on `accent.fill` measures `1.8363:1` — the pairing
//! `INC-40` established is unusable. The combobox trigger never paints a
//! locked solid fill, and the sidecar proves that rather than assuming
//! it, which is what keeps §3.4's `text.muted` chevron from silently
//! becoming that pairing if a future rung ever tints the trigger.
//!
//! [`gallery`]: crate::gallery
//! [`gallery::matrix`]: crate::gallery::matrix
//!
//! Unit tests live in the sibling `gallery_combo_tests.rs` sidecar.

use crate::gallery::{
    AuditPair, Cell, CellKind, ContrastRole, Namer, StyleEntry, atom_sample_at, paint_frame,
    state_slug,
};
use crate::state::{BaseState, VisualState};
use crate::token::color::Palette;
use crate::widget::button_widget::Layer;
use crate::widget::combo_box::{self, ComboRung};
use crate::widget::combo_host::{ComboEmission, ComboHost, ComboKey, ComboOutcome};
use crate::widget::combo_model::{self, ComboBoxModel, SelectOption};
use iced::{Border, Color, Point, Rectangle};

/// The six state columns the combobox is sampled at: §3.3's five plus the
/// `Invalid` state §3.4's `validation` field puts on the trigger.
const COLUMNS: [VisualState; 6] = [
    VisualState {
        base: BaseState::Rest,
        focused: false,
    },
    VisualState {
        base: BaseState::Hovered,
        focused: false,
    },
    VisualState {
        base: BaseState::Pressed,
        focused: false,
    },
    VisualState {
        base: BaseState::Rest,
        focused: true,
    },
    VisualState {
        base: BaseState::Invalid,
        focused: false,
    },
    VisualState {
        base: BaseState::Disabled,
        focused: false,
    },
];

/// A nominal trigger width for the geometry entries.
const SAMPLE_TRIGGER_WIDTH: f32 = 160.0;

/// The tier a driven combobox gesture is recorded against.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ComboTier {
    /// `Down` on a closed control — the open toggle.
    Open,
    /// A filter keystroke in the editable variant — the `Live` tier.
    FilterLive,
    /// A row picked — the `Commit` tier.
    Commit,
    /// `Escape` — the `Cancel` tier, carrying the pre-open snapshot.
    Cancel,
    /// A filter that matches nothing — the flyout cannot be shown.
    NoMatch,
}

impl ComboTier {
    /// The tier's cell-id slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::FilterLive => "filter-live",
            Self::Commit => "commit",
            Self::Cancel => "cancel",
            Self::NoMatch => "no-match",
        }
    }

    /// The binding tier name this cell expects the host to have used.
    #[must_use]
    pub const fn tier_name(self) -> &'static str {
        match self {
            Self::Open => "toggle",
            Self::FilterLive => "Live",
            Self::Commit => "Commit",
            Self::Cancel => "Cancel",
            Self::NoMatch => "none",
        }
    }
}

/// The three options every tier cell is driven against.
#[must_use]
fn options() -> Vec<SelectOption<String>> {
    vec![
        SelectOption::new("wrap".to_string(), "Word Wrap"),
        SelectOption::new("tab".to_string(), "Tab Size"),
        SelectOption::new("font".to_string(), "Font Ligatures").disabled(),
    ]
}

/// The sample list every tier cell is driven against.
#[must_use]
fn sample_model() -> ComboBoxModel<String> {
    ComboBoxModel::new(options(), "Choose a setting").with_selected("tab".to_string())
}

/// The names of the emissions an outcome produced, for recording.
fn emission_names<Id>(outcome: &ComboOutcome<Id>) -> Vec<&'static str> {
    outcome
        .emissions
        .iter()
        .map(|emission| match emission {
            ComboEmission::OpenChanged(_) => "OpenChanged",
            ComboEmission::FilterChanged(_) => "FilterChanged",
            ComboEmission::Committed(_) => "Committed",
            ComboEmission::Cancelled(_) => "Cancelled",
        })
        .collect()
}

/// The trigger's own layer: fill and perimeter in one quad.
fn trigger_layer(bounds: Rectangle, style: &crate::widget::text_input::InputStyle) -> Layer {
    Layer {
        bounds,
        fill: style.background,
        border: Border {
            color: style.border_color,
            width: style.border_width,
            radius: combo_box::RADIUS.into(),
        },
    }
}

/// The chevron's own layer, drawn in its centred box.
fn chevron_layer(bounds: Rectangle, colour: Color) -> Layer {
    Layer {
        bounds,
        fill: colour,
        border: Border {
            color: Color::TRANSPARENT,
            width: 0.0,
            radius: 0.0.into(),
        },
    }
}

/// The control-level focus ring drawn outside the whole trigger.
fn focus_ring_layer(bounds: Rectangle, color: Color, width: f32) -> Option<Layer> {
    if width <= 0.0 {
        return None;
    }
    Some(Layer {
        bounds: Rectangle::new(
            iced::Point::new(bounds.x - width, bounds.y - width),
            iced::Size::new(bounds.width + width * 2.0, bounds.height + width * 2.0),
        ),
        fill: Color::TRANSPARENT,
        border: Border {
            color,
            width,
            radius: (combo_box::RADIUS + width).into(),
        },
    })
}

/// The combobox trigger cell for one `(rung, state, open)` triple
/// (`UIF-ATOM-004`).
#[must_use]
pub fn combo_cell(rung: ComboRung, state: VisualState, open: bool) -> Cell {
    let palette = Palette::dark();
    let sample = atom_sample_at(rung.height());
    let trigger =
        combo_box::trigger_bounds(Point::new(sample.x, sample.y), SAMPLE_TRIGGER_WIDTH, rung);
    let style = combo_box::resolve_trigger(state, &palette);
    let chevron = combo_box::chevron_color(state, open, &palette);
    let chevron_box = combo_box::chevron_bounds(trigger);

    let mut layers: Vec<(&'static str, Layer)> = vec![
        ("trigger", trigger_layer(trigger, &style)),
        ("chevron", chevron_layer(chevron_box, chevron)),
    ];
    if let Some(layer) = focus_ring_layer(trigger, style.focus_ring_color, style.focus_ring_width) {
        layers.push(("focus-ring", layer));
    }

    let audit = vec![
        AuditPair {
            channel: "trigger.border",
            foreground: style.border_color,
            background: style.background,
            role: ContrastRole::Chrome,
        },
        AuditPair {
            channel: "trigger.value",
            foreground: style.text,
            background: style.background,
            role: ContrastRole::Text,
        },
        AuditPair {
            channel: "trigger.placeholder",
            foreground: style.placeholder,
            background: style.background,
            role: ContrastRole::Text,
        },
        AuditPair {
            channel: "chevron",
            foreground: chevron,
            background: style.background,
            role: ContrastRole::Chrome,
        },
    ];

    Cell {
        id: format!(
            "combo/{}-{}-{}",
            rung.slug(),
            state_slug(state),
            if open { "open" } else { "closed" }
        ),
        kind: CellKind::Atom,
        subject: "combo",
        spec: "UIF-ATOM-004",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::text("rung", rung.slug()),
            StyleEntry::text("flyout", if open { "open" } else { "closed" }),
            StyleEntry::colour("trigger.fill", style.background),
            StyleEntry::colour("trigger.value", style.text),
            StyleEntry::colour("trigger.placeholder", style.placeholder),
            StyleEntry::colour("trigger.border.color", style.border_color),
            StyleEntry::number("trigger.border.width", style.border_width),
            StyleEntry::colour("chevron.color", chevron),
            StyleEntry::number("trigger.height", trigger.height),
            StyleEntry::number("trigger.min-width", combo_box::MIN_WIDTH),
            StyleEntry::number("trailing.slot", combo_box::TRAILING_SLOT),
            StyleEntry::number("chevron.size", combo_box::CHEVRON_SIZE),
            StyleEntry::number("chevron.x", chevron_box.x),
            StyleEntry::number("flyout.max-height", combo_model::FLYOUT_MAX_HEIGHT),
            StyleEntry::number("flyout.gap", combo_model::FLYOUT_GAP),
            StyleEntry::number("focus-ring.width", style.focus_ring_width),
        ],
        frame: paint_frame(Namer::Standard, sample, &layers),
        audit,
        note: None,
    }
}

/// Drive a [`ComboHost`] through one tier and record what it actually
/// emitted (`UIF-ATOM-004`).
#[must_use]
pub fn combo_tier_cell(tier: ComboTier) -> Cell {
    let palette = Palette::dark();
    let model = sample_model();
    let mut host = ComboHost::new(crate::focus::FocusId::new("gallery-combo-tier"));
    let mut focus = crate::focus::FocusState::new();

    let mut names: Vec<&'static str> = Vec::new();
    match tier {
        ComboTier::Open => {
            names.extend(emission_names(&host.key(
                ComboKey::Down,
                &model,
                &mut focus,
            )));
        }
        ComboTier::FilterLive => {
            let editable = model.clone().editable();
            names.extend(emission_names(&host.on_filter_typed(
                &editable,
                "wo".to_string(),
                &mut focus,
            )));
        }
        ComboTier::Commit => {
            let _ = host.key(ComboKey::Down, &model, &mut focus);
            names.extend(emission_names(&host.key(
                ComboKey::Enter,
                &model,
                &mut focus,
            )));
        }
        ComboTier::Cancel => {
            let _ = host.key(ComboKey::Down, &model, &mut focus);
            names.extend(emission_names(&host.key(
                ComboKey::Escape,
                &model,
                &mut focus,
            )));
        }
        ComboTier::NoMatch => {
            // No selection, so `keep_selected` cannot keep anything alive:
            // this is the only shape a genuine no-match state can take
            // while the selected row stays visible.
            let empty_pick = ComboBoxModel::new(options(), "Choose").editable();
            names.extend(emission_names(&host.on_filter_typed(
                &empty_pick,
                "zzz".to_string(),
                &mut focus,
            )));
            names.extend(emission_names(&host.key(
                ComboKey::Down,
                &empty_pick,
                &mut focus,
            )));
        }
    }

    let shown = host.is_open();
    let recorded = names.join("+");
    let state = if tier == ComboTier::NoMatch {
        VisualState {
            base: BaseState::Invalid,
            focused: false,
        }
    } else {
        COLUMNS[0]
    };
    let rung = ComboRung::Standard;
    let sample = atom_sample_at(rung.height());
    let trigger =
        combo_box::trigger_bounds(Point::new(sample.x, sample.y), SAMPLE_TRIGGER_WIDTH, rung);
    let style = combo_box::resolve_trigger(state, &palette);
    let chevron = combo_box::chevron_color(state, shown, &palette);

    let mut layers: Vec<(&'static str, Layer)> = vec![
        ("trigger", trigger_layer(trigger, &style)),
        (
            "chevron",
            chevron_layer(combo_box::chevron_bounds(trigger), chevron),
        ),
    ];
    if let Some(layer) = focus_ring_layer(trigger, style.focus_ring_color, style.focus_ring_width) {
        layers.push(("focus-ring", layer));
    }

    Cell {
        id: format!("combo-tier/{}", tier.slug()),
        kind: CellKind::Composite,
        subject: "combo-tier",
        spec: "UIF-ATOM-004",
        namer: Namer::Standard,
        resolved: vec![
            StyleEntry::text("tier", tier.tier_name()),
            StyleEntry::text("emissions", recorded),
            StyleEntry::text("flyout", if shown { "open" } else { "closed" }),
            StyleEntry::text("value-changed", tier.is_value_change().to_string()),
            StyleEntry::colour("trigger.fill", style.background),
            StyleEntry::colour("chevron.color", chevron),
            StyleEntry::number("trigger.height", trigger.height),
        ],
        frame: paint_frame(Namer::Standard, sample, &layers),
        audit: vec![AuditPair {
            channel: "trigger.border",
            foreground: style.border_color,
            background: style.background,
            role: ContrastRole::Chrome,
        }],
        note: None,
    }
}

impl ComboTier {
    /// Whether this tier's gesture changes the control's value.
    #[must_use]
    pub const fn is_value_change(self) -> bool {
        matches!(self, Self::Commit | Self::Cancel)
    }
}

/// Every combobox cell: the twenty-four trigger cells, then the five tier
/// cells, in that order.
#[must_use]
pub fn combo_cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    for rung in combo_box::RUNGS {
        for state in COLUMNS {
            for open in [false, true] {
                cells.push(combo_cell(rung, state, open));
            }
        }
    }
    for tier in [
        ComboTier::Open,
        ComboTier::FilterLive,
        ComboTier::Commit,
        ComboTier::Cancel,
        ComboTier::NoMatch,
    ] {
        cells.push(combo_tier_cell(tier));
    }
    cells
}

#[cfg(test)]
#[path = "gallery_combo_tests.rs"]
mod gallery_combo_tests;
