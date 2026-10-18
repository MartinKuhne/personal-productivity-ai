//! Reusable overlay primitives for the iced UI: a cursor-anchored context
//! menu and a blocking modal dialog ([TASK-017a], DEC-005 — iced built-ins
//! only, no `iced_aw`).
//!
//! The egui build gets both shapes for free from immediate mode:
//! `response.context_menu(..)` on a hovered row
//! (`src/app/ui/tree/render.rs:18-60`, `src/app/ui/panels/center.rs:208-315`)
//! and floating `egui::Window`s (`src/app/ui/modals.rs:26-68`). iced has no
//! single call that does either, so this module supplies them as **pure view
//! functions over plain state structs** — no bus access, no side effects, no
//! `FastMdApp` reads. The individual dialogs (about / batch / tools /
//! agent-debug / attributions) are a later chunk and are NOT ported here.
//!
//! # Verified iced 0.14 APIs
//!
//! | API | Source |
//! | --- | --- |
//! | `mouse_area(w)` / `MouseArea` | `iced_widget-0.14.2/src/helpers.rs:2010-2017`, `mouse_area.rs:14-33` |
//! | `MouseArea::on_right_press` | `mouse_area.rs:68-71`; fires + captures at `mouse_area.rs:407-412` |
//! | `MouseArea::on_right_release` | `mouse_area.rs:75-78` |
//! | `MouseArea::on_move(Fn(Point) -> Message)` | `mouse_area.rs:113-116`; position is **widget-relative** (`cursor.position_in(bounds)`, `mouse_area.rs:358-360`) |
//! | `MouseArea::interaction` | `mouse_area.rs:127-131` |
//! | `Stack` — layered draw, top-down event routing, early-return on capture | `stack.rs:23-30`, `stack.rs:249-275` |
//! | `Float::translate(Fn(Rectangle, Rectangle) -> Vector)` | `float.rs:53-59`; overlay only activates when `scale > 1.0 \|\| translation != ZERO` (`float.rs:79-84`, `float.rs:226-250`) |
//! | `overlay::menu::Menu` / `menu::State` | `overlay/menu.rs:20-141`, `menu.rs:144-162`; rows are plain `ToString` (`menu.rs:557-576`) |
//! | Overlays update **before** the base tree; the base is skipped for any event the overlay `Captured` | `iced_runtime-0.14.0/src/user_interface.rs:198-254` and `user_interface.rs:310-316` |
//! | `container` alignment (`align_x`/`align_y`, default Left/Top) | `container.rs:100-101`, `container.rs:188-203` |
//! | `Padding { top, right, bottom, left }` | `iced_core-0.14.0/src/padding.rs:36-45` |
//! | `Button::on_press_maybe(Option<Message>)` | `button.rs:170-173` |
//!
//! # Design: `Stack` in the base tree, not the `overlay` layer
//!
//! Both primitives mount as extra layers of a [`iced::widget::Stack`] over the
//! caller's own content rather than as `overlay::Element`s. Three reasons:
//!
//! 1. **A scrim needs the full area.** Blocking input means a full-area layer
//!    that swallows the click. `Float` positions by *delta from where the
//!    content would naturally sit* and only becomes an overlay when that delta
//!    is non-zero (`float.rs:226`); a full-viewport scrim sits at its natural
//!    origin, so its translation is `Vector::ZERO` and it would silently stop
//!    floating. `Stack` has no such condition.
//! 2. **`overlay::menu::Menu` is not reusable here.** It is built *as* the
//!    overlay, so it takes caller-held `&mut menu::State` and
//!    `&mut Option<usize>` (`menu.rs:57-79`) — that forces `&mut` into every
//!    view function, against the pure-view pattern the panels already use
//!    (`panels/top.rs:544-553`). It also renders each row as one plain
//!    `ToString` row with no per-row handler and never closes itself. A
//!    [`Column`] of [`iced::widget::Button`]s gives per-row `Message`s,
//!    native hover styling, keyboard focus, and needs no external state.
//! 3. **Event precedence is already what we want.** `Stack::update` walks
//!    children top-down and returns as soon as one captures the event
//!    (`stack.rs:262-264`), so the dialog card gets the click and the scrim
//!    only sees clicks that miss it.
//!
//! ## What positioning IS achievable
//!
//! * Cursor-anchored menus, anywhere in the mount element's coordinate space
//!   ([`with_context_menu`]).
//! * Exact viewport clamping — the menu's size is **deterministic**
//!   ([`menu_size`]) because rows are uniform-height buttons, so
//!   [`clamp_anchor`] cannot drift out of sync with the real layout.
//! * Centred, blocking modals with a dimmed scrim ([`modal`]), and the
//!   same modal placed at an explicit clamped top-left position
//!   ([`modal_at`], [TASK-017b1] — the cursor-anchored small dialogs).
//!
//! ## What is NOT achievable with built-ins (and the workaround)
//!
//! * **The right-press point cannot travel with the message.**
//!   `MouseArea::on_right_press` takes a fixed `Message` and no closure
//!   (`mouse_area.rs:68-71`), unlike `on_move`. The anchor therefore has to
//!   be *state the caller holds*: track the cursor with `on_move`, then read
//!   it back when the right-press message is handled.
//!   [`context_menu_trigger`] wires exactly that pair.
//! * **`on_move` positions are widget-relative** (`mouse_area.rs:358-360`),
//!   not window-absolute. To get a window-absolute anchor, wrap the
//!   full-window root in the trigger (its bounds *are* the viewport), or add
//!   the wrapped widget's known offset. A menu mounted on the same element
//!   that produced the anchor needs no conversion at all.
//! * **No submenu nesting.** `Menu` is single-level and DEC-005 rules out
//!   `iced_aw`; the same flattening the hamburger menu already uses
//!   (`panels/top.rs:584-626`) applies to context menus.
//! * **No auto-flip above the anchor.** `Menu::overlay` does flip
//!   (`menu.rs:244-270`), but we are not using `Menu`. [`clamp_anchor`]
//!   clamps rather than flips; a caller wanting flip-above can compare
//!   [`menu_size`] against the space below the anchor itself.
//!
//! # Expected call shapes
//!
//! *Tree* ([TASK-012]) and *tabs* ([TASK-013]) — the panel builds
//! `Vec<MenuItem<Message>>` from its own action list and the app root mounts
//! it:
//!
//! ```text
//! // row widget (tree row / tab strip):
//! context_menu_trigger(row, Message::CursorMoved, Message::Tree(TreeMessage::ContextMenuOpened { path }))
//!
//! // app root view:
//! match slot {
//!     OverlaySlot::ContextMenu { anchor } =>
//!         with_context_menu(root, *anchor, &tree_menu_items, window_size, Message::Tree(TreeMessage::ContextMenuClosed)),
//!     _ => root,
//! }
//! ```
//!
//! *Editor* ([TASK-014]) — the blocking save/cancel dialog:
//!
//! ```text
//! // app root view:
//! stack(vec![root, modal(dialog_card, Message::Editor(EditorMessage::CancelRequested))])
//!
//! // dialog card footer — `buffer_is_valid` gates the confirm button:
//! confirm_cancel_buttons(strings::SAVE_BUTTON, buffer_is_valid, |outcome| {
//!     Message::Editor(match outcome {
//!         ConfirmCancel::Confirm => EditorMessage::SaveRequested { path },
//!         ConfirmCancel::Cancel => EditorMessage::CancelRequested,
//!     })
//! })
//! ```
//!
//! Escape is wired by the caller: the reducer calls
//! [`OverlaySlot::close`] on the Escape key and uses the returned `bool` to
//! decide whether the key was actually consumed.
//!
//! Unit tests live in the sibling `overlay_tests.rs` sidecar.

#[cfg(test)]
#[path = "overlay_tests.rs"]
mod tests;

use std::fmt;

use iced::widget::{Column, MouseArea, Row, button, container, mouse_area, space, stack, text};
use iced::{Alignment, Background, Border, Element, Length, Padding, Point, Size};

use crate::ui::strings;
use crate::ui::widgets::DialogAction;
use crate::ui_iced::theme::{scrim_color, to_iced_color};

/// Width of a rendered context-menu card ([TASK-017a]).
pub const MENU_WIDTH: f32 = 200.0;

/// Height of one rendered context-menu row ([TASK-017a]).
pub const MENU_ROW_HEIGHT: f32 = 24.0;

/// Vertical gap between adjacent context-menu rows ([TASK-017a]).
pub const MENU_ROW_SPACING: f32 = 2.0;

/// Padding inside the context-menu card ([TASK-017a]).
pub const MENU_PADDING: f32 = 4.0;

/// Gap left between the anchor point and the menu card so the cursor does
/// not sit on the first row ([TASK-017a]).
pub const MENU_ANCHOR_GAP: f32 = 2.0;

/// Padding inside a modal dialog card ([TASK-017a]).
pub const MODAL_PADDING: f32 = 16.0;

/// Width of the card border ([TASK-017a]).
pub const CARD_BORDER_WIDTH: f32 = 1.0;

/// Corner radius of the card border ([TASK-017a]).
pub const CARD_RADIUS: f32 = 4.0;

/// Horizontal gap between the confirm and cancel buttons ([TASK-017a]).
pub const CONFIRM_CANCEL_SPACING: f32 = 8.0;

/// The single overlay slot an app holds ([TASK-017a]).
///
/// `Kind` is the caller's own modal vocabulary — the tree, tabs and editor
/// each define what modals exist, while the primitive only owns the
/// scrim/positioning/dismissal behaviour. Only one overlay can be open at a
/// time, which is what makes "is anything blocking input?" a single check.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OverlaySlot<Kind> {
    /// No overlay is open.
    Closed,
    /// A cursor-anchored context menu opened at `anchor`.
    ContextMenu {
        /// The point the menu is anchored to, in the coordinate space of the
        /// element the overlay is mounted on.
        anchor: Point,
    },
    /// A blocking modal dialog of the caller's own kind.
    Modal(Kind),
}

/// Hand-written rather than derived on purpose: `#[derive(Default)]` would add
/// a `Kind: Default` bound, and the whole point of the generic `Kind` is that
/// a panel's modal vocabulary does not have to declare a default member.
#[allow(clippy::derivable_impls, reason = "derive would require Kind: Default")]
impl<Kind> Default for OverlaySlot<Kind> {
    fn default() -> Self {
        Self::Closed
    }
}

impl<Kind> OverlaySlot<Kind> {
    /// A slot with nothing open.
    #[must_use]
    pub fn closed() -> Self {
        Self::Closed
    }

    /// Whether nothing is open.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        matches!(self, Self::Closed)
    }

    /// Whether a context menu is the active overlay.
    #[must_use]
    pub fn is_context_menu(&self) -> bool {
        matches!(self, Self::ContextMenu { .. })
    }

    /// Whether a blocking modal is the active overlay.
    #[must_use]
    pub fn is_modal(&self) -> bool {
        matches!(self, Self::Modal(_))
    }

    /// The anchor of the open context menu, `None` for every other state.
    #[must_use]
    pub fn anchor(&self) -> Option<Point> {
        match self {
            Self::ContextMenu { anchor } => Some(*anchor),
            Self::Closed | Self::Modal(_) => None,
        }
    }

    /// The kind of the open modal, `None` for every other state.
    #[must_use]
    pub fn modal_kind(&self) -> Option<&Kind> {
        match self {
            Self::Modal(kind) => Some(kind),
            Self::Closed | Self::ContextMenu { .. } => None,
        }
    }

    /// Opens a context menu anchored at `anchor`.
    ///
    /// Refused while a blocking [`Self::Modal`] is up — a modal owns the
    /// input, so a context menu must not open behind it. Returns whether the
    /// request was accepted.
    pub fn open_context_menu(&mut self, anchor: Point) -> bool {
        if self.is_modal() {
            return false;
        }
        *self = Self::ContextMenu { anchor };
        true
    }

    /// Opens a modal, replacing whatever overlay was open.
    pub fn open_modal(&mut self, kind: Kind) {
        *self = Self::Modal(kind);
    }

    /// Closes the active overlay.
    ///
    /// This is the dismissal entry point for both click-outside and Escape:
    /// the widget emits its dismiss `Message`, the reducer calls this.
    /// Returns whether an overlay was actually open, which lets a caller
    /// distinguish "Escape closed the menu" from "Escape did nothing" (and
    /// therefore not swallow the key event).
    pub fn close(&mut self) -> bool {
        let was_open = !self.is_closed();
        *self = Self::Closed;
        was_open
    }
}

/// One row of a context menu ([TASK-017a]).
///
/// `Display` is the row label, so the same type serves as the menu's model
/// and as what a test asserts against. Labels must come from
/// `crate::ui::strings` ([GUD-002]) — the primitive never invents one.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuItem<Message> {
    /// The label rendered on the row.
    pub label: String,
    /// The message produced when the row is activated.
    pub message: Message,
}

impl<Message> MenuItem<Message> {
    /// A row labelled `label` that emits `message`.
    #[must_use]
    pub fn new(label: impl Into<String>, message: Message) -> Self {
        Self {
            label: label.into(),
            message,
        }
    }

    /// Maps the row's message into another message type, keeping the label.
    #[must_use]
    pub fn map<Other>(self, f: impl FnOnce(Message) -> Other) -> MenuItem<Other> {
        MenuItem {
            label: self.label,
            message: f(self.message),
        }
    }
}

impl<Message> fmt::Display for MenuItem<Message> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// The row labels a menu projects, in render order ([TASK-017a]).
///
/// This is the `MenuItem` → visible-row projection; a menu with `n` items
/// always renders exactly `n` rows.
#[must_use]
pub fn menu_labels<Message>(items: &[MenuItem<Message>]) -> Vec<String> {
    items.iter().map(|item| item.label.clone()).collect()
}

/// The row index carrying `message`, if any row does ([TASK-017a]).
///
/// Lets a reducer recover which row produced a message without the widget
/// having to encode the index itself.
#[must_use]
pub fn menu_index_of<Message: PartialEq>(
    items: &[MenuItem<Message>],
    message: &Message,
) -> Option<usize> {
    items.iter().position(|item| &item.message == message)
}

/// The deterministic rendered size of a context-menu card ([TASK-017a]).
///
/// Rows are uniform-height buttons, so this is exact rather than an
/// estimate — which is what makes [`clamp_anchor`] safe to run before the
/// card has been laid out.
#[must_use]
pub fn menu_size(item_count: usize) -> Size {
    let rows = item_count as f32;
    let spacing = if item_count > 1 {
        MENU_ROW_SPACING * (rows - 1.0)
    } else {
        0.0
    };
    Size::new(
        MENU_WIDTH,
        MENU_ROW_HEIGHT * rows + spacing + MENU_PADDING * 2.0,
    )
}

/// Clamps `anchor` so a `size` box placed at it stays inside `viewport`
/// ([TASK-017a]).
///
/// The lower bound wins when the viewport is smaller than the box, so the
/// result is always `>= 0` and the card's top-left corner is on screen.
#[must_use]
pub fn clamp_anchor(anchor: Point, size: Size, viewport: Size) -> Point {
    let max_x = (viewport.width - size.width).max(0.0);
    let max_y = (viewport.height - size.height).max(0.0);

    Point::new(anchor.x.clamp(0.0, max_x), anchor.y.clamp(0.0, max_y))
}

/// The outcome of the shared confirm/cancel button pair ([TASK-017a]).
///
/// The iced re-typing of [`DialogAction`]: the egui helper *returns* the
/// clicked button, whereas iced buttons *emit* messages, so the outcome is
/// handed to a mapping closure instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmCancel {
    /// The confirm button (OK / Save / Process).
    Confirm,
    /// The cancel button ([`strings::CANCEL_BUTTON`]).
    Cancel,
}

impl ConfirmCancel {
    /// The egui-parity [`DialogAction`] for this outcome.
    #[must_use]
    pub fn to_dialog_action(self) -> DialogAction {
        match self {
            Self::Confirm => DialogAction::Confirm,
            Self::Cancel => DialogAction::Cancel,
        }
    }
}

/// The `(confirm, cancel)` message pair a mapping closure produces
/// ([TASK-017a]).
///
/// Split out so the button pair and its tests read from exactly one place.
#[must_use]
pub fn confirm_cancel_pair<Message>(on: impl Fn(ConfirmCancel) -> Message) -> (Message, Message) {
    (on(ConfirmCancel::Confirm), on(ConfirmCancel::Cancel))
}

/// The press handler the confirm button gets for a given enabled flag
/// ([TASK-017a]).
///
/// `Some` when the dialog is valid and confirming is allowed, `None` when it
/// is not — `Button::on_press_maybe` renders a `None` handler as an inert
/// button (`button.rs:170-173`), which is how the egui helper's
/// `add_enabled(confirm_enabled, ..)` behaves
/// (`src/app/ui/widgets.rs:184-189`).
#[must_use]
pub fn confirm_press<Message>(confirm_enabled: bool, message: Message) -> Option<Message> {
    confirm_enabled.then_some(message)
}

/// Renders the shared confirm + cancel button pair ([TASK-017a]).
///
/// Mirrors `crate::ui::widgets::confirm_cancel_buttons`
/// (`src/app/ui/widgets.rs:178-194`): the confirm label varies per dialog,
/// the cancel label is always [`strings::CANCEL_BUTTON`], and
/// `confirm_enabled == false` leaves the confirm button inert (no message)
/// while cancel still works.
#[must_use]
pub fn confirm_cancel_buttons<'a, Message>(
    confirm_label: &'a str,
    confirm_enabled: bool,
    on: impl Fn(ConfirmCancel) -> Message + 'a,
) -> Row<'a, Message>
where
    Message: Clone + 'a,
{
    let (confirm, cancel) = confirm_cancel_pair(&on);

    Row::with_children(vec![
        button(text(confirm_label))
            .on_press_maybe(confirm_press(confirm_enabled, confirm))
            .into(),
        button(text(strings::CANCEL_BUTTON)).on_press(cancel).into(),
    ])
    .spacing(CONFIRM_CANCEL_SPACING)
    .align_y(Alignment::Center)
}

/// A full-area, transparent layer that swallows the click landing on it
/// ([TASK-017a]).
///
/// `Interaction::Hidden` keeps the cursor from reporting hover to layers
/// below, and `on_press` captures the event (`mouse_area.rs:376-379`) so
/// the base tree never sees it (`stack.rs:262-264`).
#[must_use]
pub fn input_blocker<'a, Message>(on_click: Message) -> MouseArea<'a, Message>
where
    Message: 'a,
{
    mouse_area(full_area())
        .on_press(on_click)
        .interaction(iced::mouse::Interaction::Hidden)
}

/// A full-area, dimmed layer that swallows the click landing on it — the
/// modal scrim ([TASK-017a]).
#[must_use]
pub fn scrim_blocker<'a, Message>(on_click: Message) -> MouseArea<'a, Message>
where
    Message: 'a,
{
    mouse_area(
        container(full_area())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(scrim_style),
    )
    .on_press(on_click)
    .interaction(iced::mouse::Interaction::Hidden)
}

/// A `Space` stretched to fill its parent.
fn full_area() -> space::Space {
    space::Space::new().width(Length::Fill).height(Length::Fill)
}

/// Paints the scrim ([`scrim_color`]) over its whole bounds.
fn scrim_style(_theme: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(scrim_color())),
        ..container::Style::default()
    }
}

/// The card chrome shared by the context menu and the modal dialog.
fn card_style(_theme: &iced::Theme) -> container::Style {
    let palette = crate::ui::theme::Palette::fastmd_dark();

    container::Style {
        background: Some(Background::Color(to_iced_color(palette.surface_variant))),
        border: Border {
            color: to_iced_color(palette.border_strong),
            width: CARD_BORDER_WIDTH,
            radius: CARD_RADIUS.into(),
        },
        ..container::Style::default()
    }
}

/// The context-menu card: one button per item ([TASK-017a]).
///
/// Each row emits its own [`MenuItem::message`] on press, so no external
/// hover state is needed.
#[must_use]
pub fn context_menu_card<'a, Message>(items: &'a [MenuItem<Message>]) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let rows = items.iter().fold(
        Column::<Message>::new().spacing(MENU_ROW_SPACING),
        |rows, item| {
            rows.push(
                button(text(item.label.clone()))
                    .width(Length::Fill)
                    .on_press(item.message.clone()),
            )
        },
    );

    container(rows)
        .width(Length::Fixed(MENU_WIDTH))
        .padding(MENU_PADDING)
        .style(card_style)
        .into()
}

/// Mounts a context menu over `base`, anchored at `anchor` ([TASK-017a]).
///
/// Layers, bottom to top: `base`, a transparent [`input_blocker`], then the
/// menu card pushed to the clamped anchor. A click that misses the card hits
/// the blocker and emits `on_close`; a click on a row emits that row's
/// message and never reaches the blocker, because `Stack::update` stops at
/// the first capture (`stack.rs:262-264`).
///
/// `viewport` is the size of the area `base` fills — the anchor is clamped
/// against it so the card can never leave the screen. An empty `items` list
/// renders `base` unchanged rather than an empty card.
#[must_use]
pub fn with_context_menu<'a, Message>(
    base: Element<'a, Message>,
    anchor: Point,
    items: &'a [MenuItem<Message>],
    viewport: Size,
    on_close: Message,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    if items.is_empty() {
        return base;
    }

    let position = clamp_anchor(
        Point::new(anchor.x + MENU_ANCHOR_GAP, anchor.y + MENU_ANCHOR_GAP),
        menu_size(items.len()),
        viewport,
    );

    stack(vec![
        base,
        input_blocker(on_close).into(),
        container(context_menu_card(items))
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(Padding {
                top: position.y,
                right: 0.0,
                bottom: 0.0,
                left: position.x,
            })
            .into(),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// A blocking modal overlay: scrim + centred dialog card ([TASK-017a]).
///
/// The returned element fills its parent, so the caller stacks it over the
/// rest of the UI (`stack(vec![root, modal(card, on_dismiss)])`). Any click
/// outside the card lands on the scrim and emits `on_dismiss`; clicks inside
/// reach the card's own widgets first.
///
/// Escape is not wired here — the caller maps its Escape key to the same
/// dismiss `Message` (or calls [`OverlaySlot::close`] directly).
#[must_use]
pub fn modal<'a, Message>(
    content: Element<'a, Message>,
    on_dismiss: Message,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let card: Element<'a, Message> = container(content)
        .padding(MODAL_PADDING)
        .style(card_style)
        .into();

    stack(vec![
        scrim_blocker(on_dismiss).into(),
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .into(),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// A blocking modal anchored at an explicit top-left `position` instead of
/// centred ([TASK-017b1]).
///
/// Same scrim + card chrome as [`modal`]; only the placement differs. The
/// card's top-left corner sits at `position`, which the caller resolves
/// with [`clamp_anchor`] (or a centre fallback) before mounting — the
/// shape the cursor-anchored small dialogs need, mirroring the egui
/// name-entry window's `fixed_pos` behaviour
/// (`src/app/ui/modals.rs:40-45`). Like [`modal`], any click outside the
/// card lands on the scrim and emits `on_dismiss`.
#[must_use]
pub fn modal_at<'a, Message>(
    content: Element<'a, Message>,
    position: Point,
    on_dismiss: Message,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let card: Element<'a, Message> = container(content)
        .padding(MODAL_PADDING)
        .style(card_style)
        .into();

    stack(vec![
        scrim_blocker(on_dismiss).into(),
        container(card)
            .width(Length::Fill)
            .height(Length::Fill)
            .padding(Padding {
                top: position.y,
                right: 0.0,
                bottom: 0.0,
                left: position.x,
            })
            .into(),
    ])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

/// Wraps `content` as a context-menu trigger ([TASK-017a]).
///
/// `MouseArea::on_right_press` cannot carry the cursor position
/// (`mouse_area.rs:68-71`), so the two halves are wired together here:
/// `on_move` keeps the caller's cursor state current, and `on_right_press`
/// asks for the menu. The reducer reads the tracked point when it handles the
/// press and stores it in [`OverlaySlot::ContextMenu { anchor }`].
///
/// Note the coordinate space: `on_move` reports the position **relative to
/// `content`'s bounds** (`mouse_area.rs:358-360`), not the window. Mount
/// the menu against the same element this wraps and no conversion is needed;
/// for a window-absolute anchor, wrap the full-window root.
#[must_use]
pub fn context_menu_trigger<'a, Message>(
    content: impl Into<Element<'a, Message>>,
    on_move: impl Fn(Point) -> Message + 'a,
    on_right_press: Message,
) -> MouseArea<'a, Message>
where
    Message: 'a,
{
    mouse_area(content)
        .on_move(on_move)
        .on_right_press(on_right_press)
}
