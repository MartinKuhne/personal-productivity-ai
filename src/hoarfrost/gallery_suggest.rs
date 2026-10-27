//! The autosuggest cells of the `UIF-VER-004` matrix (§3.2,
//! `UIF-ATOM-002d`).
//!
//! Built through [`gallery`]'s own vocabulary; [`gallery::matrix`] calls
//! [`suggest_cells`] back so the matrix remains one list in one fixed
//! order.
//!
//! # Two groups on two different axes
//!
//! The **field cells** sample the control's chrome: the `Search` input at
//! every reachable [`VisualState`] and both control-height rungs, with
//! the flyout recorded as open or closed. They deliberately reuse
//! [`text_input`]'s resolver rather than painting anything of their own —
//! §3.2 says the `AutoSuggestBox` *is* a `TextInput` with a flyout
//! attached, so a cell that introduced a second field style would put a
//! second, unreconciled definition of "what a text field looks like"
//! into the matrix. Every colour these cells emit is the colour the
//! `UIF-ATOM-002` cells already emit.
//!
//! The **tier cells** sample the machine: each one drives a real
//! [`AutoSuggestHost`] against a real [`AutoSuggestModel`] and a real
//! [`FocusState`](crate::focus::FocusState) and records the
//! [`AutoSuggestEmission`] set the host actually produced. As with the
//! combobox tiers, a cell that merely asserted a tier name would still
//! pass if the host emitted the wrong one; these cannot, because the
//! recorded names are read back out of what the host returned.
//!
//! # The three things only a driven cell can show
//!
//! [`SuggestTier::DebounceLive`] and [`SuggestTier::Request`] are split
//! apart because that gap *is* the debounce: the keystroke reports on the
//! `Live` tier immediately and the provider is not asked until the window
//! closes. A single "typing" cell would hide the one behaviour §3.2's
//! async contract turns on.
//!
//! [`SuggestTier::StaleAnswer`] drives the out-of-order race — a second
//! request in flight, the first answer arriving late — and records
//! `stale_dropped` rather than claiming the guard works.
//!
//! [`SuggestTier::NoMatch`] types a query the provider answers with
//! nothing and records that the flyout *did not open*, which is the
//! engine's
//! [`NoFocusableRows`](crate::surface::menu::cascade::IgnoredReason::NoFocusableRows)
//! invariant made visible instead of asserted.
//!
//! [`gallery`]: crate::gallery
//! [`gallery::matrix`]: crate::gallery::matrix
//!
//! Unit tests live in the sibling `gallery_suggest_tests.rs` sidecar.

use crate::focus::{FocusId, FocusMember};
use crate::gallery::{
    AuditPair, Cell, CellKind, ContrastRole, Namer, RUNGS, Rung, StyleEntry, atom_sample_at,
    paint_frame, snapshot_states, state_slug,
};
use crate::state::VisualState;
use crate::surface::tooltip::TimeMs;
use crate::token::color::Palette;
use crate::widget::auto_suggest_host::{AutoSuggestEmission, AutoSuggestHost, AutoSuggestKey};
use crate::widget::auto_suggest_model::{self, AutoSuggestModel, DEBOUNCE_MS, SuggestionItem};
use crate::widget::button_widget::Layer;
use crate::widget::text_input::{self, InputKind};
use iced::Rectangle;

/// The field's focus identity inside the gallery's own focus group.
const FIELD_ID: &str = "gallery-suggest-field";

/// The instant every driven cell starts from.
const T0: TimeMs = TimeMs::millis(10_000);

/// An instant `ms` after [`T0`].
fn at(ms: u64) -> TimeMs {
    TimeMs::millis(10_000 + ms)
}

/// The gesture one tier cell drives.
///
/// The set is the control's whole observable surface: the two halves of
/// the debounce, the highlight, the three commit sources, the two
/// `Escape` stages, and the two failure shapes — no match and a stale
/// answer — that no happy-path cell can reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SuggestTier {
    /// A keystroke inside the window — the query's `Live` tier, with no
    /// provider request yet.
    DebounceLive,
    /// The window closing — exactly one request dispatched, still `Live`.
    Request,
    /// `Down` on an open flyout — the highlight's `Live` tier.
    Highlight,
    /// `Enter` on the highlighted row — `Commit`.
    CommitEnter,
    /// A pointer release over a row — `Commit`.
    CommitClick,
    /// `Tab` on the highlighted row — `Commit`, with focus expected to
    /// move on.
    CommitTab,
    /// `Escape` stage one — the `Dismiss` tier; the text survives.
    Dismiss,
    /// `Escape` stage two — the query clears on the `Commit` tier.
    Clear,
    /// A query the provider answers with nothing — nothing opens.
    NoMatch,
    /// An answer for a generation no longer in flight — refused.
    StaleAnswer,
    /// Focus loss with edits outstanding — the query settles.
    FocusLoss,
}

impl SuggestTier {
    /// The tier's cell-id slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::DebounceLive => "debounce-live",
            Self::Request => "request",
            Self::Highlight => "highlight",
            Self::CommitEnter => "commit-enter",
            Self::CommitClick => "commit-click",
            Self::CommitTab => "commit-tab",
            Self::Dismiss => "dismiss",
            Self::Clear => "clear",
            Self::NoMatch => "no-match",
            Self::StaleAnswer => "stale-answer",
            Self::FocusLoss => "focus-loss",
        }
    }

    /// The binding tier this cell expects the host to have used last.
    #[must_use]
    pub const fn tier_name(self) -> &'static str {
        match self {
            Self::DebounceLive | Self::Request | Self::Highlight => "Live",
            Self::CommitEnter
            | Self::CommitClick
            | Self::CommitTab
            | Self::Clear
            | Self::FocusLoss => "Commit",
            Self::Dismiss => "Dismiss",
            Self::NoMatch | Self::StaleAnswer => "none",
        }
    }

    /// Whether this tier's gesture settles a value rather than reporting
    /// motion toward one.
    #[must_use]
    pub const fn settles(self) -> bool {
        matches!(
            self,
            Self::CommitEnter | Self::CommitClick | Self::CommitTab | Self::Clear | Self::FocusLoss
        )
    }
}

/// The three candidates every driven cell is answered with.
#[must_use]
fn candidates() -> Vec<SuggestionItem<String>> {
    vec![
        SuggestionItem::new("doc-readme".to_string(), "README.md"),
        SuggestionItem::new("doc-notes".to_string(), "notes.md"),
        SuggestionItem::new("doc-todo".to_string(), "todo.md"),
    ]
}

/// A focus state in which the field can hold the keyboard and has a
/// neighbour to fall back to.
#[must_use]
fn gallery_focus() -> crate::focus::FocusState {
    let mut focus = crate::focus::FocusState::new();
    focus.register_group(
        crate::focus::GroupId::new("gallery-suggest"),
        vec![
            FocusMember::new(FIELD_ID),
            FocusMember::new("gallery-suggest-next"),
        ],
    );
    focus
}

/// The names of the emissions a call produced, for recording.
fn names(emissions: &[AutoSuggestEmission<String>]) -> Vec<&'static str> {
    emissions.iter().map(AutoSuggestEmission::name).collect()
}

/// The generation a `QueryRequested` carried, if one was produced.
fn generation_of(emissions: &[AutoSuggestEmission<String>]) -> Option<u64> {
    emissions.iter().find_map(|emission| match emission {
        AutoSuggestEmission::QueryRequested { generation, .. } => Some(*generation),
        _ => None,
    })
}

/// What a driven gesture left behind, as the cell records it.
struct Driven {
    /// Every emission the sequence produced, oldest first.
    emissions: Vec<&'static str>,
    /// Whether the flyout ended up open.
    open: bool,
    /// Responses refused for the wrong generation.
    stale: usize,
    /// The text the field settled on.
    query: String,
    /// Whether a provider request is still outstanding.
    busy: bool,
}

impl Driven {
    /// Snapshot the host's observable state after a sequence.
    fn capture(
        host: &AutoSuggestHost<String>,
        model: &AutoSuggestModel<String>,
        emissions: Vec<&'static str>,
    ) -> Self {
        Self {
            emissions,
            open: host.is_open(),
            stale: host.stale_dropped(),
            query: model.query.clone(),
            busy: host.is_busy(),
        }
    }
}

/// Type `text`, wait the window out, and answer with `list`.
///
/// Returns the emissions the answer produced together with the
/// generation the request carried, so a caller reasoning about a *later*
/// answer has the token to reason with.
fn open_over(
    host: &mut AutoSuggestHost<String>,
    model: &mut AutoSuggestModel<String>,
    focus: &mut crate::focus::FocusState,
    text: &str,
    ms: u64,
    list: Vec<SuggestionItem<String>>,
) -> (Vec<&'static str>, u64) {
    let _ = host.note_edit(model, text.to_string(), at(ms));
    let fired = host.tick(model, at(ms + DEBOUNCE_MS));
    let generation =
        generation_of(&fired).expect("a closed window must dispatch exactly one request");
    (
        names(&host.apply_suggestions(model, generation, list, focus)),
        generation,
    )
}

/// Drive one tier and record what the host actually did.
#[must_use]
fn drive(tier: SuggestTier) -> Driven {
    let mut host = AutoSuggestHost::<String>::new(FocusId::new(FIELD_ID));
    let mut focus = gallery_focus();
    let mut model = AutoSuggestModel::<String>::new();
    let _ = host.note_focus(&mut model, true, &mut focus);

    let emissions: Vec<&'static str> = match tier {
        SuggestTier::DebounceLive => {
            // Inside the window: the text reports, the provider is not
            // asked yet.
            names(&host.note_edit(&mut model, "do".to_string(), T0))
        }
        SuggestTier::Request => {
            let _ = host.note_edit(&mut model, "do".to_string(), T0);
            names(&host.tick(&mut model, at(DEBOUNCE_MS)))
        }
        SuggestTier::Highlight => {
            let _ = open_over(&mut host, &mut model, &mut focus, "do", 0, candidates());
            names(&host.key(&mut model, AutoSuggestKey::Down, &mut focus))
        }
        SuggestTier::CommitEnter => {
            let _ = open_over(&mut host, &mut model, &mut focus, "do", 0, candidates());
            names(&host.key(&mut model, AutoSuggestKey::Enter, &mut focus))
        }
        SuggestTier::CommitClick => {
            let _ = open_over(&mut host, &mut model, &mut focus, "do", 0, candidates());
            names(&host.click(&mut model, 1, &mut focus))
        }
        SuggestTier::CommitTab => {
            let _ = open_over(&mut host, &mut model, &mut focus, "do", 0, candidates());
            names(&host.key(&mut model, AutoSuggestKey::Tab, &mut focus))
        }
        SuggestTier::Dismiss => {
            let _ = open_over(&mut host, &mut model, &mut focus, "do", 0, candidates());
            names(&host.key(&mut model, AutoSuggestKey::Escape, &mut focus))
        }
        SuggestTier::Clear => {
            let _ = open_over(&mut host, &mut model, &mut focus, "do", 0, candidates());
            let _ = host.key(&mut model, AutoSuggestKey::Escape, &mut focus);
            names(&host.key(&mut model, AutoSuggestKey::Escape, &mut focus))
        }
        SuggestTier::NoMatch => {
            // The provider answers, and the answer is nothing.
            let (applied, _) = open_over(&mut host, &mut model, &mut focus, "zz", 0, Vec::new());
            applied
        }
        SuggestTier::StaleAnswer => {
            let (_, first) = open_over(&mut host, &mut model, &mut focus, "do", 0, candidates());
            // Typing on puts a *second* request in flight ...
            let _ = host.note_edit(&mut model, "doc".to_string(), at(1_000));
            let second = host.tick(&mut model, at(1_000 + DEBOUNCE_MS));
            let second_generation =
                generation_of(&second).expect("the second window must dispatch a request");
            assert!(
                second_generation > first,
                "the second request must carry a later generation"
            );
            // ... and the *first* answer arrives late.
            names(&host.apply_suggestions(&mut model, first, candidates(), &mut focus))
        }
        SuggestTier::FocusLoss => {
            let _ = open_over(&mut host, &mut model, &mut focus, "do", 0, candidates());
            names(&host.note_focus(&mut model, false, &mut focus))
        }
    };

    Driven::capture(&host, &model, emissions)
}

/// The field's painted layers, resolved through `text_input` so the
/// autosuggest contributes no chrome of its own.
#[must_use]
fn field_layers(state: VisualState, bounds: Rectangle) -> Vec<(&'static str, Layer)> {
    let palette = Palette::dark();
    let style = text_input::resolve_input_style(InputKind::Search, state, &palette);
    let mut layers: Vec<(&'static str, Layer)> = Vec::new();
    if let Some(layer) = text_input::background_layer(bounds, &style) {
        layers.push(("background", layer));
    }
    if let Some(layer) = text_input::focus_ring_layer(bounds, &style) {
        layers.push(("focus-ring", layer));
    }
    layers
}

/// The contrast pairs the field paints, mirroring the `UIF-ATOM-002`
/// cell for the same `(kind, state)`.
#[must_use]
fn field_audit(state: VisualState) -> Vec<AuditPair> {
    let palette = Palette::dark();
    let style = text_input::resolve_input_style(InputKind::Search, state, &palette);
    let mut audit = vec![
        AuditPair {
            channel: "value",
            foreground: style.text,
            background: style.background,
            role: ContrastRole::Text,
        },
        AuditPair {
            channel: "placeholder",
            foreground: style.placeholder,
            background: style.background,
            role: ContrastRole::Text,
        },
        AuditPair {
            channel: "icon",
            foreground: style.icon,
            background: style.background,
            role: ContrastRole::Text,
        },
    ];
    if style.border_width > 0.0 {
        audit.push(AuditPair {
            channel: "border",
            foreground: style.border_color,
            background: style.background,
            role: ContrastRole::Chrome,
        });
    }
    if style.focus_ring_width > 0.0 {
        audit.push(AuditPair {
            channel: "ring",
            foreground: style.focus_ring_color,
            background: style.background,
            role: ContrastRole::Chrome,
        });
    }
    audit
}

/// The geometry every suggest cell records, so the figures §3.2 fixes are
/// visible in the snapshot rather than buried in a constant.
#[must_use]
fn geometry_entries(rung: Rung, bounds: Rectangle) -> Vec<StyleEntry> {
    vec![
        StyleEntry::number("flyout.max-height", auto_suggest_model::FLYOUT_MAX_HEIGHT),
        StyleEntry::number("flyout.row-height", auto_suggest_model::FLYOUT_ROW_HEIGHT),
        StyleEntry::number("flyout.gap", auto_suggest_model::FLYOUT_GAP),
        StyleEntry::number("flyout.radius", auto_suggest_model::FLYOUT_RADIUS),
        StyleEntry::number("debounce.window", DEBOUNCE_MS as f32),
        StyleEntry::number("geometry.width", bounds.width),
        StyleEntry::number("geometry.height", bounds.height),
        StyleEntry::text("geometry.height-token", rung.token()),
        StyleEntry::number("geometry.radius", text_input::RADIUS),
    ]
}

/// The autosuggest field cell for one `(state, rung, open)` triple
/// (`UIF-ATOM-002d`).
#[must_use]
pub fn suggest_field_cell(state: VisualState, rung: Rung, open: bool) -> Cell {
    // The same sample rectangle the `UIF-ATOM-002` cells use, so the
    // field's painted frame is comparable quad-for-quad against them.
    let bounds = atom_sample_at(rung.height());

    Cell {
        id: format!(
            "suggest/{}-{}{}",
            state_slug(state),
            if open { "open" } else { "closed" },
            rung.slug()
        ),
        kind: CellKind::Atom,
        subject: "suggest",
        spec: "UIF-ATOM-002d",
        namer: Namer::Standard,
        resolved: {
            let mut entries = vec![StyleEntry::text(
                "flyout",
                if open { "open" } else { "closed" },
            )];
            entries.extend(geometry_entries(rung, bounds));
            entries
        },
        frame: paint_frame(Namer::Standard, bounds, &field_layers(state, bounds)),
        audit: field_audit(state),
        note: None,
    }
}

/// Drive an [`AutoSuggestHost`] through one tier and record what it
/// actually emitted (`UIF-ATOM-002d`).
#[must_use]
pub fn suggest_tier_cell(tier: SuggestTier) -> Cell {
    let driven = drive(tier);
    let bounds = atom_sample_at(Rung::Standard.height());
    let state = VisualState {
        base: crate::state::BaseState::Rest,
        focused: true,
    };
    let recorded = driven.emissions.join("+");

    Cell {
        id: format!("suggest-tier/{}", tier.slug()),
        kind: CellKind::Composite,
        subject: "suggest-tier",
        spec: "UIF-ATOM-002d",
        namer: Namer::Standard,
        resolved: {
            let mut entries = vec![
                StyleEntry::text("tier", tier.tier_name()),
                StyleEntry::text("emissions", recorded),
                StyleEntry::text("flyout", if driven.open { "open" } else { "closed" }),
                StyleEntry::text("settles", tier.settles().to_string()),
                StyleEntry::text("busy", driven.busy.to_string()),
                StyleEntry::number("stale.dropped", driven.stale as f32),
                StyleEntry::text("query", driven.query.clone()),
            ];
            entries.extend(geometry_entries(Rung::Standard, bounds));
            entries
        },
        frame: paint_frame(Namer::Standard, bounds, &field_layers(state, bounds)),
        audit: field_audit(state),
        note: None,
    }
}

/// Every autosuggest cell: the field cells, then the tier cells, in that
/// order.
#[must_use]
pub fn suggest_cells() -> Vec<Cell> {
    let mut cells = Vec::new();
    for state in snapshot_states() {
        for rung in RUNGS {
            for open in [false, true] {
                cells.push(suggest_field_cell(state, rung, open));
            }
        }
    }
    for tier in [
        SuggestTier::DebounceLive,
        SuggestTier::Request,
        SuggestTier::Highlight,
        SuggestTier::CommitEnter,
        SuggestTier::CommitClick,
        SuggestTier::CommitTab,
        SuggestTier::Dismiss,
        SuggestTier::Clear,
        SuggestTier::NoMatch,
        SuggestTier::StaleAnswer,
        SuggestTier::FocusLoss,
    ] {
        cells.push(suggest_tier_cell(tier));
    }
    cells
}

#[cfg(test)]
#[path = "gallery_suggest_tests.rs"]
mod gallery_suggest_tests;
