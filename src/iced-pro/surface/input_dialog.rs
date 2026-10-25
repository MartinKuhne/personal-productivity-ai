//! The input dialog host — the §4.5 card anatomy, the permanently reserved
//! validation slot, cursor-proximal placement and destructive parity
//! (`UIF-INP-001`, `UIF-INP-002`, `UIF-INP-003`, `UIF-INP-004`).
//!
//! # What the host is
//!
//! §4.5 describes five small cards — `Create Directory`, `New Document`,
//! `Rename`, `Move` and `Delete Confirmation` — that share one box model
//! and four invariants. Like [`modal`](super::modal), this module is **not**
//! a widget. It is the frame: the thing that decides, for every keystroke,
//! pointer press and submitted value, whether the dialog closes, refuses,
//! reveals an error, or swallows the event — from geometry and declared
//! state, with no renderer in sight.
//!
//! The four invariants are enforced by *what the code can reach*, not by
//! rules a caller is asked to remember:
//!
//! * **`UIF-INP-001`** is [`anchor::solve`] with
//!   [`Anchor::CursorProximal`]. The host
//!   does not own a clamping implementation and could not write one if it
//!   wanted to; the card's origin comes out of the shared solver, which is
//!   the same code path the menus use.
//! * **`UIF-INP-002`** is the shape of
//!   [`on_pointer`](InputDialogHost::on_pointer): the card rectangle is
//!   tested **first**, and no arm of the `match` behind that test can
//!   reach the scrim. A click on a disabled `Ok` cannot leak upward
//!   through a boundary it was never outside of.
//! * **`UIF-INP-003`** is the layout function's *signature*.
//!   [`InputLayout::solve`] takes the card, the control kind and the two
//!   button widths — it has **no parameter for whether an error is
//!   showing**, so there is no channel by which an error could move the
//!   footer. The slot is not "kept the same height"; it is a constant that
//!   nothing is allowed to vary.
//! * **`UIF-INP-004`** is that every dialog this host opens — destructive
//!   or not — is the same [`InputDialogHost`], answers the same
//!   [`InputAnswer`], dismisses through the same private
//!   `attempt_dismiss`, and lays out with the same
//!   [`InputLayout::solve`]. A destructive card differs in exactly two
//!   declared values: [`ButtonIntent::Danger`] on the primary, and which
//!   [`InputControl`] the middle row holds.
//!
//! # The zero-shift guarantee
//!
//! §4.5.1 prints the box model, and the whole card height is a function
//! of one thing: which control the middle row holds.
//!
//! ```text
//! padding 16 ┌──────────────────────────────────────────────┐
//!            │ Title Row                    24  TITLE_SM.lh │
//!     8 gap  ├──────────────────────────────────────────────┤
//!            │ Context / Target Label       20  BODY.lh      │
//!     8 gap  ├──────────────────────────────────────────────┤
//!            │ Primary Input Control       H   see below     │
//!     8 gap  ├──────────────────────────────────────────────┤
//!            │ Validation Message Slot    20  BODY.lh       │
//!     8 gap  ├──────────────────────────────────────────────┤
//!            │ Footer Action Row          32  CONTROL_MD    │
//! padding 16 └──────────────────────────────────────────────┘
//! ```
//!
//! `H` is 32 for a text entry, `clamp(rows × 28, 28, 200)` for a folder
//! list and 0 for a bare confirmation. A text-entry card is therefore
//! 192px tall and stays 192px tall whether the validation slot is empty,
//! showing "A name is required." or showing anything else.
//!
//! The guarantee is structural rather than maintained. `InputLayout` is
//! computed once, at [`open`](InputDialogHost::open), from
//! `(card, control, button widths)`. Nothing in the host recomputes it
//! afterwards: revealing an error writes
//! [`revealed`](InputDialogHost::revealed), and clearing one writes it
//! back to `None`, and neither value reaches the layout.
//!
//! # Enter, and why focus stays in the text box
//!
//! `UIF-INP-003` asks for three things at once on an invalid `Enter`:
//! reveal the specific error, keep the dialog open, keep focus inside the
//! text box. [`submit`](InputDialogHost::submit) does exactly that and
//! nothing else — it writes the revealed message, re-asserts focus on the
//! entry, and returns without tearing anything down. There is no close
//! path in the invalid branch to *not* take.
//!
//! Note that clicking a disabled `Ok` is **not** the same interaction.
//! `UIF-INP-002` says a click on a disabled `Ok` is absorbed, and
//! `UIF-INP-003` says `Enter` reveals the error. Both hold, because they
//! are different entry points: the click is answered by the absorption
//! boundary and reports nothing; the key is answered by the submit path
//! and reports the message.
//!
//! # Escape, and the menu above
//!
//! Every user-initiated close goes through one private function,
//! `attempt_dismiss`, which is the whole of the dialog's cancel
//! behaviour. Through the z-order chain the dialog answers
//! [`UnwindAnswer::Dismissed`] — an input dialog has no unsaved-work
//! concept, so it is always clean enough to close — and because
//! [`escape_rank`](ZBand::escape_rank) for [`ZBand::Menu`] is `3` against
//! this band's `2`, a menu
//! open above the dialog is asked **first** and closes before the dialog
//! is touched (see **G-13** in `doc/planning/iced-pro-tasks.md`). The
//! host does not know menus exist; the registry's walk order does.
//!
//! A scrim click cancels for the same reason: §5 gives Band 3 a
//! full-window scrim that "dismisses only if clean/non-dirty", and an
//! input dialog is always clean.
//!
//! # Validation is injected
//!
//! The host never touches a filesystem and never decides on its own what a
//! legal name is. [`ValidationRule`] carries the §4.5.2 rule set —
//! non-empty, legal basename characters, distinct from the current name
//! for `Rename` — as data, and [`ValidationRule::check`] is a pure
//! function returning a [`Verdict`]. The verdict drives three things and
//! nothing else: whether the `Ok` member is enabled in the focus model,
//! what `Enter` does, and what the reserved slot says when it is
//! revealed.
//!
//! Unit tests live in the sibling `input_dialog_tests.rs` sidecar.

use crate::binding::ActionPending;
use crate::binding::Cancel;
use crate::binding::CancelRequest;
use crate::binding::Commit;
use crate::binding::Ephemeral;
use crate::focus::FocusId;
use crate::focus::FocusMember;
use crate::focus::FocusState;
use crate::focus::GroupId;
use crate::geometry::anchor;
use crate::geometry::anchor::Anchor;
use crate::geometry::anchor::Placement;
use crate::surface::dialog_button::DialogButtonController;
use crate::surface::dialog_button::DialogButtonRole;
use crate::surface::zorder::DeclineReason;
use crate::surface::zorder::EscapeOutcome;
use crate::surface::zorder::KeyClass;
use crate::surface::zorder::KeyDisposition;
use crate::surface::zorder::SurfaceId;
use crate::surface::zorder::SurfaceKind;
use crate::surface::zorder::SurfaceRegistry;
use crate::surface::zorder::UnwindAnswer;
use crate::surface::zorder::ZBand;
use crate::token::shape;
use crate::token::size;
use crate::token::space;
use crate::token::typography;

use iced::Point;
use iced::Rectangle;
use iced::Size;

// ---------------------------------------------------------------------------
// Box-model constants (§4.5.1)
// ---------------------------------------------------------------------------

/// §4.5.1 — the card is `320px` wide and never any wider.
///
/// The token scale carries control *heights*, not card widths, so this is
/// a named constant rather than a token read. It is the only place the
/// figure appears, and [`InputLayout::card_size`] is the only thing
/// allowed to use it.
pub const CARD_WIDTH: f32 = 320.0;

/// §4.5.1 — the card's inner padding (`space.lg`).
pub const CARD_PADDING: f32 = space::LG;

/// §4.5.1 — the gap between every pair of adjacent rows (`space.sm`).
pub const ROW_GAP: f32 = space::SM;

/// §4.5.1 — the title row is `24px`, which is the line height of
/// [`typography::TITLE_SM`], the rung §4.5.1 renders it on.
pub const TITLE_ROW_HEIGHT: f32 = typography::TITLE_SM.line_height;

/// §4.5.1 — the context / target label row is `20px`, the line height of
/// [`typography::BODY`].
pub const CONTEXT_ROW_HEIGHT: f32 = typography::BODY.line_height;

/// §4.5.1 — a text entry is `32px` (`size.control.md`).
pub const TEXT_ENTRY_HEIGHT: f32 = size::CONTROL_MD;

/// §4.5.1 — one destination folder row is `28px` (`size.control.sm`).
pub const FOLDER_ROW_HEIGHT: f32 = size::CONTROL_SM;

/// §4.5.1 — the tallest the destination folder list gets before it
/// scrolls inside the card.
pub const FOLDER_LIST_MAX_HEIGHT: f32 = 200.0;

/// §4.5.1 / `UIF-INP-003` — the validation message slot is `20px` and
/// **permanently reserved**.
///
/// The slot's text is `type.caption`, but the reserved height is the
/// `20px` §4.5.1 states, which is [`typography::BODY`]’s line height.
/// Reading the reservation from a token rather than a literal keeps the
/// zero-shift invariant traceable to the same scale everything else is.
pub const VALIDATION_SLOT_HEIGHT: f32 = typography::BODY.line_height;

/// §4.5.1 — the footer action row is `32px` (`size.control.md`).
pub const FOOTER_HEIGHT: f32 = size::CONTROL_MD;

/// The floor a footer button's width before its own label is measured.
///
/// §4.5.1 fixes the row's height and its alignment but leaves button
/// widths to the label, which only a renderer knows. This is the value
/// used until one is supplied, so the host can lay the footer out before
/// the first frame.
pub const BUTTON_MIN_WIDTH: f32 = 64.0;

/// §4.5.1 — a bare confirmation card has no primary input control, so
/// the middle row collapses.
///
/// §4.5.1 names exactly two controls, and `Delete Confirmation` has
/// neither: its whole message lives in the context row. The row collapses
/// rather than being skipped, so the box model stays one function and the
/// reserved validation slot is unaffected.
pub const CONFIRMATION_CONTROL_HEIGHT: f32 = 0.0;

/// Characters a filesystem basename may not contain.
///
/// The union of the Windows-reserved set and the two path separators. POSIX
/// allows most of these, but a name written here is expected to survive a
/// round trip through Windows, so the stricter set is the rule.
///
/// Aliased to [`FILENAME_ILLEGAL_CHARS`](crate::validation::disallowed::FILENAME_ILLEGAL_CHARS)
/// rather than spelled out again: this validator and `UIF-PRIN-009`'s
/// instant-punish class must refuse exactly the same characters, and two
/// literals that happen to agree today are two literals that can disagree
/// tomorrow.
const ILLEGAL_BASENAME_CHARS: &str = crate::validation::disallowed::FILENAME_ILLEGAL_CHARS;

/// Validation message codes (`NFR-004`).
const CODE_EMPTY: &str = "INP-1001";
/// Validation message code: leading or trailing whitespace.
const CODE_EDGESPACING: &str = "INP-1002";
/// Validation message code: control characters.
const CODE_CONTROL: &str = "INP-1003";
/// Validation message code: illegal basename characters.
const CODE_ILLEGAL_CHAR: &str = "INP-1004";
/// Validation message code: rename to the same name.
const CODE_UNCHANGED: &str = "INP-1005";
/// Validation message code: nothing selected to confirm against.
const CODE_NOTHING_TO_CONFIRM: &str = "INP-1006";
/// Validation message code: no destination chosen.
const CODE_NO_DESTINATION: &str = "INP-1007";

// ---------------------------------------------------------------------------
// Selection (UIF-INP-004)
// ---------------------------------------------------------------------------

/// The caret range a text-entry dialog selects when it opens
/// (`UIF-INP-004`).
///
/// Both indices are counted in **`chars()`**, not bytes. A framework that
/// spoke byte offsets would have to say what happens when a multi-byte
/// character lands inside the selection; in `chars` the question does not
/// arise.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    /// Index of the first selected character.
    pub start: usize,
    /// Index one past the last selected character.
    pub end: usize,
}

impl Selection {
    /// A collapsed selection at the very start of the field.
    pub const EMPTY: Self = Self { start: 0, end: 0 };

    /// How many characters the selection covers.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Whether nothing is selected.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The selected slice of `value`, or `""` when the range is empty,
    /// inverted, or past the end of the value.
    #[must_use]
    pub fn slice<'a>(&self, value: &'a str) -> &'a str {
        if self.is_empty() || self.end < self.start {
            return "";
        }
        let start = char_boundary(value, self.start);
        let end = char_boundary(value, self.end);
        if start >= end {
            return "";
        }
        &value[start..end]
    }

    /// Select the whole value.
    #[must_use]
    pub fn all(value: &str) -> Self {
        let len = value.chars().count();
        Self { start: 0, end: len }
    }

    /// Select the filename **stem** — everything before the final
    /// extension dot (`UIF-INP-004`).
    ///
    /// The rules, in order:
    ///
    /// * an empty value selects nothing;
    /// * a value with no dot selects the whole thing, because there is no
    ///   stem/extension split to make;
    /// * a **dotfile** — a value whose first dot is at index `0`, like
    ///   `.gitignore` — selects the whole name. Selecting nothing would
    ///   leave the caret in front of a name the user cannot see past, and
    ///   by convention the whole of a dotfile's name *is* its stem;
    /// * otherwise the selection stops before the last dot, so
    ///   `archive.tar.gz` selects `archive.tar` and leaves the extension
    ///   alone.
    #[must_use]
    pub fn rename_stem(value: &str) -> Self {
        if value.is_empty() {
            return Self::EMPTY;
        }
        // `rfind` would hand back a *byte* index; the selection speaks in
        // chars, so the dot's position is counted in chars from the start.
        let dot = value
            .char_indices()
            .rfind(|(_, c)| *c == '.')
            .map(|(byte, _)| value[..byte].chars().count());
        match dot {
            None => Self::all(value),
            Some(0) => Self::all(value),
            Some(index) => Self {
                start: 0,
                end: index,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Validation (UIF-INP-003)
// ---------------------------------------------------------------------------

/// The message the reserved slot carries while the input is invalid
/// (`UIF-INP-003`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidationMessage {
    /// A stable code so a message can be grouped and tested without
    /// matching on prose (`NFR-004`).
    pub code: String,
    /// The text shown in `status.danger`.
    pub text: String,
}

impl ValidationMessage {
    /// Build a validation message.
    #[must_use]
    pub fn new(code: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            text: text.into(),
        }
    }
}

/// The verdict one value reaches against one [`ValidationRule`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The value is good; `Ok` is enabled and `Enter` closes the dialog.
    Accept,
    /// The value is bad; `Ok` is disabled and `Enter` reveals the
    /// message.
    Reject(ValidationMessage),
}

impl Verdict {
    /// Whether the value was accepted.
    #[must_use]
    pub const fn is_accept(&self) -> bool {
        matches!(self, Self::Accept)
    }

    /// The rejection message, if this is a rejection.
    #[must_use]
    pub fn message(&self) -> Option<&ValidationMessage> {
        match self {
            Self::Accept => None,
            Self::Reject(message) => Some(message),
        }
    }
}

/// The rule a submitted value is checked against (`UIF-INP-003`).
///
/// The rule is **injected data**, not filesystem logic. The host cannot
/// see a disk and must not be able to: what makes a name legal is stated
/// here as a value so the same host serves `Rename`, `Move`, `Create` and
/// `Delete Confirmation` without a branch on which dialog it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidationRule {
    /// A plain name: non-empty, no illegal basename characters.
    Name,
    /// A rename: a [`ValidationRule::Name`], and distinct from the name
    /// it replaces.
    Rename {
        /// The name being replaced.
        current: String,
    },
    /// A `Move` destination: a folder must actually be chosen.
    Destination,
    /// A destructive confirmation: there must be something to confirm
    /// against. `count` is `1` for a single item and `n` for a
    /// multi-selection (`UIF-INP-004`).
    Confirm {
        /// How many items the confirmation covers.
        count: usize,
    },
    /// Always accept — the caller validates somewhere else, and the slot
    /// is still reserved.
    Always,
}

impl ValidationRule {
    /// Check `value` against this rule.
    ///
    /// Pure: the same rule and value always produce the same verdict, and
    /// nothing outside the return value is read or written.
    #[must_use]
    pub fn check(&self, value: &str) -> Verdict {
        self.problem(value).map_or(Verdict::Accept, Verdict::Reject)
    }

    /// The first rule `value` breaks, if it breaks any.
    fn problem(&self, value: &str) -> Option<ValidationMessage> {
        match self {
            Self::Always => None,
            Self::Destination => {
                if value.trim().is_empty() {
                    Some(reject(CODE_NO_DESTINATION, "Choose a destination folder."))
                } else {
                    None
                }
            }
            Self::Confirm { count } => {
                if *count == 0 {
                    Some(reject(
                        CODE_NOTHING_TO_CONFIRM,
                        "Nothing is selected to delete.",
                    ))
                } else {
                    None
                }
            }
            Self::Name | Self::Rename { .. } => check_basename(value).or_else(|| match self {
                Self::Rename { current } if same_name(value, current) => Some(reject(
                    CODE_UNCHANGED,
                    "The new name must differ from the current name.",
                )),
                _ => None,
            }),
        }
    }
}

/// The basename rules every named dialog shares.
fn check_basename(value: &str) -> Option<ValidationMessage> {
    if value.trim().is_empty() {
        return Some(reject(CODE_EMPTY, "A name is required."));
    }
    if starts_or_ends_with_space(value) {
        return Some(reject(
            CODE_EDGESPACING,
            "A name cannot begin or end with a space.",
        ));
    }
    if value.chars().any(|c| c == '\0' || c.is_control()) {
        return Some(reject(
            CODE_CONTROL,
            "A name cannot contain control characters.",
        ));
    }
    if value.chars().any(|c| ILLEGAL_BASENAME_CHARS.contains(c)) {
        return Some(reject(
            CODE_ILLEGAL_CHAR,
            "A name cannot contain / \\ : * ? \" < > |",
        ));
    }
    None
}

/// Whether `value` has leading or trailing whitespace while not being
/// wholly whitespace (that case is the empty-name rule's job).
fn starts_or_ends_with_space(value: &str) -> bool {
    if value.trim().is_empty() {
        return false;
    }
    let first = value.chars().next().unwrap_or('x');
    let last = value.chars().next_back().unwrap_or('x');
    first.is_whitespace() || last.is_whitespace()
}

/// Whether a proposed rename lands on the name it replaces.
fn same_name(value: &str, current: &str) -> bool {
    value.trim() == current.trim()
}

fn reject(code: &str, text: &str) -> ValidationMessage {
    ValidationMessage::new(code, text)
}

// ---------------------------------------------------------------------------
// The control row (§4.5.1)
// ---------------------------------------------------------------------------

/// What the card's primary input row holds (§4.5.1).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputControl {
    /// A single-line text entry, auto-focused on open (`UIF-INP-004`).
    TextEntry {
        /// The value the field starts with.
        initial: String,
        /// The caret range selected on open.
        selection: Selection,
    },
    /// A scrollable destination folder list (§4.5.1).
    FolderList {
        /// The destinations, in display order.
        folders: Vec<String>,
        /// The row selected on open, clamped into range by the host.
        selected: usize,
    },
    /// No input control — a bare confirmation whose whole message is the
    /// context row.
    Confirmation {
        /// The summary the card is confirming against.
        summary: String,
    },
}

impl InputControl {
    /// The height the middle row takes for this control (§4.5.1).
    #[must_use]
    pub fn row_height(&self) -> f32 {
        match self {
            Self::TextEntry { .. } => TEXT_ENTRY_HEIGHT,
            Self::FolderList { folders, .. } => folder_list_height(folders.len()),
            Self::Confirmation { .. } => CONFIRMATION_CONTROL_HEIGHT,
        }
    }

    /// Whether this control has a text field that can hold the caret.
    #[must_use]
    pub const fn is_text_entry(&self) -> bool {
        matches!(self, Self::TextEntry { .. })
    }

    /// The value this control answers with, given the host's current
    /// selection.
    #[must_use]
    pub fn value_at(&self, selected: usize) -> String {
        match self {
            Self::TextEntry { initial, .. } => initial.clone(),
            Self::FolderList { folders, .. } => folders.get(selected).cloned().unwrap_or_default(),
            Self::Confirmation { summary } => summary.clone(),
        }
    }
}

/// The height of a folder list with `rows` entries (§4.5.1).
///
/// Clamped to at least one row so an empty list still reserves a control
/// region rather than collapsing the card around a hole.
#[must_use]
pub fn folder_list_height(rows: usize) -> f32 {
    let wanted = rows.max(1) as f32 * FOLDER_ROW_HEIGHT;
    wanted.min(FOLDER_LIST_MAX_HEIGHT)
}

// ---------------------------------------------------------------------------
// The box model (§4.5.1)
// ---------------------------------------------------------------------------

/// The widths the two footer buttons are laid out at (§4.5.1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FooterButtons {
    /// The `Cancel` button's width.
    pub cancel: f32,
    /// The `Ok` / `Delete` button's width.
    pub ok: f32,
}

impl Default for FooterButtons {
    fn default() -> Self {
        Self {
            cancel: BUTTON_MIN_WIDTH,
            ok: BUTTON_MIN_WIDTH,
        }
    }
}

/// Every rectangle of the §4.5.1 box model, resolved inside one card.
///
/// The struct is computed once and never mutated. That is the whole of
/// `UIF-INP-003`'s zero-shift guarantee: [`solve`](Self::solve) has no
/// parameter that could carry an error, and no method that recomputes a
/// layout, so an error appearing has no route to the footer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InputLayout {
    /// The card itself.
    pub card: Rectangle,
    /// The title row.
    pub title: Rectangle,
    /// The context / target label row.
    pub context: Rectangle,
    /// The primary input control's region.
    pub control: Rectangle,
    /// The permanently reserved validation message slot.
    pub validation: Rectangle,
    /// The footer action row.
    pub footer: Rectangle,
    /// The `Cancel` button, inside the footer.
    pub cancel: Rectangle,
    /// The `Ok` / `Delete` button, inside the footer.
    pub ok: Rectangle,
}

impl InputLayout {
    /// The card's height for one control kind (§4.5.1).
    ///
    /// The sum of the box model: padding, the two fixed rows above the
    /// control, the control, the reserved slot, the footer, and the four
    /// gaps between them.
    #[must_use]
    pub fn card_height(control: &InputControl) -> f32 {
        let fixed = TITLE_ROW_HEIGHT
            + CONTEXT_ROW_HEIGHT
            + VALIDATION_SLOT_HEIGHT
            + FOOTER_HEIGHT
            + 4.0 * ROW_GAP
            + 2.0 * CARD_PADDING;
        fixed + control.row_height()
    }

    /// The card's size for one control kind — the width is fixed at
    /// [`CARD_WIDTH`] whatever the control is.
    #[must_use]
    pub fn card_size(control: &InputControl) -> Size {
        Size::new(CARD_WIDTH, Self::card_height(control))
    }

    /// Resolve every row inside `card`.
    ///
    /// **Note the parameter list.** There is no error, no verdict, no
    /// "is the slot showing" flag. The validation slot occupies the same
    /// rectangle whether it is empty or full, because nothing here can be
    /// told the difference.
    #[must_use]
    pub fn solve(card: Rectangle, control: &InputControl, buttons: FooterButtons) -> Self {
        let pad = CARD_PADDING;
        let inner_width = (card.width - 2.0 * pad).max(0.0);
        let x = card.x + pad;
        let row =
            |y: f32, height: f32| Rectangle::new(Point::new(x, y), Size::new(inner_width, height));

        let top = card.y + pad;
        let title = row(top, TITLE_ROW_HEIGHT);

        let below_title = top + TITLE_ROW_HEIGHT + ROW_GAP;
        let context = row(below_title, CONTEXT_ROW_HEIGHT);

        let below_context = below_title + CONTEXT_ROW_HEIGHT + ROW_GAP;
        let region = row(below_context, control.row_height());

        let below_control = below_context + control.row_height() + ROW_GAP;
        let validation = row(below_control, VALIDATION_SLOT_HEIGHT);

        let below_validation = below_control + VALIDATION_SLOT_HEIGHT + ROW_GAP;
        let footer = row(below_validation, FOOTER_HEIGHT);

        let (cancel, ok) = footer_buttons(footer, buttons);

        Self {
            card,
            title,
            context,
            control: region,
            validation,
            footer,
            cancel,
            ok,
        }
    }
}

/// Right-align the two footer buttons with a [`ROW_GAP`] between them
/// (§4.5.1 "Right-Aligned, Gap: 8px").
fn footer_buttons(footer: Rectangle, buttons: FooterButtons) -> (Rectangle, Rectangle) {
    let ok_x = footer.x + footer.width - buttons.ok;
    let cancel_x = ok_x - ROW_GAP - buttons.cancel;

    let cancel = Rectangle::new(
        Point::new(cancel_x, footer.y),
        Size::new(buttons.cancel, FOOTER_HEIGHT),
    );
    let ok = Rectangle::new(
        Point::new(ok_x, footer.y),
        Size::new(buttons.ok, FOOTER_HEIGHT),
    );
    (cancel, ok)
}

// ---------------------------------------------------------------------------
// The spec
// ---------------------------------------------------------------------------

/// How the primary button is styled (`UIF-INP-004`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonIntent {
    /// The ordinary primary action.
    #[default]
    Standard,
    /// A destructive action: the primary renders in `status.danger_fill`.
    Danger,
}

impl ButtonIntent {
    /// Whether this intent destroys something.
    #[must_use]
    pub const fn is_destructive(self) -> bool {
        matches!(self, Self::Danger)
    }
}

/// What a delete confirmation covers (`UIF-INP-004`).
///
/// §4.5.1 asks for the item **name** on a single delete and the item
/// **count** on a multi-select delete, so the two cases are different
/// data rather than one string the caller formats by hand.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeleteTargets {
    /// One item, named.
    Single(String),
    /// A multi-selection, counted.
    Many(usize),
}

impl DeleteTargets {
    /// How many items are being deleted.
    #[must_use]
    pub fn count(&self) -> usize {
        match self {
            Self::Single(_) => 1,
            Self::Many(count) => *count,
        }
    }

    /// The summary the context row carries.
    #[must_use]
    pub fn summary(&self) -> String {
        match self {
            Self::Single(name) => name.clone(),
            Self::Many(count) => format!("{count} items"),
        }
    }
}

/// Everything needed to open one input dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct InputDialogSpec {
    /// The surface identity.
    pub id: SurfaceId,
    /// The title row's text.
    pub title: String,
    /// The context / target label's text.
    pub context: String,
    /// The primary input control.
    pub control: InputControl,
    /// How the primary button is styled.
    pub intent: ButtonIntent,
    /// The rule the value is checked against.
    pub rule: ValidationRule,
    /// The footer button widths.
    pub buttons: FooterButtons,
}

impl InputDialogSpec {
    /// Build a dialog spec.
    #[must_use]
    pub fn new(
        id: impl Into<SurfaceId>,
        title: impl Into<String>,
        context: impl Into<String>,
        control: InputControl,
        rule: ValidationRule,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            context: context.into(),
            control,
            intent: ButtonIntent::Standard,
            rule,
            buttons: FooterButtons::default(),
        }
    }

    /// §4.5 — a `Rename` card: the stem of the current name is selected
    /// on open and the value must differ from it (`UIF-INP-004`).
    #[must_use]
    pub fn rename(
        id: impl Into<SurfaceId>,
        container: impl Into<String>,
        current: impl Into<String>,
    ) -> Self {
        let current = current.into();
        Self::new(
            id,
            "Rename",
            container,
            InputControl::TextEntry {
                initial: current.clone(),
                selection: Selection::rename_stem(&current),
            },
            ValidationRule::Rename { current },
        )
    }

    /// §4.5 — a `Create Directory` / `New Document` card.
    #[must_use]
    pub fn create(id: impl Into<SurfaceId>, title: &str, container: impl Into<String>) -> Self {
        Self::new(
            id,
            title,
            container,
            InputControl::TextEntry {
                initial: String::new(),
                selection: Selection::EMPTY,
            },
            ValidationRule::Name,
        )
    }

    /// §4.5 — a `Move` card: a destination folder list.
    #[must_use]
    pub fn move_item(
        id: impl Into<SurfaceId>,
        name: impl Into<String>,
        folders: Vec<String>,
    ) -> Self {
        Self::new(
            id,
            "Move",
            name,
            InputControl::FolderList {
                folders,
                selected: 0,
            },
            ValidationRule::Destination,
        )
    }

    /// §4.5 / `UIF-INP-004` — a `Delete Confirmation`, destructive by
    /// definition, and otherwise the same card as every other dialog here.
    #[must_use]
    pub fn delete_confirmation(id: impl Into<SurfaceId>, targets: DeleteTargets) -> Self {
        let summary = targets.summary();
        let mut spec = Self::new(
            id,
            "Delete",
            summary.clone(),
            InputControl::Confirmation { summary },
            ValidationRule::Confirm {
                count: targets.count(),
            },
        );
        spec.intent = ButtonIntent::Danger;
        spec
    }

    /// The card size this spec's control implies.
    #[must_use]
    pub fn card_size(&self) -> Size {
        InputLayout::card_size(&self.control)
    }

    /// The focusable roster this spec implies, in tab order.
    ///
    /// The primary is disabled when the value the spec starts with does
    /// not validate, so a card opens with `Ok` already greyed rather than
    /// greying one frame late.
    #[must_use]
    pub fn roster(&self) -> Vec<FocusMember> {
        let primary = self.primary_focus();
        let ok_enabled = self
            .rule
            .check(&self.control.value_at(self.selected()))
            .is_accept();
        let ok = if ok_enabled {
            FocusMember::new(self.control_id(OK_ROLE))
        } else {
            FocusMember::disabled(self.control_id(OK_ROLE))
        };
        vec![
            FocusMember::new(primary),
            FocusMember::new(self.control_id(CANCEL_ROLE)),
            ok,
        ]
    }

    /// The control the dialog focuses when it opens.
    ///
    /// A text entry takes the caret (`UIF-INP-004`). A folder list takes
    /// focus as a unit. A bare confirmation lands on **`Cancel`**, not on
    /// the `Danger` button: nothing in the spec asks a destructive card to
    /// start with its finger on the trigger, and starting there means a
    /// stray `Enter` deletes.
    #[must_use]
    pub fn primary_focus(&self) -> FocusId {
        match &self.control {
            InputControl::TextEntry { .. } => self.control_id(ENTRY_ROLE),
            InputControl::FolderList { .. } => self.control_id(LIST_ROLE),
            InputControl::Confirmation { .. } => self.control_id(CANCEL_ROLE),
        }
    }

    /// The `FocusId` for one of this dialog's roles.
    #[must_use]
    pub fn control_id(&self, role: &str) -> FocusId {
        FocusId::new(format!("{}:{}", self.id.as_str(), role))
    }

    /// The selected index for the current control kind.
    #[must_use]
    pub fn selected(&self) -> usize {
        match &self.control {
            InputControl::FolderList { selected, .. } => *selected,
            _ => 0,
        }
    }
}

/// Focus role: the text entry.
pub const ENTRY_ROLE: &str = "entry";
/// Focus role: the destination folder list.
pub const LIST_ROLE: &str = "list";
/// Focus role: the `Cancel` button.
pub const CANCEL_ROLE: &str = "cancel";
/// Focus role: the `Ok` / `Delete` button.
pub const OK_ROLE: &str = "ok";

/// The focus group a dialog's controls live in.
///
/// Derived from the surface id so the host and the caller cannot disagree
/// about which group the trap confines.
#[must_use]
pub fn group_for(id: &SurfaceId) -> GroupId {
    GroupId::new(format!("input:{}", id.as_str()))
}

// ---------------------------------------------------------------------------
// Outcomes
// ---------------------------------------------------------------------------

/// What the pointer was reported to have hit inside the card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputTarget {
    /// Empty card padding.
    Padding,
    /// A control the caller says is disabled.
    Disabled,
    /// The text entry.
    Entry,
    /// A destination folder row, by index.
    FolderRow(usize),
    /// The `Cancel` button.
    Cancel,
    /// The `Ok` / `Delete` button.
    Ok,
}

/// Why the host did nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputNoOp {
    /// The dialog is already closed.
    NotOpen,
    /// [`ZBand::Modal`] does not trap this key class.
    KeyNotTrapped,
    /// The band swallows the class but the focused control owns it.
    KeyOwnedByControl,
    /// Nothing in the card can take focus.
    NothingFocusable,
    /// A surface above consumed the key first.
    KeyConsumedElsewhere,
    /// The dialog is not registered in the z-order registry.
    NotRegistered,
}

/// Why a dismissal was attempted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DismissCause {
    /// The `Escape` key.
    Escape,
    /// The `Cancel` button.
    CancelButton,
    /// The backdrop scrim.
    Scrim,
    /// The caller asked programmatically.
    Programmatic,
}

/// What the dialog answered with (`UIF-INP-004`).
///
/// One answer type for every dialog kind, destructive or not: that is
/// destructive parity. A caller handling `Delete` handles exactly what it
/// handles for `Rename`, plus a button colour.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputAnswer {
    /// The user confirmed, with this value.
    Accepted {
        /// The accepted value.
        value: String,
    },
    /// The user cancelled.
    Cancelled,
}

/// What one interaction with the host did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InputOutcome {
    /// The dialog closed with an accepted value.
    Accepted {
        /// The accepted value.
        value: String,
    },
    /// The dialog closed as cancelled.
    Cancelled {
        /// What closed it.
        cause: DismissCause,
    },
    /// `Enter` landed on an invalid value: the slot is now showing.
    ErrorRevealed {
        /// The message now in the reserved slot.
        message: ValidationMessage,
    },
    /// A press inside the card was swallowed by the card boundary
    /// (`UIF-INP-002`).
    Absorbed {
        /// What was hit.
        target: InputTarget,
    },
    /// Focus moved inside the card.
    FocusMoved {
        /// Where it landed.
        focused: FocusId,
    },
    /// Nothing happened.
    NoOp {
        /// Why.
        reason: InputNoOp,
    },
}

// ---------------------------------------------------------------------------
// The host
// ---------------------------------------------------------------------------

/// The input dialog host — the frame that makes `UIF-INP-001/002/003/004`
/// hold.
///
/// The host owns the card geometry, the current value, the live verdict
/// and whether the error has been revealed. The focus group and the
/// z-order surface are registered *into* collaborators the caller passes
/// in, because both are shared with the rest of the window.
#[derive(Clone, Debug, PartialEq)]
pub struct InputDialogHost {
    id: SurfaceId,
    group: GroupId,
    control: InputControl,
    rule: ValidationRule,
    intent: ButtonIntent,
    primary: FocusId,
    cancel: FocusId,
    ok: FocusId,
    value: String,
    selected: usize,
    verdict: Verdict,
    revealed: Option<ValidationMessage>,
    layout: InputLayout,
    placement: Placement,
    viewport: Rectangle,
    /// The `Ephemeral` half: the `Ok` button's activation engine, holding
    /// the gate that suppresses a second accept while the first is still
    /// running (`UIF-ATOM-000b`, §3.0.4.3).
    primary_cta: DialogButtonController,
    open: bool,
}

impl InputDialogHost {
    /// Open a dialog.
    ///
    /// In order: size the card from the control kind, solve the placement
    /// through the shared anchor solver, resolve the box model, register
    /// the focus group, push the trap, move focus to the primary control,
    /// and register the Band 3 surface over the viewport.
    ///
    /// The trap goes on **before** focus is moved so the primary control
    /// is focused *through* it. If nothing inside the card can hold focus,
    /// focus is cleared rather than left on whatever the background had —
    /// a lit ring under the scrim is the leak the trap exists to prevent.
    ///
    /// `point` is the trigger point the dialog opens near and
    /// `keyboard_invoked` selects centred placement, both handed straight
    /// to [`Anchor::CursorProximal`] (`UIF-INP-001`).
    #[must_use]
    pub fn open(
        spec: InputDialogSpec,
        point: Point,
        keyboard_invoked: bool,
        viewport: Rectangle,
        focus: &mut FocusState,
        registry: &mut SurfaceRegistry,
    ) -> Self {
        let size = spec.card_size();
        let placement = anchor::solve(
            Anchor::CursorProximal {
                point,
                keyboard_invoked,
            },
            size,
            viewport,
        );
        let card = placement.rect();
        let layout = InputLayout::solve(card, &spec.control, spec.buttons);

        let group = group_for(&spec.id);
        let primary = spec.primary_focus();
        let cancel = spec.control_id(CANCEL_ROLE);
        let ok = spec.control_id(OK_ROLE);
        let selected = clamp_selected(&spec.control, spec.selected());
        let value = spec.control.value_at(selected);
        let verdict = spec.rule.check(&value);

        focus.register_group(group.clone(), spec.roster());
        focus.push_trap(group.clone());

        if !focus.focus(&primary) && focus.focus_first_in_group(&group).is_none() {
            focus.clear();
        }

        registry.register(spec.id.clone(), SurfaceKind::InputDialog, vec![viewport]);

        Self {
            id: spec.id,
            group,
            control: spec.control,
            rule: spec.rule,
            intent: spec.intent,
            primary,
            cancel,
            ok,
            value,
            selected,
            verdict,
            revealed: None,
            layout,
            placement,
            viewport,
            primary_cta: DialogButtonController::new(DialogButtonRole::Primary),
            open: true,
        }
    }

    // -- identity ----------------------------------------------------------

    /// The dialog's surface identity.
    #[must_use]
    pub fn id(&self) -> &SurfaceId {
        &self.id
    }

    /// The focus group this dialog's controls live in.
    #[must_use]
    pub fn group_id(&self) -> &GroupId {
        &self.group
    }

    /// The control id for one of this dialog's roles.
    #[must_use]
    pub fn control_id(&self, role: &str) -> FocusId {
        FocusId::new(format!("{}:{}", self.id.as_str(), role))
    }

    /// The `Ok` / `Delete` button's id.
    #[must_use]
    pub fn ok_id(&self) -> &FocusId {
        &self.ok
    }

    /// The `Cancel` button's id.
    #[must_use]
    pub fn cancel_id(&self) -> &FocusId {
        &self.cancel
    }

    /// The control that holds the caret or the list selection.
    #[must_use]
    pub fn primary_id(&self) -> &FocusId {
        &self.primary
    }

    /// Whether the dialog is still open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        self.open
    }

    /// How the primary button is styled (`UIF-INP-004`).
    #[must_use]
    pub const fn intent(&self) -> ButtonIntent {
        self.intent
    }

    /// The control kind the middle row holds.
    #[must_use]
    pub const fn control(&self) -> &InputControl {
        &self.control
    }

    // -- geometry ----------------------------------------------------------

    /// The card rectangle, in root coordinates.
    #[must_use]
    pub const fn card(&self) -> Rectangle {
        self.layout.card
    }

    /// The full resolved box model.
    #[must_use]
    pub const fn layout(&self) -> &InputLayout {
        &self.layout
    }

    /// The placement the anchor solver produced (`UIF-INP-001`).
    #[must_use]
    pub const fn placement(&self) -> Placement {
        self.placement
    }

    /// The viewport the dialog was solved against.
    #[must_use]
    pub const fn viewport(&self) -> Rectangle {
        self.viewport
    }

    /// The scrim's bounds — the whole viewport, because the Band 3 scrim
    /// is full-window (§5).
    #[must_use]
    pub const fn scrim_bounds(&self) -> Rectangle {
        self.viewport
    }

    /// Whether `at` is inside the card.
    ///
    /// Uses iced's convention, which the whole crate shares: the right and
    /// bottom edges are *outside*.
    #[must_use]
    pub fn is_inside_card(&self, at: Point) -> bool {
        self.layout.card.contains(at)
    }

    // -- value and validation (UIF-INP-003) --------------------------------

    /// The current value.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// The live verdict for the current value.
    #[must_use]
    pub fn verdict(&self) -> &Verdict {
        &self.verdict
    }

    /// Whether `Ok` is currently enabled.
    #[must_use]
    pub fn ok_is_enabled(&self) -> bool {
        self.verdict.is_accept()
    }

    /// The message currently shown in the reserved slot, if `Enter` has
    /// revealed one.
    #[must_use]
    pub fn revealed(&self) -> Option<&ValidationMessage> {
        self.revealed.as_ref()
    }

    /// The index selected in a destination folder list.
    #[must_use]
    pub fn selected(&self) -> usize {
        self.selected
    }

    /// Record a new value from the text entry and re-run the rule
    /// (`UIF-INP-003`'s live validation).
    ///
    /// Updates the verdict, enables or disables `Ok` in the focus model,
    /// and clears a previously revealed message once the value is good.
    /// It does **not** reveal a message: only `Enter` does that.
    pub fn set_value(&mut self, value: String, focus: &mut FocusState) -> Verdict {
        self.value = value;
        let verdict = self.rule.check(&self.value);
        self.verdict = verdict.clone();
        focus.set_member_enabled(&self.group, &self.ok, verdict.is_accept());
        if verdict.is_accept() {
            self.revealed = None;
        }
        verdict
    }

    /// Select a destination folder row and re-run the rule.
    ///
    /// Out-of-range indices are clamped rather than rejected, so a list
    /// that shrank underneath the selection still answers with a folder
    /// that exists.
    pub fn set_selected(&mut self, index: usize, focus: &mut FocusState) -> usize {
        self.selected = clamp_selected(&self.control, index);
        self.value = self.control.value_at(self.selected);
        let verdict = self.rule.check(&self.value);
        self.verdict = verdict.clone();
        focus.set_member_enabled(&self.group, &self.ok, verdict.is_accept());
        if verdict.is_accept() {
            self.revealed = None;
        }
        self.selected
    }

    // -- submission (§3.0.4.2 tiers) ---------------------------------------

    /// Try to accept the dialog, on the `Commit` tier.
    ///
    /// Accepted: tear down and hand the value back inside a [`Commit`] —
    /// the tier that says *this is the settled answer, persist it*.
    /// Rejected: reveal the message, pull focus back to the text entry,
    /// and **stay open** — the invalid branch contains no teardown call at
    /// all, which is why `UIF-INP-003`'s "keeps focus inside the text
    /// box" cannot regress into a close.
    ///
    /// The activation gate runs *after* the rule and *before* the
    /// teardown. That order matters twice over: a rejected value must not
    /// put an action in flight, and a second accept while the first is
    /// still running is withheld without re-validating and without
    /// revealing anything — §3.0.4.3's duplicate-activation suppression
    /// for a dialog's primary call to action.
    #[must_use]
    pub fn commit_answer(
        &mut self,
        focus: &mut FocusState,
        registry: &mut SurfaceRegistry,
    ) -> Option<Commit<InputAnswer>> {
        if !self.open {
            return None;
        }
        let verdict = self.rule.check(&self.value);
        self.verdict = verdict.clone();
        focus.set_member_enabled(&self.group, &self.ok, verdict.is_accept());

        match verdict {
            Verdict::Accept => {
                if !self.primary_cta.try_dispatch() {
                    return None;
                }
                let value = std::mem::take(&mut self.value);
                self.teardown(focus);
                registry.unregister(&self.id);
                Some(Commit::new(InputAnswer::Accepted { value }))
            }
            Verdict::Reject(message) => {
                self.revealed = Some(message);
                if self.control.is_text_entry() {
                    focus.focus(&self.primary);
                }
                None
            }
        }
    }

    /// Discard the dialog, on the `Cancel` tier.
    ///
    /// The value the revert hands back is [`InputAnswer::Cancelled`] — the
    /// dialog's answer *is* "nothing was accepted" — and carrying it on
    /// the `Cancel` tier rather than as a bare message is what tells the
    /// host this is the discard path, not the persist path.
    #[must_use]
    pub fn cancel_answer(
        &mut self,
        focus: &mut FocusState,
        registry: &mut SurfaceRegistry,
    ) -> Option<Cancel<InputAnswer>> {
        if !self.open {
            return None;
        }
        self.teardown(focus);
        registry.unregister(&self.id);
        Some(Cancel::new(InputAnswer::Cancelled))
    }

    /// Try to accept the dialog.
    ///
    /// The pre-tier shape of [`commit_answer`](Self::commit_answer), kept so
    /// existing callers are unaffected: the tier is unwrapped and the
    /// answer is exactly what it always was.
    #[must_use]
    pub fn submit(
        &mut self,
        focus: &mut FocusState,
        registry: &mut SurfaceRegistry,
    ) -> Option<InputAnswer> {
        self.commit_answer(focus, registry).map(Commit::into_value)
    }

    /// Cancel the dialog at the caller's request.
    ///
    /// The pre-tier shape of [`cancel_answer`](Self::cancel_answer).
    #[must_use]
    pub fn cancel(
        &mut self,
        focus: &mut FocusState,
        registry: &mut SurfaceRegistry,
    ) -> Option<InputAnswer> {
        self.cancel_answer(focus, registry)
            .map(Cancel::into_restored)
    }

    // -- pending action (§3.0.4.3) -----------------------------------------

    /// Record the host's current [`ActionPending`] projection for the
    /// dialog's `Ok` button.
    ///
    /// Call once per frame, before any press is handled. Busy-ness enters
    /// only as a value; the host never reads a clock.
    pub fn set_pending(&mut self, pending: ActionPending) {
        self.primary_cta.observe(pending);
    }

    /// The pending-action projection currently in force.
    #[must_use]
    pub fn pending(&self) -> ActionPending {
        self.primary_cta.pending()
    }

    /// Whether the dialog's accept action is still running.
    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.primary_cta.pending().is_busy()
    }

    /// Ask the running accept action to stop — the `on_cancel_request()`
    /// path, surfaced once per busy period.
    pub fn request_cancel(&mut self) -> Option<CancelRequest> {
        self.primary_cta.take_cancel_request()
    }

    /// The `Ephemeral` view of the dialog's interaction half (§3.0.4.1).
    #[must_use]
    pub fn ephemeral(&self) -> Ephemeral<&DialogButtonController> {
        Ephemeral::new(&self.primary_cta)
    }

    // -- keyboard ----------------------------------------------------------

    /// Drive one key class through the host.
    ///
    /// What is handled is read from [`ZBand::key_disposition`] for
    /// [`ZBand::Modal`], not from a list of keys the host likes.
    pub fn on_key(
        &mut self,
        class: KeyClass,
        focus: &mut FocusState,
        registry: &mut SurfaceRegistry,
    ) -> InputOutcome {
        if !self.open {
            return InputOutcome::NoOp {
                reason: InputNoOp::NotOpen,
            };
        }

        match ZBand::Modal.key_disposition(class) {
            KeyDisposition::PassThrough => InputOutcome::NoOp {
                reason: InputNoOp::KeyNotTrapped,
            },
            KeyDisposition::Swallow | KeyDisposition::SwallowWhenFocused => match class {
                KeyClass::Tab => self.step_focus(focus, true),
                KeyClass::ShiftTab => self.step_focus(focus, false),
                KeyClass::Enter => self.enter(focus, registry),
                KeyClass::Escape => self.escape_outcome(registry, focus),
                KeyClass::Space | KeyClass::ArrowNavigation => InputOutcome::NoOp {
                    reason: InputNoOp::KeyOwnedByControl,
                },
            },
        }
    }

    /// `Tab` / `Shift+Tab` inside the trap.
    fn step_focus(&mut self, focus: &mut FocusState, forward: bool) -> InputOutcome {
        match if forward {
            focus.focus_next()
        } else {
            focus.focus_previous()
        } {
            Some(focused) => InputOutcome::FocusMoved { focused },
            None => InputOutcome::NoOp {
                reason: InputNoOp::NothingFocusable,
            },
        }
    }

    /// `Enter` — the reveal-and-keep-focus path (`UIF-INP-003`).
    fn enter(&mut self, focus: &mut FocusState, registry: &mut SurfaceRegistry) -> InputOutcome {
        match self.submit(focus, registry) {
            Some(InputAnswer::Accepted { value }) => InputOutcome::Accepted { value },
            Some(InputAnswer::Cancelled) => InputOutcome::NoOp {
                reason: InputNoOp::NotOpen,
            },
            None => match self.revealed.clone() {
                Some(message) => InputOutcome::ErrorRevealed { message },
                None => InputOutcome::NoOp {
                    reason: InputNoOp::NothingFocusable,
                },
            },
        }
    }

    // -- pointer -----------------------------------------------------------

    /// Drive one pointer press.
    ///
    /// **The card boundary is decided first, from the host's own
    /// rectangle.** If the point is inside the card, control flow cannot
    /// reach the scrim at all: the scrim branch is the `else` of a test
    /// that has already been taken. That is the structural form of
    /// `UIF-INP-002` — a click on a disabled `Ok` or on empty padding is
    /// not a case that is handled, it is a case that cannot escape.
    ///
    /// `target` is what the caller's own hit test found inside the card.
    /// It is **not consulted when the point is outside the card**, and it
    /// is checked rather than trusted: an `Ok` report while the value is
    /// invalid is downgraded to a disabled hit.
    pub fn on_pointer(
        &mut self,
        at: Point,
        target: InputTarget,
        focus: &mut FocusState,
        registry: &mut SurfaceRegistry,
    ) -> InputOutcome {
        if !self.open {
            return InputOutcome::NoOp {
                reason: InputNoOp::NotOpen,
            };
        }

        if !self.is_inside_card(at) {
            return self.attempt_dismiss(DismissCause::Scrim, focus, registry);
        }

        match target {
            InputTarget::Padding | InputTarget::Disabled => InputOutcome::Absorbed {
                target: InputTarget::Disabled,
            },
            InputTarget::Entry => self.focus_role(ENTRY_ROLE, focus),
            InputTarget::FolderRow(index) => {
                self.set_selected(index, focus);
                InputOutcome::FocusMoved {
                    focused: self.control_id(LIST_ROLE),
                }
            }
            InputTarget::Cancel => {
                self.attempt_dismiss(DismissCause::CancelButton, focus, registry)
            }
            InputTarget::Ok if !self.ok_is_enabled() => InputOutcome::Absorbed {
                target: InputTarget::Disabled,
            },
            InputTarget::Ok => match self.submit(focus, registry) {
                Some(InputAnswer::Accepted { value }) => InputOutcome::Accepted { value },
                Some(InputAnswer::Cancelled) | None => InputOutcome::NoOp {
                    reason: InputNoOp::NotOpen,
                },
            },
        }
    }

    /// Move focus to one of this dialog's own controls, by role.
    ///
    /// The roster is checked rather than trusted: a role this dialog does
    /// not own, or one the focus model refuses, is downgraded to an
    /// absorbed press.
    fn focus_role(&mut self, role: &str, focus: &mut FocusState) -> InputOutcome {
        let id = self.control_id(role);
        let owns = focus
            .group_of(&id)
            .is_some_and(|group| group.id() == &self.group);
        if !owns || !focus.focus(&id) {
            return InputOutcome::Absorbed {
                target: InputTarget::Disabled,
            };
        }
        InputOutcome::FocusMoved { focused: id }
    }

    // -- dismissal ---------------------------------------------------------

    /// The only route from a user gesture to a closed dialog.
    ///
    /// `Escape`, `Cancel` and the scrim all arrive here, which is the
    /// point: there is exactly one thing a user gesture can do to close
    /// this card, and it is always a cancel. Destructive parity
    /// (`UIF-INP-004`) depends on it — the delete card cannot grow a
    /// second, different close path because there is no second door.
    fn attempt_dismiss(
        &mut self,
        cause: DismissCause,
        focus: &mut FocusState,
        registry: &mut SurfaceRegistry,
    ) -> InputOutcome {
        self.teardown(focus);
        registry.unregister(&self.id);
        InputOutcome::Cancelled { cause }
    }

    /// Close the dialog at the caller's request.
    #[must_use]
    pub fn close(
        &mut self,
        focus: &mut FocusState,
        registry: &mut SurfaceRegistry,
    ) -> InputOutcome {
        if !self.open {
            return InputOutcome::NoOp {
                reason: InputNoOp::NotOpen,
            };
        }
        self.teardown(focus);
        registry.unregister(&self.id);
        InputOutcome::Cancelled {
            cause: DismissCause::Programmatic,
        }
    }

    /// Tear the dialog down around its focus, without touching the
    /// registry.
    ///
    /// **Pop the trap, then unregister the group** — the order is
    /// load-bearing for the same reason it is in [`modal`](super::modal):
    /// popping first lets the focus model settle the caret back into the
    /// background before the group it came from disappears.
    ///
    /// The registry is deliberately *not* part of this: the escape walk
    /// removes the surface itself, by index, and unregistering from
    /// inside that closure would shift the list under its feet.
    fn teardown(&mut self, focus: &mut FocusState) {
        focus.pop_trap();
        focus.unregister_group(&self.group);
        self.open = false;
        self.primary_cta.reset();
    }

    // -- z-order integration -----------------------------------------------

    /// The dialog's own answer when the escape chain asks it to unwind.
    ///
    /// Always `Dismissed` while open: an input dialog holds no unsaved
    /// work, so unlike a [`modal`](super::modal) it never answers
    /// `CannotUnwind`.
    ///
    /// **Must not mutate the registry** — the caller is normally
    /// [`SurfaceRegistry::escape`], which indexes the surface list and
    /// removes the entry it was told closed.
    pub fn unwind_answer(&mut self, focus: &mut FocusState) -> UnwindAnswer {
        if !self.open {
            return UnwindAnswer::Declined(DeclineReason::NotOpen);
        }
        self.teardown(focus);
        UnwindAnswer::Dismissed
    }

    /// Run one `Escape` through the z-order chain with this dialog as the
    /// only surface it answers for.
    ///
    /// Every other surface is answered `Declined(NotOpen)` — "not mine,
    /// the key keeps travelling" — which is what lets a menu open above
    /// the dialog unwind first, exactly as **G-13** orders it, without
    /// the dialog knowing menus exist.
    pub fn escape_via(
        &mut self,
        registry: &mut SurfaceRegistry,
        focus: &mut FocusState,
    ) -> EscapeOutcome {
        let id = self.id.clone();
        registry.escape(move |surface| {
            if surface.id() != &id {
                return UnwindAnswer::Declined(DeclineReason::NotOpen);
            }
            self.unwind_answer(focus)
        })
    }

    /// Map the chain's answer onto a host outcome.
    fn escape_outcome(
        &mut self,
        registry: &mut SurfaceRegistry,
        focus: &mut FocusState,
    ) -> InputOutcome {
        let outcome = self.escape_via(registry, focus);

        if outcome.consumed_by() == Some(&self.id) {
            return InputOutcome::Cancelled {
                cause: DismissCause::Escape,
            };
        }
        if outcome.is_consumed() {
            return InputOutcome::NoOp {
                reason: InputNoOp::KeyConsumedElsewhere,
            };
        }
        InputOutcome::NoOp {
            reason: InputNoOp::NotRegistered,
        }
    }
}

/// Clamp a folder-list selection into the range that actually exists.
fn clamp_selected(control: &InputControl, index: usize) -> usize {
    match control {
        InputControl::FolderList { folders, .. } => index.min(folders.len().saturating_sub(1)),
        _ => 0,
    }
}

/// The byte offset of character number `index` in `value`, saturating at
/// `value.len()`.
///
/// [`Selection`] speaks in `chars`, so turning a selection into a slice
/// needs this translation. Saturating rather than failing means a selection
/// that runs past the end of a shortened value resolves to the end instead
/// of panicking on a render path.
fn char_boundary(value: &str, index: usize) -> usize {
    value
        .char_indices()
        .nth(index)
        .map_or(value.len(), |(byte, _)| byte)
}

/// The stroke a card perimeter is drawn with (§4.5.1 `border.strong`).
#[must_use]
pub const fn card_border() -> f32 {
    shape::BORDER_HAIRLINE
}

#[cfg(test)]
#[path = "input_dialog_tests.rs"]
mod input_dialog_tests;
