//! The text input atom — entry, search and dropdown trigger across the
//! five-state matrix of §3.2 (`UIF-ATOM-002`).
//!
//! Like [`button`](super::button), this module has two halves.
//! [`resolve_input_style`] is a pure function from `(kind, state, palette)`
//! to an [`InputStyle`], and [`TextInput`] is the iced widget that paints
//! it. The resolver is what makes every cell of §3.2 checkable without a
//! window; the widget is what makes it a thing a user can type into.
//!
//! # The matrix, as implemented
//!
//! | State | Background | Text | Placeholder | Icon | Border |
//! |---|---|---|---|---|---|
//! | Rest | `surface.code` | `text.primary` | `text.disabled` | `text.muted` | `1px border.subtle` |
//! | Hovered | `surface.code` | `text.primary` | `text.muted` | `text.primary` | `1px border.strong` |
//! | Focused | `surface.base` | `text.strong` | `text.muted` | `text.primary` | `2px accent.border` |
//! | Invalid | `surface.base` | `text.strong` | `text.muted` | `status.danger.text` | `2px status.danger.text` |
//! | Disabled | `surface.raised` | `text.disabled` | `text.disabled` | `text.disabled` | `1px border.subtle` |
//!
//! Three cells are derived rather than transcribed, because §3.2 leaves them
//! open and a resolver cannot return "unspecified":
//!
//! * **Focused placeholder and icon.** The spec names only the text and the
//!   selection highlight for that row. The placeholder takes `text.muted`
//!   — the same value Hovered already gives it, because a placeholder the
//!   user is about to replace should be more present than the resting
//!   `text.disabled` and less present than the value it will become — and
//!   the icon takes `text.primary`, extending the matrix's own rule that
//!   an *attended* icon (hovered) is brighter than a resting one.
//! * **Invalid placeholder.** The error row names the error *icon* and the
//!   value colour; a placeholder in an invalid field is still a
//!   placeholder, so it keeps `text.muted`.
//! * **Pressed.** §3.2 has no Pressed cell, because pressing an input
//!   places a caret rather than committing an action.
//!   [`crate::state`] can still produce `BaseState::Pressed`, so the
//!   resolver maps it to the Hovered cell rather than inventing a sixth
//!   skin. The fold is of the *base* only: a pressed-and-focused input
//!   still paints the Focused cell, because focus is the orthogonal
//!   half and keeps its replacement. [`TextInput`] never sets `pressed`
//!   at all, so the mapping is reachability plumbing for the resolver's
//!   totality, not a visual state the widget can show.
//!
//! # Focus is *not* additive here, and that is the spec
//!
//! §3.1 writes every focused button cell as "Rest **+** 2px … ring": the
//! base cell survives and a ring is added. §3.2 writes its Focused cell as
//! a **replacement** — the background moves from `surface.code` to
//! `surface.base` and the border becomes `2px accent.border`. An input
//! that kept its resting background under the focus ring would not match
//! the row it is specified by.
//!
//! [`crate::state`]'s `focused` half still applies — it is still an
//! orthogonal flag, not a rung — but what it *adds* here is a different
//! shape than it adds to a button: a whole cell rather than an annulus.
//!
//! # Invalid + focused: the error keeps the border, focus moves outside
//!
//! Both the Focused row and the Invalid row claim the same channel — a
//! 2px perimeter stroke — and only one stroke can be painted. The
//! decision is that **invalid wins the border and focus relocates to a
//! ring drawn outside the control**, for three reasons:
//!
//! 1. [`crate::state`] already ranks `Invalid` above every pointer state
//!    precisely so that "the act of clicking at [an error] would not
//!    visually clear it". Letting focus take the border would undo that
//!    ruling at the one place the two collide.
//! 2. The error is the more expensive thing to lose. A user who cannot see
//!    that a field is invalid cannot fix it; a user who cannot see which
//!    of two invalid fields is theirs can still tab.
//! 3. Neither signal is actually lost. [`focus::resolve_ring_color_for_background`]
//!    puts an accent ring around the danger border — `accent.border`
//!    clears `surface.base` at `6.6698:1`, well over the 3.0:1 chrome
//!    floor — so the keyboard user still sees exactly which control is
//!    theirs, and the error still owns the perimeter.
//!
//! When the field is focused and *not* invalid there is nothing competing
//! for the perimeter, so the 2px accent border is the focus indicator on
//! its own and no ring is drawn.
//!
//! # The trailing slot never moves the text
//!
//! `UIF-PRIN-003` forbids a state change from shifting a widget's
//! contents, and an input has three things that want the same corner: a
//! validation glyph, a search glyph and a clear button. Iced's own
//! `text_input` icon slot cannot satisfy this — its icon is measured from
//! the glyph, so swapping `🔍` for `×` changes the text bounds by the
//! difference between the two, and removing it changes them again.
//!
//! So the slot is reserved by the atom rather than by its content.
//! [`InputKind::trailing_slot_width`] fixes the width per kind from tokens,
//! [`trailing_slot_bounds`] and [`text_area_bounds`] are pure functions of
//! `(outer bounds, kind)` with **no parameter for what is in the slot**,
//! and the editor is laid out into the text area. Nothing in the geometry
//! path can see whether the slot is empty, showing an error or showing a
//! clear button — which is what makes the invariant structural rather
//! than a promise the sidecar has to keep re-checking.
//!
//! The slot is wide enough for the widest trailing affordance the spec
//! names for that kind, so the reservation is honest rather than a guess:
//! the search field reserves `size.control.sm` because §4.3.1's pinned
//! search footer puts a `28 × 28` action button there.
//!
//! # The Selection Foreground Lock, again
//!
//! [`InputStyle::selection_foreground`] is pinned to
//! [`text_strong`](Palette::text_strong) over
//! [`accent_fill`](Palette::accent_fill) — the `UIF-TOK-006` rule 4
//! foreground lock, which measures `4.8020:1` at the revised token
//! values. Iced renders a selection as a filled rectangle under the
//! value colour, so the lock holds because the value colour *is*
//! `text.strong` in every state a selection can exist in (focused, and
//! invalid-and-focused). The sidecar asserts the ratio rather than
//! trusting the pairing.
//!
//! # What the widget borrows and what it wraps
//!
//! Iced's `text_input` is one of only two widgets in the crate that
//! implement `operation::Focusable`, so real keyboard focus is available
//! and none of it is reimplemented here: editing, caret, selection,
//! clipboard and IME all stay iced's. What iced's style surface cannot
//! do is own this atom's chrome — its `Style` has one background and one
//! border for the widget's own bounds, and this atom's bounds are not its
//! text's bounds. So the inner editor is styled **transparent with a
//! zero-width border** and contributes only text, caret and selection,
//! while [`TextInput`] paints the [`Layer`]s the resolver describes,
//! exactly as [`button_widget`](super::button_widget) does. Wrap, not
//! fork.
//!
//! # On the binding foundation (`UIF-ATOM-002a`)
//!
//! [`TextInput::controlled`] puts a field on [`TextInputModel`] the way
//! [`Button::controlled`](super::button_widget::Button::controlled) put
//! the button on its model: the model is the truth, the widget reads it
//! every frame, and every edit travels back out as a
//! [`TextInputEmission`] tier rather than as a raw string the host
//! happens to be listening for. The legacy [`TextInput::new`] →
//! `on_input` path is untouched and still works, and the two paths paint
//! identically — which is what the sidecar's equality test pins rather
//! than what this comment asserts.
//!
//! Two problems the button never had to solve show up here.
//!
//! **The editor publishes the instant a keystroke lands, and
//! [`Shell::publish`] cannot be declined.** So the inner editor's
//! message type is `Option<Message>`: the wrapper drives it through a
//! shell over a buffer it owns and republishes only what it chooses. A
//! legacy cell forwards `Some(message)` one-for-one; a bound cell
//! records the edit and returns `None`, so the raw string never reaches
//! the host and the host hears only tiers.
//!
//! **`Escape` needs a snapshot, and where that snapshot is taken decides
//! whether `Escape` is useful or destructive.** The gesture is the
//! *editing session*, and the snapshot is taken at the **first edit
//! after focus**, not at focus itself. The controller's module doc
//! argues the two failure modes this avoids; the short version is that a
//! focus-gain snapshot reverts sessions nobody opened and goes stale the
//! moment the host syncs a field while it is focused, which turns
//! `Escape` into a way to destroy a value the user was already shown.
//!
//! [`TextInputEmission`]: super::text_input_controller::TextInputEmission
//! [`TextInput::new`]: crate::widget::text_input::TextInput::new
//!
//! Unit tests live in the sibling `text_input_tests.rs` sidecar.

use crate::binding::Controlled;
use crate::binding::Ephemeral;
use crate::binding::Live;
use crate::focus::FocusId;
use crate::state::{self, BaseState, InteractionInput, VisualState};
use crate::token::color::Palette;
use crate::token::contrast;
use crate::token::{shape, size, space, typography};
use crate::widget::button_widget::Layer;
use crate::widget::focus;
use crate::widget::text_input_controller::TextInputController;
use crate::widget::text_input_controller::TextInputEmission;
use crate::widget::text_input_model::TextInputModel;

use iced::Alignment;
use iced::Background;
use iced::Border;
use iced::Color;
use iced::Element;
use iced::Event;
use iced::Length;
use iced::Padding;
use iced::Pixels;
use iced::Rectangle;
use iced::Size;
use iced::advanced::Clipboard;
use iced::advanced::Shell;
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text::{self as text_core, LineHeight, Shaping, Wrapping};
use iced::advanced::widget::Id;
use iced::advanced::widget::Tree;
use iced::advanced::widget::Widget;
use iced::advanced::widget::operation::{self, Operation};
use iced::advanced::widget::tree;
use iced::alignment;
use iced::keyboard;
use iced::keyboard::Key;
use iced::keyboard::key::Named;
use iced::mouse;
use iced::widget::text_input::{
    self, State as EditorState, Status as EditorStatus, Style as EditorStyle,
};

use std::cell::RefCell;
use std::rc::Rc;

/// The standard input height — `size.control.md`, for dialog and form
/// fields (§3.2, `UIF-TOK-002`).
pub const HEIGHT_STANDARD: f32 = size::CONTROL_MD;

/// The compact input height — `size.control.sm`, for dense toolbars
/// (§3.2, `UIF-TOK-002`).
pub const HEIGHT_COMPACT: f32 = size::CONTROL_SM;

/// Horizontal padding inside an input — `space.md` (§3.2, `UIF-TOK-001`).
pub const HORIZONTAL_PADDING: f32 = space::MD;

/// Corner radius of every input — `radius.widget` (§3.2, `UIF-TOK-003`).
pub const RADIUS: f32 = shape::RADIUS_WIDGET;

/// Gap between the editable text and the reserved trailing slot —
/// `space.sm` (`UIF-TOK-001`).
///
/// The gap is part of the reservation, not of the glyph: it is charged to
/// the slot's side of the boundary so the text area is the same whether
/// the slot is empty or occupied.
pub const SLOT_GAP: f32 = space::SM;

/// The glyph size drawn inside the trailing slot — `size.icon.md`
/// (`UIF-TOK-002`).
pub const SLOT_GLYPH_SIZE: f32 = size::ICON_MD;

/// The idle search glyph (§4.3.1, pinned search footer).
pub const GLYPH_SEARCH: char = '🔍';

/// The clear glyph shown in a search field that holds a filter (§4.3.1).
pub const GLYPH_CLEAR: char = '×';

/// The validation-error glyph (§3.2, Invalid row).
///
/// §3.2 requires a trailing error *icon* but the spec's icon vocabulary
/// names no glyph for it — `⚠` is the only warning-shaped mark the spec
/// uses elsewhere (§4.7's Tools warning). Recorded as a token gap in the
/// style of ledger **G-12**: the shape is a named constant so a future
/// dedicated error glyph changes one line rather than three widgets.
pub const GLYPH_ERROR: char = '⚠';

/// The dropdown-trigger chevron (`UIF-ATOM-002`, trigger row).
///
/// Same gap as [`GLYPH_ERROR`]: the spec calls the third kind a "dropdown
/// trigger" without naming its glyph. `▾` is the conventional closed-state
/// mark and is distinct from the menu's submenu `>`, which points at a
/// sibling card rather than at a list this control owns.
pub const GLYPH_CHEVRON: char = '▾';

/// The reveal toggle of a password field (`UIF-ATOM-002c`).
///
/// Same shape as the [`GLYPH_ERROR`] / [`GLYPH_CHEVRON`] gap: §3.2 names
/// `👁‍🗨`, which is a **three-scalar** ZWJ sequence (eye + ZWJ + speech
/// balloon) and [`SlotContent`] carries its glyph as one `char`. The bare
/// eye is used — the same pictograph without the speech-balloon join.
/// Recorded as a token gap in the style of ledger **G-12**.
pub const GLYPH_REVEAL: char = '\u{1F441}';

/// A stroke width that draws no stroke at all.
///
/// The *absence* of a property rather than a visual magnitude, which is
/// what the widget layer is allowed to carry, and named so
/// [`background_layer`] and the inner editor's style read as "no stroke".
const NO_STROKE_WIDTH: f32 = 0.0;

/// A fill that paints nothing behind what it covers.
const NO_FILL: Color = Color::TRANSPARENT;

/// The colour of a focus ring that is not being drawn.
const NO_RING_COLOR: Color = Color::TRANSPARENT;

/// The floor a subtraction between two extents is clamped at, so a
/// viewport narrower than the atom's own chrome collapses the text area
/// rather than making it negative.
const NO_EXTENT: f32 = 0.0;

/// The three input kinds §3.2 names (`UIF-ATOM-002`).
///
/// The kind selects **slot content**, never paint: all three share one
/// colour matrix. It is carried through the resolver rather than inferred
/// from a placeholder string so a caller cannot change an input's state
/// matrix by rewording its hint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InputKind {
    /// A plain single-line text entry. Its slot is empty until the value
    /// fails validation, at which point it carries [`GLYPH_ERROR`].
    TextEntry,
    /// A search field. Idle it carries [`GLYPH_SEARCH`]; once it holds a
    /// filter the glyph becomes the actionable [`GLYPH_CLEAR`], per the
    /// pinned search footer (§4.3.1).
    Search,
    /// A dropdown trigger. Its slot permanently carries [`GLYPH_CHEVRON`]
    /// — the affordance *is* the content, so there is no empty state for
    /// it even at rest.
    DropdownTrigger,
    /// A secret field (`UIF-ATOM-002c`). Its slot permanently carries the
    /// reveal toggle [`GLYPH_REVEAL`], which **outranks** [`GLYPH_ERROR`]:
    /// a masked field must always be able to show itself, and the invalid
    /// state is still carried by the 2px danger border.
    Password,
    /// A multi-line field (`UIF-ATOM-002e`). Same horizontal partition as
    /// the single-line kinds, a taller row, and a slot reserved for
    /// [`GLYPH_ERROR`] — empty while the value is valid.
    TextArea,
}

impl InputKind {
    /// The width this kind permanently reserves for its trailing slot.
    ///
    /// Each kind reserves the widest thing that may legally occupy its
    /// slot, so the reservation is sized to the content set rather than
    /// guessed at:
    ///
    /// * [`Search`](Self::Search) reserves `size.control.sm` because
    ///   §4.3.1 puts a `28 × 28` action button in that slot.
    /// * Every other kind reserves `size.hit_target.min`, the floor
    ///   [`size`] states for "a close `×`, a clear `×`, an icon button".
    ///   Their glyphs are `size.icon.md`, but reserving only the glyph
    ///   would leave no room to make either of them actionable later
    ///   without moving the text — the exact regression the slot exists
    ///   to prevent.
    ///
    /// Every value clears the `24 × 24` hit-target floor in width.
    #[must_use]
    pub const fn trailing_slot_width(self) -> f32 {
        match self {
            InputKind::TextEntry => size::HIT_TARGET_MIN,
            InputKind::Search => size::CONTROL_SM,
            InputKind::DropdownTrigger => size::HIT_TARGET_MIN,
            InputKind::Password => size::HIT_TARGET_MIN,
            InputKind::TextArea => size::HIT_TARGET_MIN,
        }
    }
}

/// What the permanently reserved trailing slot carries this frame.
///
/// Slot *content* is a value decided by [`slot_content`], not a widget
/// the caller has to know to build. That is what keeps the search field's
/// idle-`🔍`-versus-clear-`×` behaviour from being a special-cased
/// widget: it is one arm of one function, and the geometry is identical
/// whichever arm is taken.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotContent {
    /// Nothing is shown. The slot's rectangle is still reserved — an
    /// empty slot is not an absent slot.
    Empty,
    /// A glyph the user cannot act on: the idle search mark, the
    /// dropdown chevron, the validation error mark.
    Passive(char),
    /// A glyph that publishes the widget's clear message when pressed and
    /// released inside the slot.
    Actionable(char),
    /// A glyph that publishes the widget's **toggle** message rather than
    /// the clear message (`UIF-ATOM-002c`'s reveal control).
    ///
    /// A third arm rather than a second `Actionable` because the two press
    /// differently *and* differently: a clear settles the field empty, a
    /// reveal changes nothing about the value and must not open an editing
    /// session. Same hit target, different contract.
    Toggle(char),
}

impl SlotContent {
    /// The glyph to draw, if any.
    #[must_use]
    pub const fn glyph(self) -> Option<char> {
        match self {
            SlotContent::Empty => None,
            SlotContent::Passive(glyph)
            | SlotContent::Actionable(glyph)
            | SlotContent::Toggle(glyph) => Some(glyph),
        }
    }

    /// Whether a press inside the slot publishes the clear message.
    #[must_use]
    pub const fn is_actionable(self) -> bool {
        matches!(self, SlotContent::Actionable(_))
    }

    /// Whether a press inside the slot publishes the toggle message
    /// (`UIF-ATOM-002c`).
    #[must_use]
    pub const fn is_toggle(self) -> bool {
        matches!(self, SlotContent::Toggle(_))
    }

    /// Whether the slot owns the pointer over its rectangle — i.e. whether
    /// a press there is the slot's rather than the editor's.
    ///
    /// A [`Passive`](SlotContent::Passive) glyph is decoration: pressing
    /// the idle search mark must still put the caret in the field. Only a
    /// glyph that *answers* something takes the press.
    #[must_use]
    pub const fn captures_press(self) -> bool {
        matches!(self, SlotContent::Actionable(_) | SlotContent::Toggle(_))
    }
}

/// Decide the trailing slot's content for a kind, a value and a verdict
/// (`UIF-ATOM-002`, `-002c`, `-002e`, §4.3.1).
///
/// Validation outranks everything *except a secret's own affordance*: an
/// invalid field shows the error mark even when it also holds a value
/// that could be cleared, because the error is why the user cannot
/// proceed. A password field is the one exception — taking its reveal
/// toggle away at the exact moment the user most wants to check what they
/// typed would be a worse failure than losing the glyph, and §3.2's
/// Invalid row still carries the 2px danger border, so the verdict is
/// not lost. Below that the kinds differ, and that is the *only* place
/// the kinds differ at all.
#[must_use]
pub const fn slot_content(kind: InputKind, has_value: bool, invalid: bool) -> SlotContent {
    if invalid && !matches!(kind, InputKind::Password) {
        return SlotContent::Passive(GLYPH_ERROR);
    }

    match kind {
        InputKind::TextEntry => SlotContent::Empty,
        InputKind::Search => {
            if has_value {
                SlotContent::Actionable(GLYPH_CLEAR)
            } else {
                SlotContent::Passive(GLYPH_SEARCH)
            }
        }
        InputKind::DropdownTrigger => SlotContent::Passive(GLYPH_CHEVRON),
        InputKind::Password => SlotContent::Toggle(GLYPH_REVEAL),
        InputKind::TextArea => SlotContent::Empty,
    }
}

/// Which cell of §3.2 a resolved [`VisualState`] paints.
///
/// Named rather than inlined so the resolver reads as a table and the
/// two derived combinations — `Pressed` folding into Hovered, and
/// `Invalid + focused` getting its own cell because the border channel is
/// contested — are visible as decisions instead of falling through a
/// match arm.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PaintedCell {
    /// §3.2 Rest.
    Rest,
    /// §3.2 Hovered, and the folded `Pressed` (see the module doc).
    Hovered,
    /// §3.2 Focused.
    Focused,
    /// §3.2 Invalid / Error.
    Invalid,
    /// §3.2 Invalid / Error *while focused* — the error keeps the border
    /// and focus moves to an outside ring.
    InvalidFocused,
    /// §3.2 Disabled.
    Disabled,
}

/// Project a [`VisualState`] onto the cell it paints.
///
/// `Disabled` dominates absolutely — [`state::resolve`] already zeroes
/// the ring for a dead control, and this keeps the disabled cell winning
/// even if a caller hands the resolver a state it did not produce.
/// `Invalid` outranks focus for the border, which is the whole
/// [`PaintedCell::InvalidFocused`] decision.
const fn painted_cell(state: VisualState) -> PaintedCell {
    match state.base {
        BaseState::Disabled => PaintedCell::Disabled,
        BaseState::Invalid => {
            if state.focused {
                PaintedCell::InvalidFocused
            } else {
                PaintedCell::Invalid
            }
        }
        BaseState::Pressed | BaseState::Hovered => {
            // `Pressed` folds into Hovered — §3.2 gives an input no
            // pressed cell — but the fold is of the *base* only. Focus is
            // the orthogonal half and still replaces the cell, exactly as
            // it does from `Hovered`.
            if state.focused {
                PaintedCell::Focused
            } else {
                PaintedCell::Hovered
            }
        }
        BaseState::Rest => {
            if state.focused {
                PaintedCell::Focused
            } else {
                PaintedCell::Rest
            }
        }
    }
}

/// The colours one painted cell fixes, before the focus ring is added.
struct CellPaint {
    background: Color,
    text: Color,
    placeholder: Color,
    border_color: Color,
    border_width: f32,
}

/// §3.2 — the cell table.
///
/// Every colour is a [`Palette`] field; the only non-colour values are
/// the two stroke weights, both tokens.
const fn cell_paint(cell: PaintedCell, palette: &Palette) -> CellPaint {
    match cell {
        PaintedCell::Rest => CellPaint {
            background: palette.surface_code,
            text: palette.text_primary,
            placeholder: palette.text_disabled,
            border_color: palette.border_subtle,
            border_width: shape::BORDER_HAIRLINE,
        },
        PaintedCell::Hovered => CellPaint {
            background: palette.surface_code,
            text: palette.text_primary,
            placeholder: palette.text_muted,
            border_color: palette.border_strong,
            border_width: shape::BORDER_HAIRLINE,
        },
        PaintedCell::Focused => CellPaint {
            background: palette.surface_base,
            text: palette.text_strong,
            placeholder: palette.text_muted,
            border_color: palette.accent_border,
            border_width: shape::BORDER_FOCUS,
        },
        PaintedCell::Invalid | PaintedCell::InvalidFocused => CellPaint {
            background: palette.surface_base,
            text: palette.text_strong,
            placeholder: palette.text_muted,
            border_color: palette.status_danger_text,
            border_width: shape::BORDER_FOCUS,
        },
        PaintedCell::Disabled => CellPaint {
            background: palette.surface_raised,
            text: palette.text_disabled,
            placeholder: palette.text_disabled,
            border_color: palette.border_subtle,
            border_width: shape::BORDER_HAIRLINE,
        },
    }
}

/// The trailing-slot glyph colour for a painted cell.
///
/// §3.2 prints one icon rule for all three kinds: the kind decides *what*
/// sits in the slot, never what colour it is. Matching the kinds
/// separately anyway is what pins that claim in code rather than leaving
/// it to a sentence — a future kind-specific rule changes exactly one arm
/// here instead of turning the resolver into a string comparison, and the
/// sidecar asserts every arm still agrees.
///
/// `UIF-ATOM-002c` is the rule's one visible consequence: because the
/// reveal toggle takes the cell's own icon colour, an invalid password
/// field paints its eye in `status.danger.text` even though the error
/// *glyph* was displaced by it.
const fn icon_color(kind: InputKind, cell: PaintedCell, palette: &Palette) -> Color {
    match kind {
        InputKind::TextEntry => cell_icon_color(cell, palette),
        InputKind::Search => cell_icon_color(cell, palette),
        InputKind::DropdownTrigger => cell_icon_color(cell, palette),
        InputKind::Password => cell_icon_color(cell, palette),
        InputKind::TextArea => cell_icon_color(cell, palette),
    }
}

/// The cell's own icon colour, shared by every kind today.
///
/// The Invalid row is the only one that swaps the icon to a status colour,
/// and it is `status.danger` — the error *text/icon* red of decision
/// **D-5**, not [`status_danger_fill`](Palette::status_danger_fill). The
/// glyph is written in the danger, never painted on it.
const fn cell_icon_color(cell: PaintedCell, palette: &Palette) -> Color {
    match cell {
        PaintedCell::Rest => palette.text_muted,
        PaintedCell::Hovered | PaintedCell::Focused => palette.text_primary,
        PaintedCell::Invalid | PaintedCell::InvalidFocused => palette.status_danger_text,
        PaintedCell::Disabled => palette.text_disabled,
    }
}

/// An input's fully resolved paint for one `(kind, state)` pair.
///
/// Every field is a value the renderer consumes as-is: no token lookup,
/// no conditional, no further resolution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputStyle {
    /// The fill behind the text.
    pub background: Color,
    /// The value colour.
    pub text: Color,
    /// The placeholder colour, shown while the value is empty.
    pub placeholder: Color,
    /// The trailing-slot glyph colour.
    pub icon: Color,
    /// The selection highlight fill — `accent.fill` in every state.
    pub selection_background: Color,
    /// The text colour over a selection: the `UIF-TOK-006` rule 3
    /// Selection Foreground Lock, pinned to
    /// [`text_strong`](Palette::text_strong).
    pub selection_foreground: Color,
    /// The perimeter stroke colour.
    pub border_color: Color,
    /// The perimeter stroke width, in logical pixels.
    pub border_width: f32,
    /// The keyboard focus ring colour — fully transparent when no ring is
    /// drawn.
    pub focus_ring_color: Color,
    /// The keyboard focus ring width — `0.0` when no ring is drawn.
    pub focus_ring_width: f32,
}

impl InputStyle {
    /// Whether the perimeter is carrying the validation error rather than
    /// a resting edge or a focus indicator.
    #[must_use]
    pub fn is_error_border(&self, palette: &Palette) -> bool {
        self.border_width == shape::BORDER_FOCUS && self.border_color == palette.status_danger_text
    }
}

/// Resolve the paint for one `(kind, state)` pair against a palette
/// (`UIF-ATOM-002`).
///
/// A pure function: the same three inputs always produce the same
/// [`InputStyle`]. [`state::resolve`] owns the precedence that decides
/// *which* cell applies; this function owns what that cell looks like.
///
/// The focus ring is not a constant. It is drawn only when the perimeter
/// is already spoken for — the `PaintedCell::InvalidFocused` case — and
/// it is resolved through [`focus::resolve_ring_color_for_background`]
/// against the background this function just resolved, so it stays
/// visible on whatever surface the input landed on (`UIF-TOK-006` rule 2,
/// decision **D-6**).
#[must_use]
pub fn resolve_input_style(kind: InputKind, state: VisualState, palette: &Palette) -> InputStyle {
    let cell = painted_cell(state);
    let base = cell_paint(cell, palette);

    let (focus_ring_color, focus_ring_width) = if cell == PaintedCell::InvalidFocused {
        (
            focus::resolve_ring_color_for_background(base.background, palette),
            shape::BORDER_FOCUS,
        )
    } else {
        (NO_RING_COLOR, NO_STROKE_WIDTH)
    };

    let style = InputStyle {
        background: base.background,
        text: base.text,
        placeholder: base.placeholder,
        icon: icon_color(kind, cell, palette),
        selection_background: palette.accent_fill,
        selection_foreground: palette.text_strong,
        border_color: base.border_color,
        border_width: base.border_width,
        focus_ring_color,
        focus_ring_width,
    };

    assert_fill_foreground_locks(&style, palette);

    style
}

/// The `UIF-TOK-006` rule 4 shield for every foreground channel an
/// [`InputStyle`] carries.
///
/// Four pairs rather than one, because an input paints more than a value
/// over its own background: the placeholder and the trailing glyph are
/// separate foregrounds, and the rule reaches each of them. The fourth is
/// the selection, which is the pair this atom has always had to get right
/// — iced draws the selection as a filled rectangle under the value
/// colour, and that fill is [`accent_fill`](Palette::accent_fill), one of
/// the four locked solid fills.
///
/// No cell of §3.2 puts a foreground-only token on a locked fill today,
/// so every one of these four calls is a no-op on every state the atom
/// can reach. That is the point: the check is what keeps it that way
/// rather than the table merely happening to be correct.
fn assert_fill_foreground_locks(style: &InputStyle, palette: &Palette) {
    contrast::assert_fill_foreground_lock(style.background, style.text, palette);
    contrast::assert_fill_foreground_lock(style.background, style.placeholder, palette);
    contrast::assert_fill_foreground_lock(style.background, style.icon, palette);
    contrast::assert_fill_foreground_lock(
        style.selection_background,
        style.selection_foreground,
        palette,
    );
}

/// The rectangle the editable text occupies inside an input's bounds.
///
/// A function of the outer bounds and the kind **only**. There is no
/// parameter for the slot's content, and that absence is the zero-shift
/// invariant: an empty slot, an error glyph and a clear button all
/// reserve the same rectangle because they all reserve *this* rectangle.
#[must_use]
pub fn text_area_bounds(bounds: Rectangle, kind: InputKind) -> Rectangle {
    let left = HORIZONTAL_PADDING;
    let right = HORIZONTAL_PADDING + SLOT_GAP + kind.trailing_slot_width();

    Rectangle {
        x: bounds.x + left,
        y: bounds.y,
        width: (bounds.width - left - right).max(NO_EXTENT),
        height: bounds.height,
    }
}

/// The permanently reserved trailing slot inside an input's bounds.
///
/// The complement of [`text_area_bounds`]: same two inputs, so the pair
/// cannot disagree about how much room the slot takes.
#[must_use]
pub fn trailing_slot_bounds(bounds: Rectangle, kind: InputKind) -> Rectangle {
    let width = kind.trailing_slot_width();

    Rectangle {
        x: (bounds.x + bounds.width - HORIZONTAL_PADDING - width).max(bounds.x),
        y: bounds.y,
        width,
        height: bounds.height,
    }
}

/// The input's fill and border for a resolved style.
///
/// Returns `None` when neither would be painted. Every §3.2 cell paints a
/// background, so in practice this always returns a layer; the `Option`
/// keeps the shape identical to
/// [`button_widget::background_layer`](super::button_widget::background_layer)
/// so a caller that walks both atoms' layers does
/// not need a special case for inputs.
#[must_use]
pub fn background_layer(bounds: Rectangle, style: &InputStyle) -> Option<Layer> {
    if style.background.a == NO_FILL.a && style.border_width <= NO_STROKE_WIDTH {
        return None;
    }

    Some(Layer {
        bounds,
        fill: style.background,
        border: Border::default()
            .color(style.border_color)
            .width(style.border_width)
            .rounded(RADIUS),
    })
}

/// The focus ring drawn around an input's bounds, if one is due.
///
/// Only the invalid-and-focused cell produces a ring: everywhere else the
/// 2px accent border *is* the focus indicator, and drawing a ring on top
/// of it would double the stroke. As with a button, the ring lives
/// **outside** the control's own bounds, so the fill and the error border
/// are untouched and the layout never moves (`UIF-PRIN-003`).
///
/// The corner radius grows by the ring width so the ring's curve stays
/// concentric with the box it surrounds.
#[must_use]
pub fn focus_ring_layer(bounds: Rectangle, style: &InputStyle) -> Option<Layer> {
    if style.focus_ring_width <= NO_STROKE_WIDTH {
        return None;
    }

    Some(Layer {
        bounds: bounds.expand(style.focus_ring_width),
        fill: NO_FILL,
        border: Border::default()
            .color(style.focus_ring_color)
            .width(style.focus_ring_width)
            .rounded(RADIUS + style.focus_ring_width),
    })
}

/// Map iced's own editor [`EditorStatus`] onto this atom's
/// [`VisualState`].
///
/// Iced already distinguishes active / hovered / focused / disabled, so
/// the widget does not re-derive what the editor knows. What iced cannot
/// know is the caller's verdict, so `invalid` comes from outside, and
/// `pressed` is always `false` — §3.2 gives an input no pressed cell
/// (see the [module doc](self)).
///
/// A `Disabled` status on an enabled input, or the reverse, resolves to
/// whichever half [`state::resolve`] ranks higher: `enabled: false`
/// dominates, so the wrapper's own flag is the authority and iced's
/// status can only ever agree with it or be overruled.
#[must_use]
pub fn visual_from_status(status: EditorStatus, enabled: bool, invalid: bool) -> VisualState {
    let (hovered, focused) = match status {
        EditorStatus::Active => (false, false),
        EditorStatus::Hovered => (true, false),
        EditorStatus::Focused { is_hovered } => (is_hovered, true),
        EditorStatus::Disabled => (false, false),
    };

    state::resolve(InteractionInput {
        enabled,
        hovered,
        pressed: false,
        focused,
        invalid,
    })
}

/// Host sink for a bound field: turns one routed [`TextInputEmission`]
/// into the app's message, or returns `None` when that tier needs no
/// round-trip.
///
/// A single sum-returning closure rather than four separate callbacks,
/// for the reason the button gives: one call site in the widget, and a
/// host that wires three of the four `002a` callbacks still has to
/// *look* at the fourth rather than silently forget it.
///
/// [`TextInputEmission`]: super::text_input_controller::TextInputEmission
type Emit<'a, Message> = Box<dyn Fn(TextInputEmission) -> Option<Message> + 'a>;

/// A themed text entry, search field or dropdown trigger
/// (`UIF-ATOM-002`).
///
/// The widget owns the box, the chrome and the reserved trailing slot;
/// iced's `text_input` inside it owns the text. The inner editor is
/// styled transparent with no border so the two never paint over each
/// other, and the wrapper paints the [`Layer`]s
/// [`resolve_input_style`] describes before drawing it.
///
/// Keyboard focus is real: the inner editor registers with iced's focus
/// router under [`bridge::widget_id`](crate::bridge::widget_id) derived
/// from the same [`FocusId`] the host registers in
/// [`FocusState`](crate::focus::FocusState), so a bridge push and a click
/// address the same control. Disabling removes it from that address by
/// unfocusing it every pass, exactly as
/// [`Button`](super::button_widget::Button) does — a control the host has
/// disabled must not keep a focus the model can still land on.
///
/// Validation is the caller's. The widget never inspects the value to
/// decide it is invalid; [`invalid`](Self::invalid) is set from outside,
/// which is what keeps §3.2's Invalid row a *reported* state rather than
/// a second validator invented in the view layer.
pub struct TextInput<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    kind: InputKind,
    input: Element<'a, Option<Message>, Theme, Renderer>,
    on_input: Option<Rc<dyn Fn(String) -> Message + 'a>>,
    on_clear: Option<Message>,
    /// What a press-and-release in the trailing slot publishes when the
    /// slot carries a [`Toggle`](SlotContent::Toggle) — the reveal
    /// control of `UIF-ATOM-002c`.
    ///
    /// A separate route from `on_clear` because the two settle different
    /// things: a clear empties the field, a reveal changes no value at
    /// all and must not open an editing session.
    on_toggle: Option<Message>,
    focus_id: FocusId,
    widget_id: Id,
    placeholder: String,
    value: String,
    /// Whether the editor masks what it holds (`UIF-ATOM-002c`).
    ///
    /// Seeded from the kind — a [`Password`](InputKind::Password) field
    /// starts masked — and driven from outside by the host's reveal
    /// state. The atom does not own the policy; it draws what it is told.
    masked: bool,
    palette: Palette,
    width: Length,
    height: f32,
    enabled: bool,
    invalid: bool,
    slot: SlotContent,
    /// The legacy path's own read-only flag; a bound field answers from
    /// the model instead. See [`is_read_only`](Self::is_read_only).
    read_only: bool,
    /// The binding, when this field is driven by a [`TextInputModel`].
    model: Option<Controlled<'a, TextInputModel>>,
    /// Where routed tiers go. `None` on the legacy path, which has no
    /// tiers — it has `on_input`.
    emit: Option<Emit<'a, Message>>,
    /// Edits the inner editor published during this pass, captured by
    /// the wrapper's own shell instead of reaching the host.
    edits: Rc<RefCell<Vec<String>>>,
}

impl<'a, Message, Theme, Renderer> TextInput<'a, Message, Theme, Renderer>
where
    Message: Clone + 'a,
    Theme: text_input::Catalog + 'a,
    Renderer: text_core::Renderer + 'a,
    Theme::Class<'a>: From<text_input::StyleFn<'a, Theme>>,
{
    /// Build an input of `kind` bound to `focus_id`.
    ///
    /// `on_input` is the editor callback; the widget keeps a shared handle
    /// to it because [`disabled`](Self::disabled) and
    /// [`invalid`](Self::invalid) rebuild the inner editor — its style
    /// closure has to capture the current flags — and a `dyn Fn` cannot be
    /// cloned, so ownership is shared rather than transferred.
    #[must_use]
    pub fn new(
        kind: InputKind,
        placeholder: impl Into<String>,
        value: impl Into<String>,
        on_input: impl Fn(String) -> Message + 'a,
        focus_id: impl Into<FocusId>,
    ) -> Self {
        let mut widget = Self::base(kind, placeholder, value, focus_id);
        widget.on_input = Some(Rc::new(on_input));
        widget.sync();
        widget
    }

    /// Build a field whose state comes from `model` and whose edits leave
    /// as tiers through `emit` (`UIF-ATOM-002a`).
    ///
    /// The `Controlled<'_, TextInputModel>` is a shared borrow, not a
    /// copy: the host keeps the model and mutates it, and the field reads
    /// the current value every frame. That is the §3.0.4.1 rule — the
    /// model is the truth, so the widget must not have its own.
    ///
    /// There is no `on_input` here, and that is the point. A host that
    /// adopts the model cannot also keep receiving the raw string on a
    /// separate channel: the tiers exist precisely because the raw string
    /// is not the event. `emit` sees every tier the field produces —
    /// [`Changed`](TextInputEmission::Changed),
    /// [`Committed`](TextInputEmission::Committed),
    /// [`FocusCommitted`](TextInputEmission::FocusCommitted),
    /// [`Cleared`](TextInputEmission::Cleared),
    /// [`Cancelled`](TextInputEmission::Cancelled) and
    /// [`FocusChanged`](TextInputEmission::FocusChanged) — and returns
    /// `None` for the ones the app does not want a message for.
    ///
    /// [`on_clear`](Self::on_clear) is the legacy route and is ignored on
    /// this path: a bound field clears by routing
    /// [`clear`](TextInputController::clear), which is a tier, not a
    /// message the widget holds.
    #[must_use]
    pub fn controlled(
        kind: InputKind,
        model: Controlled<'a, TextInputModel>,
        focus_id: impl Into<FocusId>,
        emit: impl Fn(TextInputEmission) -> Option<Message> + 'a,
    ) -> Self {
        let mut widget = Self::base(
            kind,
            model.get().placeholder.clone(),
            model.get().value.clone(),
            focus_id,
        );
        widget.enabled = !model.get().disabled;
        widget.invalid = model.get().punishment().painted();
        widget.model = Some(model);
        widget.emit = Some(Box::new(emit));
        widget.sync();
        widget
    }

    /// The shared construction both paths start from: identity, chrome and
    /// geometry, with no callback and no binding.
    fn base(
        kind: InputKind,
        placeholder: impl Into<String>,
        value: impl Into<String>,
        focus_id: impl Into<FocusId>,
    ) -> Self {
        let focus_id = focus_id.into();
        let widget_id = crate::bridge::widget_id(&focus_id);

        Self {
            kind,
            input: Element::new(EmptyEditor),
            on_input: None,
            on_clear: None,
            on_toggle: None,
            focus_id,
            widget_id,
            placeholder: placeholder.into(),
            value: value.into(),
            masked: matches!(kind, InputKind::Password),
            palette: Palette::dark(),
            width: Length::Fill,
            height: HEIGHT_STANDARD,
            enabled: true,
            invalid: false,
            slot: SlotContent::Empty,
            read_only: false,
            model: None,
            emit: None,
            edits: Rc::new(RefCell::new(Vec::new())),
        }
    }

    /// Rebuild the inner editor and re-derive the slot from the current
    /// flags.
    ///
    /// Called after every flag-changing builder so the inner editor's
    /// style closure always captures the truth. The slot is recomputed
    /// here too, so `value` and the glyph shown for it cannot drift apart
    /// inside one frame.
    ///
    /// A bound field derives its slot from the model rather than from the
    /// widget's own copy of the value, which is the whole point of the
    /// binding: `slot_for` and `slot_content` agree today, and the model
    /// is where the disagreement would start if `clearable` ever made the
    /// glyph inert.
    fn sync(&mut self) {
        self.slot = match self.model {
            Some(model) => model.get().slot_for(self.kind),
            None => slot_content(self.kind, !self.value.is_empty(), self.invalid),
        };
        self.input = self.build_editor();
    }

    /// Construct the inner iced editor with this atom's chrome stripped off
    /// it.
    ///
    /// The editor's message type is `Option<Message>` because a bound
    /// field must be able to *not* answer it: iced publishes the instant
    /// a keystroke lands and there is no way to decline. `None` is the
    /// editor's own "nothing for the host", and the wrapper decides what
    /// happens instead.
    fn build_editor(&self) -> Element<'a, Option<Message>, Theme, Renderer> {
        let kind = self.kind;
        let palette = self.palette;
        let enabled = self.enabled;
        let invalid = match self.model {
            Some(model) => model.get().punishment().painted(),
            None => self.invalid,
        };
        let on_input = self.on_input.clone();
        let edits = self.edits.clone();
        let bound = self.model.is_some();

        let editor = text_input::TextInput::new(&self.placeholder, &self.value)
            .id(self.widget_id.clone())
            .width(Length::Fill)
            // Input values use the `type.body` rung (UIF-TOK-004).
            .size(typography::BODY.size)
            .line_height(typography::BODY.iced_line_height())
            .padding(Padding::ZERO)
            // `UIF-ATOM-002c`: masking is iced's own `Value::secure`, which
            // substitutes `•` (U+2022) for **each grapheme** rather than
            // collapsing the string. Two consequences the wrapper does not
            // get to choose: a masked run is *not* the same pixel width as
            // the revealed run it stands for, and iced couples the mask to
            // `input_method::Purpose::Secure`, so revealing a field drops
            // the platform's "do not autocorrect" hint with it.
            .secure(self.masked)
            .style(move |_theme: &Theme, status: EditorStatus| -> EditorStyle {
                let resolved = resolve_input_style(
                    kind,
                    visual_from_status(status, enabled, invalid),
                    &palette,
                );

                EditorStyle {
                    // The chrome belongs to the wrapper, not to the
                    // editor: the editor's bounds are the text area, and
                    // this atom's box is wider than that.
                    background: Background::Color(NO_FILL),
                    border: Border {
                        width: NO_STROKE_WIDTH,
                        color: NO_FILL,
                        radius: RADIUS.into(),
                    },
                    icon: resolved.icon,
                    placeholder: resolved.placeholder,
                    value: resolved.text,
                    selection: resolved.selection_background,
                }
            });

        // A field that cannot accept edits gets no callback at all, so
        // iced's own `Status::Disabled` is what the style closure sees.
        // The wrapper additionally refuses to forward events; both gates
        // are needed, because the first fixes the colours and the second
        // fixes the behaviour. Read-only takes the same *behavioural*
        // gate as disabled — no edit can reach the host either way —
        // while the wrapper keeps painting it as the enabled control it
        // still is.
        if !enabled || self.is_read_only() {
            return editor.into();
        }

        if bound {
            // A bound field never lets the raw string through. The edit
            // is recorded here and re-emitted as whichever tier the
            // controller says it is, so `None` is the honest answer to
            // "what should the host see right now".
            return editor
                .on_input(move |text| {
                    edits.borrow_mut().push(text);
                    None
                })
                .into();
        }

        if let Some(callback) = on_input {
            return editor.on_input(move |text| Some((callback)(text))).into();
        }

        editor.into()
    }

    /// Use the compact control height — dense toolbars (`size.control.sm`).
    #[must_use]
    pub fn compact(mut self) -> Self {
        self.height = HEIGHT_COMPACT;
        self
    }

    /// Use the standard control height — dialogs and forms
    /// (`size.control.md`). This is what [`new`](Self::new) builds.
    #[must_use]
    pub fn standard(mut self) -> Self {
        self.height = HEIGHT_STANDARD;
        self
    }

    /// Set the width the input asks its parent for.
    #[must_use]
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Make the input inert: the disabled skin, no message of any kind,
    /// and evicted from the focus address it registered at.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self.sync();
        self
    }

    /// Make the input live again.
    #[must_use]
    pub fn enabled(mut self) -> Self {
        self.enabled = true;
        self.sync();
        self
    }

    /// Report the value invalid, as decided by the caller's validation.
    ///
    /// This drives the Invalid row of §3.2 — the `surface.base` fill, the
    /// `status.danger` trailing glyph and the 2px danger border. The
    /// widget never derives invalidity from the value itself.
    #[must_use]
    pub fn invalid(mut self) -> Self {
        self.invalid = true;
        self.sync();
        self
    }

    /// Clear a previously reported invalid verdict.
    #[must_use]
    pub fn valid(mut self) -> Self {
        self.invalid = false;
        self.sync();
        self
    }

    /// Make the field readable and selectable but not editable.
    ///
    /// This is *not* [`disabled`](Self::disabled), and the difference is
    /// the point: a disabled field is one the host has taken away, and it
    /// paints the Disabled cell. A read-only field is one the user can
    /// still look at, select and copy from, and it paints the same Rest,
    /// Hovered or Invalid cell it would paint if it were live.
    ///
    /// What changes is that no edit can reach the host. Iced's editor
    /// gets no `on_input` at all, which is the only gate the editor
    /// actually respects — it has no `read_only` of its own, and every
    /// edit path returns early when the callback is absent.
    ///
    /// A bound field answers from [`TextInputModel::read_only`] instead;
    /// this builder is the legacy path's equivalent.
    /// [`TextInputModel::read_only`]: super::text_input_model::TextInputModel::read_only
    #[must_use]
    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self.sync();
        self
    }

    /// The message published when the actionable trailing glyph — the
    /// search field's clear `×` — is pressed and released inside the
    /// slot.
    #[must_use]
    pub fn on_clear(mut self, message: Message) -> Self {
        self.on_clear = Some(message);
        self
    }

    /// The message published when the trailing **toggle** — the reveal
    /// control of `UIF-ATOM-002c` — is pressed and released inside the
    /// slot.
    ///
    /// Only consulted while [`slot`](Self::slot) is a
    /// [`Toggle`](SlotContent::Toggle); a field whose slot is a clear
    /// button keeps publishing [`on_clear`](Self::on_clear). The two are
    /// separate messages because the host does separate things with them,
    /// and folding them together would make "the user cleared the field"
    /// and "the user looked at the field" the same event.
    #[must_use]
    pub fn on_reveal(mut self, message: Message) -> Self {
        self.on_toggle = Some(message);
        self
    }

    /// Mask the field's contents, or show them (`UIF-ATOM-002c`).
    ///
    /// The atom holds no reveal policy: it masks exactly what it is told
    /// to. What it does guarantee is that this changes **no geometry** —
    /// the slot reservation, the text area and the row height are all
    /// functions of `(bounds, kind)`, never of the mask.
    #[must_use]
    pub fn masked(mut self, masked: bool) -> Self {
        self.masked = masked;
        self
    }

    /// Whether the editor is currently masking its contents.
    #[must_use]
    pub const fn is_masked(&self) -> bool {
        self.masked
    }

    /// The message a trailing toggle publishes, if one is wired.
    #[must_use]
    pub const fn reveal_message(&self) -> Option<&Message> {
        self.on_toggle.as_ref()
    }

    /// Paint from a palette other than [`Palette::dark`].
    #[must_use]
    pub fn palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self.sync();
        self
    }

    /// The kind whose slot rules this input follows.
    #[must_use]
    pub fn kind(&self) -> InputKind {
        self.kind
    }

    /// The framework-level identity this input answers to.
    #[must_use]
    pub fn focus_id(&self) -> &FocusId {
        &self.focus_id
    }

    /// The iced [`Id`] this input registers with the focus router.
    #[must_use]
    pub fn widget_id(&self) -> &Id {
        &self.widget_id
    }

    /// Whether this input accepts interaction.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Whether the caller has reported this input's value invalid.
    #[must_use]
    pub fn is_invalid(&self) -> bool {
        self.invalid
    }

    /// The binding this field reads, if it has one.
    #[must_use]
    pub fn model(&self) -> Option<Controlled<'a, TextInputModel>> {
        self.model
    }

    /// Whether this field is driven by a [`TextInputModel`] rather than
    /// by the legacy `on_input` callback.
    #[must_use]
    pub fn is_controlled(&self) -> bool {
        self.model.is_some()
    }

    /// What the reserved trailing slot carries right now.
    #[must_use]
    pub fn slot(&self) -> SlotContent {
        self.slot
    }
}

/// The tier-routing half of the widget, in its own impl block on purpose.
///
/// Neither method builds the inner editor, so neither needs
/// `Theme::Class<'a>: From<text_input::StyleFn<'a, Theme>>` — and the
/// `'a` in that bound is exactly what makes it impossible to satisfy from
/// [`Widget::update`], which is instantiated at an anonymous lifetime.
/// Keeping the routing in the *minimal* block, the same reason
/// [`visual`](Self::visual) lives in one, is what lets the widget trait
/// call it at all.
impl<Message, Theme, Renderer> TextInput<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: text_core::Renderer,
{
    /// A press-and-release in the trailing slot, dispatched to whichever
    /// action the slot's content owns.
    ///
    /// A [`Toggle`](SlotContent::Toggle) publishes its message and stops:
    /// revealing a secret settles no value, opens no editing session and
    /// routes no tier, so routing it through the clear path would tell
    /// the host the field was emptied when all that happened is that the
    /// host was asked to show it. Everything else keeps the clear
    /// contract it has always had.
    fn release_slot(&self, tree: &mut Tree, shell: &mut Shell<'_, Message>) {
        if self.slot.is_toggle() {
            if let Some(message) = self.on_toggle.clone() {
                shell.publish(message);
            }
            return;
        }

        self.release_clear(tree, shell);
    }

    /// The trailing `×` released inside the slot.
    ///
    /// A bound field routes [`TextInputController::clear`] — a tier, like
    /// everything else that leaves a bound control. A legacy field
    /// publishes the [`on_clear`](Self::on_clear) message it was given.
    /// The two never both fire: a field is on one path or the other.
    ///
    /// [`TextInputController::clear`]: super::text_input_controller::TextInputController::clear
    fn release_clear(&self, tree: &mut Tree, shell: &mut Shell<'_, Message>) {
        let Some(model) = self.model else {
            if let Some(message) = self.on_clear.clone() {
                shell.publish(message);
            }
            return;
        };

        // The slot is the wrapper's own hit target, so this path never
        // passes through `route_tiers` — which is where the model is
        // normally observed. Observe it here: a clear of a value nobody
        // has been told about yet is a clear of nothing, and the `x`
        // would silently do nothing on the very field that shows it.
        let controller = controller_mut(tree);
        controller.observe(model.get());

        route(&self.emit, controller.clear(), shell);
    }

    /// Fold this pass's raw signals into tiers and hand them to the host.
    ///
    /// The order is the order the events actually happen in: the model is
    /// observed, edits are recorded, the gesture key settles whatever is
    /// open, the focus edge closes the session, and the surviving live
    /// tick is delivered last. A settled tier discards the pending tick
    /// as it goes, so a host handed `Committed` is never woken afterwards
    /// with a `Live` describing the same string.
    ///
    /// Only ever called for a bound field; the `None` model case returns
    /// rather than panicking so the function stays honest about its
    /// preconditions.
    ///
    /// `holds_keyboard` is the focus state captured *before* the event
    /// was forwarded, not the one read here. The two differ exactly when
    /// the event moved the keyboard — and `Escape` is one of them, which
    /// is why the caller has to hand it over rather than recompute it.
    fn route_tiers(
        &self,
        tree: &mut Tree,
        event: &Event,
        holds_keyboard: bool,
        shell: &mut Shell<'_, Message>,
    ) {
        let Some(model) = self.model else {
            return;
        };

        let focused = editor_focused::<Renderer>(tree) && !self.is_read_only();
        let controller = controller_mut(tree);

        controller.observe(model.get());

        // Every edit the editor made since the last pass, folded into the
        // coalescer. However many keystrokes landed, at most one live tick
        // comes out of `take_live` below.
        for edit in self.edits.borrow_mut().drain(..) {
            controller.note_edit(edit);
        }

        // Gesture keys belong to the field that holds the keyboard. The
        // wrapper is handed every event while its subtree is on screen;
        // acting on `Escape` without this gate would revert or clear a
        // field the user was never in.
        if holds_keyboard {
            if is_named_key(event, Named::Enter) {
                route(&self.emit, controller.commit(), shell);
            } else if is_named_key(event, Named::Escape) {
                // Revert if a session is open. Otherwise clear — but only
                // where the model says the clear affordance can act,
                // which is the same predicate that makes the `×`
                // actionable. A plain text entry is never `Escape`-cleared,
                // because a key that silently deletes what the user typed
                // and never lets them undo it is worse than no key at all.
                if controller.session_open() {
                    route(&self.emit, controller.revert(), shell);
                } else if model.get().clear_affordance(self.kind) {
                    route(&self.emit, controller.clear(), shell);
                }
            }
        }

        // The focus edge is recorded last so the host sees whatever the
        // gesture settled before it sees the session boundary move.
        for edge in controller.note_focus(focused) {
            route(&self.emit, Some(edge), shell);
        }

        if let Some(value) = controller.take_live() {
            route(
                &self.emit,
                Some(TextInputEmission::Changed(Live::new(value))),
                shell,
            );
        }
    }
}

/// The state read-out, in its own impl block on purpose.
///
/// The block above carries `Theme::Class<'a>: From<text_input::StyleFn>`
/// because building the inner editor needs it. [`visual`](Self::visual) needs nothing
/// of the kind — it is [`state::resolve`] over the widget's own flags and
/// the editor's focus — so pinning it to the *minimal* bound is what lets
/// [`Widget::draw`] call it from an impl block that has no `Theme` bound
/// at all. Folding the two together would push a styling requirement into
/// the read path for no reason.
impl<Message, Theme, Renderer> TextInput<'_, Message, Theme, Renderer>
where
    Renderer: text_core::Renderer,
{
    /// Whether the field can be read but not edited.
    ///
    /// A bound field answers from the model, so the model stays the truth
    /// even if both a builder and a model flag are somehow in play; the
    /// legacy [`read_only`](Self::read_only) builder is the only way a
    /// field with no binding can report this.
    ///
    /// Deliberately in the minimal block: the read path needs nothing of
    /// styling, and both [`visual`](Self::visual) and the tier router call
    /// it from places that cannot name `Theme::Class<'a>`.
    #[must_use]
    pub fn is_read_only(&self) -> bool {
        self.read_only || self.model.is_some_and(|model| model.get().read_only)
    }

    /// The resolved state this input paints, given its laid-out bounds,
    /// what the router is doing and where the cursor is.
    ///
    /// Focus is read from the inner editor's own state rather than
    /// invented here, so the chrome cannot claim focus the editor does not
    /// have — and, with [`operate`](Widget::operate)'s eviction, cannot
    /// claim it for a disabled control either.
    ///
    /// A read-only field never claims it either. Iced still lets the
    /// router focus the inner editor, but a field that cannot take a
    /// caret must not paint the cell that says it can, so the focus half
    /// is dropped for it. The rest of the cell is untouched, which is
    /// what keeps read-only a *different* state from disabled rather
    /// than a synonym for it.
    ///
    /// `bounds` is the wrapper's own box rather than the editor's, which
    /// is why it is a parameter: the hover test has to cover the reserved
    /// slot too, and the slot is outside the child.
    #[must_use]
    pub fn visual(&self, tree: &Tree, bounds: Rectangle, cursor: mouse::Cursor) -> VisualState {
        let invalid = match self.model {
            Some(model) => model.get().punishment().painted(),
            None => self.invalid,
        };
        state::resolve(InteractionInput {
            enabled: self.enabled,
            hovered: cursor.is_over(bounds),
            pressed: false,
            focused: editor_focused::<Renderer>(tree) && !self.is_read_only(),
            invalid,
        })
    }
}

/// Whether the inner editor currently holds keyboard focus.
///
/// # Panics
/// Panics if the child's state is not the editor's, which cannot happen:
/// [`Widget::children`] builds the child from the editor element, and
/// iced's `text_input` gives that tree a `State<Paragraph>`.
fn editor_focused<Renderer: text_core::Renderer>(tree: &Tree) -> bool {
    tree.children.first().is_some_and(|child| {
        child
            .state
            .downcast_ref::<EditorState<Renderer::Paragraph>>()
            .is_focused()
    })
}

/// The [`TextInputController`] carried by a field's tree node.
///
/// Reached through the [`Ephemeral`] wrapper the tag declares, so there is
/// one obvious way to get at the ephemeral half and no silent `downcast`
/// of a bare controller that would panic against a tree built under some
/// other tag.
///
/// # Panics
/// Never in practice: [`Widget::state`] always builds the node this
/// function reads, and no other code path builds a text input's tree.
fn controller_mut(tree: &mut Tree) -> &mut TextInputController {
    tree.state
        .downcast_mut::<Ephemeral<TextInputController>>()
        .get_mut()
}

/// Send one tier of the editing engine's output to the host's sink.
///
/// A no-op when the field has no sink (it was built through
/// [`new`](TextInput::new), which publishes `on_input` directly) or when
/// the engine produced nothing. Keeping both checks here rather than at
/// each of the five call sites is what stops one of them forgetting.
fn route<Message>(
    emit: &Option<Emit<'_, Message>>,
    emission: Option<TextInputEmission>,
    shell: &mut Shell<'_, Message>,
) {
    let (Some(emit), Some(emission)) = (emit, emission) else {
        return;
    };

    if let Some(message) = (emit)(emission) {
        shell.publish(message);
    }
}

/// Whether `event` is a fresh press of the named key `name`.
///
/// `Press` only, never `Repeat`: a held `Enter` would otherwise settle
/// the same value once per autorepeat tick, and a held `Escape` would
/// settle a session that closed on the first one.
fn is_named_key(event: &Event, name: Named) -> bool {
    matches!(
        event,
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: Key::Named(key),
            repeat: false,
            ..
        }) if *key == name
    )
}

/// Copy the side effects a nested shell picked up onto the host's shell.
///
/// [`Shell::merge`] cannot be used here: it moves *every* message it
/// carries, mapping each one, and the whole point of the nested shell is
/// that some messages are dropped rather than forwarded. Everything that
/// is not a message still has to travel — event capture, layout and
/// widget invalidation, the redraw request, the IME strategy — or
/// suppressing a message would also suppress the frame the editor asked
/// for.
fn carry_over<Message>(from: &Shell<'_, Option<Message>>, to: &mut Shell<'_, Message>) {
    if from.is_event_captured() {
        to.capture_event();
    }

    if from.is_layout_invalid() {
        to.invalidate_layout();
    }

    if from.are_widgets_invalid() {
        to.invalidate_widgets();
    }

    to.request_redraw_at(from.redraw_request());
    to.request_input_method(from.input_method());
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for TextInput<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Theme: text_input::Catalog,
    Renderer: text_core::Renderer,
{
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.input)]
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Ephemeral<TextInputController>>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Ephemeral::new(TextInputController::new()))
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.input));
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, Length::Fixed(self.height))
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let size = limits.resolve(self.width, Length::Fixed(self.height), Size::ZERO);
        let outer = Rectangle::with_size(size);
        let area = text_area_bounds(outer, self.kind);

        let editor = self
            .input
            .as_widget_mut()
            .layout(
                &mut tree.children[0],
                renderer,
                &layout::Limits::NONE.width(area.width),
            )
            .move_to(area.position())
            .align(
                Alignment::Start,
                Alignment::Center,
                Size::new(area.width, area.height),
            );

        layout::Node::with_children(size, vec![editor])
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let editor_layout = layout
            .children()
            .next()
            .expect("a text input lays out exactly one editor node");

        if !self.enabled {
            // The inner editor registers itself with the router whether
            // or not it has a callback, so disabling it is not enough —
            // it has to be evicted from the address, the same way
            // `Focusable` and `Button` evict themselves. Otherwise a
            // field focused while enabled keeps a focus nothing can see
            // or leave.
            //
            // Before the traverse, not after. A read-only operation like
            // `is_focused` records what it sees as the traversal passes
            // the editor; evicting afterwards would let one pass report a
            // focus the same pass had already taken away.
            if let Some(child) = tree.children.first_mut() {
                let editor = child
                    .state
                    .downcast_mut::<EditorState<Renderer::Paragraph>>();

                <EditorState<Renderer::Paragraph> as operation::Focusable>::unfocus(editor);
            }
        }

        operation.traverse(&mut |operation| {
            self.input.as_widget_mut().operate(
                &mut tree.children[0],
                editor_layout,
                renderer,
                operation,
            );
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        if !self.enabled {
            // Nothing is forwarded: a disabled input emits no message,
            // edits nothing and captures nothing. The event is left for
            // anything else that wants it.
            //
            // A bound field also loses its editing session here. A
            // snapshot carried across the stretch in which the control
            // could not act would let a later `Escape` restore a value
            // from before the field stopped existing, and neither a
            // commit nor a revert is owed for a control that was turned
            // off mid-keystroke.
            if self.model.is_some() {
                controller_mut(tree).reset();
            }
            return;
        }

        let slot = trailing_slot_bounds(layout.bounds(), self.kind);

        if self.slot.captures_press() && cursor.is_over(slot) {
            // The clear affordance is the wrapper's own hit target, so
            // pressing it never falls through to the editor and moves the
            // caret or steals focus. Press and release both belong to the
            // slot: a press that slides off and releases elsewhere fires
            // nothing, the same convention a button follows.
            match event {
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                    shell.capture_event();
                }
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                    self.release_slot(tree, shell);
                    shell.capture_event();
                }
                _ => {}
            }
            return;
        }

        let editor_layout = layout
            .children()
            .next()
            .expect("a text input lays out exactly one editor node");

        // Whether this field held the keyboard *before* the event ran,
        // which is the only honest moment to ask. Iced's own `Escape`
        // handler blurs the inner editor, so a gate read after the
        // editor has had its turn would find no keyboard to attribute the
        // gesture to and drop it.
        let holds_keyboard = editor_focused::<Renderer>(tree) && !self.is_read_only();

        // A bound field takes `Escape` for itself and never lets the
        // editor see it, for the same reason. `Cancel` restores the text
        // you were in the middle of; it would be a poor joke to restore
        // it and throw the keyboard away with it, and the spec's own
        // two-`Escape` patterns (`002d`) assume the field is still there
        // to take the second one.
        let swallow_escape =
            self.model.is_some() && holds_keyboard && is_named_key(event, Named::Escape);

        // The inner editor publishes the moment a keystroke lands, and
        // `Shell::publish` cannot be declined. So the editor is driven
        // through a shell this widget owns, over a buffer of
        // `Option<Message>`: `Some` is a legacy message to republish,
        // `None` is an edit that belongs to the tiers instead.
        let mut buffer: Vec<Option<Message>> = Vec::new();

        if !swallow_escape {
            let mut inner = Shell::new(&mut buffer);

            self.input.as_widget_mut().update(
                &mut tree.children[0],
                event,
                editor_layout,
                cursor,
                renderer,
                clipboard,
                &mut inner,
                viewport,
            );

            carry_over(&inner, shell);
        }

        if self.model.is_none() {
            // Legacy: what the editor said, one for one, in order.
            for message in buffer.into_iter().flatten() {
                shell.publish(message);
            }
            return;
        }

        if swallow_escape {
            shell.capture_event();
        }

        self.route_tiers(tree, event, holds_keyboard, shell);
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if !self.enabled {
            return mouse::Interaction::None;
        }

        let bounds = layout.bounds();

        if self.slot.captures_press() && cursor.is_over(trailing_slot_bounds(bounds, self.kind)) {
            mouse::Interaction::Pointer
        } else if cursor.is_over(text_area_bounds(bounds, self.kind)) {
            mouse::Interaction::Text
        } else {
            mouse::Interaction::None
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let paint =
            resolve_input_style(self.kind, self.visual(tree, bounds, cursor), &self.palette);

        for layer in [
            background_layer(bounds, &paint),
            focus_ring_layer(bounds, &paint),
        ]
        .into_iter()
        .flatten()
        {
            renderer.fill_quad(layer.quad(), Background::Color(layer.fill));
        }

        let editor_layout = layout
            .children()
            .next()
            .expect("a text input lays out exactly one editor node");

        self.input.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            &renderer::Style {
                text_color: paint.text,
            },
            editor_layout,
            cursor,
            viewport,
        );

        if let Some(glyph) = self.slot.glyph() {
            draw_slot_glyph(
                renderer,
                glyph,
                trailing_slot_bounds(bounds, self.kind),
                paint.icon,
                viewport,
            );
        }
    }
}

/// Draw the trailing slot's glyph, centred in the reserved rectangle.
///
/// The glyph is painted by the wrapper rather than by a child element so
/// its colour comes straight from the resolved style — a child `text`
/// element would have to be rebuilt every frame to track hover and focus,
/// and rebuilding is exactly what the zero-shift invariant is designed to
/// make irrelevant.
fn draw_slot_glyph<Renderer: text_core::Renderer>(
    renderer: &mut Renderer,
    glyph: char,
    slot: Rectangle,
    color: Color,
    viewport: &Rectangle,
) {
    let mut buffer = [0_u8; 4];

    renderer.fill_text(
        text_core::Text {
            content: glyph.encode_utf8(&mut buffer).to_owned(),
            bounds: slot.size(),
            size: Pixels(SLOT_GLYPH_SIZE),
            // A line box as tall as the slot, so the glyph sits on the
            // slot's own centre line rather than at its top.
            line_height: LineHeight::Absolute(Pixels(slot.height)),
            font: renderer.default_font(),
            align_x: text_core::Alignment::Center,
            align_y: alignment::Vertical::Center,
            shaping: Shaping::Basic,
            wrapping: Wrapping::None,
        },
        slot.position(),
        color,
        *viewport,
    );
}

/// Placeholder editor used only for the field's initial value before
/// [`TextInput::new`] replaces it.
///
/// `Element` cannot be built from nothing, and the real editor needs the
/// constructor's own fields, so the field starts with a zero-sized widget
/// that is overwritten by [`TextInput::sync`] before anything sees it.
struct EmptyEditor;

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for EmptyEditor
where
    Renderer: renderer::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(NO_EXTENT), Length::Fixed(NO_EXTENT))
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        _limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(Size::ZERO)
    }

    fn draw(
        &self,
        _tree: &Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }
}

impl<'a, Message, Theme, Renderer> From<TextInput<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a + text_input::Catalog,
    Renderer: 'a + text_core::Renderer,
{
    fn from(
        input: TextInput<'a, Message, Theme, Renderer>,
    ) -> Element<'a, Message, Theme, Renderer> {
        Element::new(input)
    }
}

#[cfg(test)]
#[path = "text_input_tests.rs"]
mod text_input_tests;
