//! Low-level iced overlay chrome and modal primitives; application menus are
//! rendered by [`menu_surface`](crate::ui_iced::menu_surface) and its two
//! consumers, [`hamburger`](crate::ui_iced::hamburger) and
//! [`context_menu`](crate::ui_iced::context_menu).
//!
//! iced has no single call that produces either shape, so this module
//! supplies them as **pure view functions over plain state structs** — no
//! bus access, no side effects, no `FastMdApp` reads. The individual
//! dialogs (about / batch / tools / agent-debug / attributions) are built
//! on top of these primitives.
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
//! 2. **The built-in `overlay::menu::Menu` is not the application menu.** It
//!    takes caller-held mutable state and renders plain `ToString` rows, so
//!    it does not fit the pure-view pattern or the nested row types FastMD
//!    needs. The `iced-pro` menu engine owns placement and rendering from
//!    immutable menu data plus reducer state, mounted through
//!    [`menu_surface::overlay`](crate::ui_iced::menu_surface::overlay).
//! 3. **Event precedence is already what we want.** `Stack::update` walks
//!    children top-down and returns as soon as one captures the event
//!    (`stack.rs:262-264`), so the dialog card gets the click and the scrim
//!    only sees clicks that miss it.
//!
//! ## What positioning IS achievable
//!
//! * Viewport clamping is no longer this module's job. Every floating card
//!   here is placed by [`anchor::solve`](iced_pro::geometry::anchor::solve),
//!   which resolves the flip (menu cards) or the cursor-proximal clamp and
//!   `8px` viewport margin (input dialogs) before the card is laid out.
//!   The hand-rolled `clamp_anchor` this module used to own was retired in
//!   integration increment INT-4 for exactly that reason.
//! * Centred, blocking modals with a dimmed scrim ([`modal`]), and the
//!   same modal placed at an explicit clamped top-left position
//!   ([`modal_at`] — the cursor-anchored small dialogs).
//!
//! ## What is NOT achievable with built-ins (and the workaround)
//!
//! * **The right-press point cannot travel with the message.**
//!   `MouseArea::on_right_press` takes a fixed `Message` and no closure
//!   (`mouse_area.rs:68-71`), unlike `on_move`. The app therefore tracks the
//!   cursor on the full-window root and reads that root-space point when the
//!   right-press message is handled. [`context_menu_trigger`] emits only the
//!   row/tab open request.
//! * **`on_move` positions are widget-relative** (`mouse_area.rs:358-360`),
//!   not window-absolute. The app tracks pointer movement on its full-window
//!   content root so the shared menu's point anchor and overlay viewport use
//!   the same coordinate space. Individual row/tab triggers only request
//!   opening; they do not report local coordinates.
//! * **The built-in `Menu` has no submenu nesting.** FastMD's reusable
//!   engine ([`MenuCascade`](iced_pro::surface::menu::cascade::MenuCascade))
//!   supplies nested parent/child surfaces without adding `iced_aw`; the
//!   flat helpers in this module remain intentionally single-level.
//! * **No auto-flip above the anchor.** `Menu::overlay` does flip
//!   (`menu.rs:244-270`), but we are not using `Menu`. This module clamps
//!   nothing at all: [`anchor::solve`](iced_pro::geometry::anchor::solve)
//!   mirrors an axis when the preferred side cannot hold the card, and
//!   `Anchor::CursorProximal` deliberately never flips.
//!
//! # Expected call shapes
//!
//! *Tree* and *tabs* — the reducer snapshots the selected rows and the root
//! overlay host routes the shared widget's events back into the message loop:
//!
//! ```text
//! // row widget (tree row / tab strip):
//! context_menu_trigger(row, Message::Tree(TreeMessage::ContextMenuOpened { path }))
//!
//! // app root view (simplified):
//! menu_overlay(
//!     root.map(MenuEvent::Action), &context_menu.items, &context_menu.state,
//!     MenuPosition::Point { point: context_menu.anchor, .. }, viewport,
//!     MenuStyle::default(),
//! )
//! ```
//!
//! *Editor* — the blocking save/cancel dialog:
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

use iced::widget::{MouseArea, Row, button, container, mouse_area, space, stack, text};
use iced::{Alignment, Background, Border, Element, Length, Padding, Point};

use crate::ui::strings;
use crate::ui_iced::theme::scrim_color;

/// Padding inside a modal dialog card.
pub const MODAL_PADDING: f32 = 16.0;

/// Width of the card border.
pub const CARD_BORDER_WIDTH: f32 = 1.0;

/// Corner radius of the card border.
pub const CARD_RADIUS: f32 = 4.0;

/// Horizontal gap between the confirm and cancel buttons.
pub const CONFIRM_CANCEL_SPACING: f32 = 8.0;

/// Mouse interaction used by the modal scrim while it blocks the UI beneath it.
const SCRIM_INTERACTION: iced::mouse::Interaction = iced::mouse::Interaction::Idle;

/// Mouse interaction for the click-away blocker behind an open menu.
const MENU_BLOCKER_INTERACTION: iced::mouse::Interaction = iced::mouse::Interaction::Idle;

/// The single overlay slot an app holds.
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

/// The outcome of a confirm / cancel button pair.
///
/// The enum is **framework-free by construction** — three unit variants,
/// no framework type anywhere.
///
/// It is kept as a distinct type rather than folded into [`ConfirmCancel`]
/// because it carries a `None` member: `ConfirmCancel` has none, since an
/// iced button either emits a message or does not exist.
/// [`ConfirmCancel::to_dialog_action`] maps the two real outcomes across.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogAction {
    /// Neither button was clicked.
    None,
    /// The confirm button (OK / Save / Process / …) was clicked.
    Confirm,
    /// The cancel button was clicked.
    Cancel,
}

/// The outcome of the shared confirm/cancel button pair.
///
/// Iced buttons *emit* messages rather than *returning* the clicked
/// button, so the outcome is handed to a mapping closure instead.
/// [`Self::to_dialog_action`] maps onto [`DialogAction`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmCancel {
    /// The confirm button (OK / Save / Process).
    Confirm,
    /// The cancel button ([`strings::CANCEL_BUTTON`]).
    Cancel,
}

impl ConfirmCancel {
    /// The equivalent [`DialogAction`] for this outcome.
    #[must_use]
    pub fn to_dialog_action(self) -> DialogAction {
        match self {
            Self::Confirm => DialogAction::Confirm,
            Self::Cancel => DialogAction::Cancel,
        }
    }
}

/// The `(confirm, cancel)` message pair a mapping closure produces.
///
/// Split out so the button pair and its tests read from exactly one place.
#[must_use]
pub fn confirm_cancel_pair<Message>(on: impl Fn(ConfirmCancel) -> Message) -> (Message, Message) {
    (on(ConfirmCancel::Confirm), on(ConfirmCancel::Cancel))
}

/// The press handler the confirm button gets for a given enabled flag.
///
/// `Some` when the dialog is valid and confirming is allowed, `None` when
/// it is not. `Button::on_press_maybe` renders a `None` handler as an
/// inert button (`button.rs:170-173`).
#[must_use]
pub fn confirm_press<Message>(confirm_enabled: bool, message: Message) -> Option<Message> {
    confirm_enabled.then_some(message)
}

/// Renders the shared confirm + cancel button pair.
///
/// The confirm label varies per dialog, the cancel label is always
/// [`strings::CANCEL_BUTTON`], and `confirm_enabled == false` leaves the
/// confirm button inert (no message) while cancel still works.
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

/// A full-area, transparent layer that swallows the click landing on it.
///
/// `Interaction::Idle` leaves the default pointer visible, while `on_press`
/// still captures the event (`mouse_area.rs:376-379`) so the base tree never
/// sees it (`stack.rs:262-264`).
#[must_use]
pub fn input_blocker<'a, Message>(on_click: Message) -> MouseArea<'a, Message>
where
    Message: 'a,
{
    mouse_area(full_area())
        .on_press(on_click)
        .interaction(MENU_BLOCKER_INTERACTION)
}

/// A full-area, dimmed layer that swallows the click landing on it — the
/// modal scrim. The default cursor stays visible over the scrim.
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
    .interaction(SCRIM_INTERACTION)
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

/// The card chrome shared by the reusable menu widget and modal surfaces.
///
/// The style's colours come from the app palette.
pub(crate) fn card_style(_theme: &iced::Theme) -> container::Style {
    let palette = crate::ui::theme::Palette::fastmd_dark();

    container::Style {
        background: Some(Background::Color(palette.surface_variant)),
        border: Border {
            color: palette.border_strong,
            width: CARD_BORDER_WIDTH,
            radius: CARD_RADIUS.into(),
        },
        ..container::Style::default()
    }
}

/// A blocking modal overlay: scrim + centred dialog card.
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
/// centred.
///
/// Same scrim + card chrome as [`modal`]; only the placement differs. The
/// card's top-left corner sits at `position`, which the caller resolves
/// through [`iced_pro::geometry::anchor::solve`] before mounting — the
/// shape the cursor-anchored small dialogs need.
///
/// # The card's hit-capture boundary (`UIF-INP-002`)
///
/// The padded card is wrapped in a [`mouse_area`] that captures every
/// press landing inside the card perimeter and publishes `on_absorb`.
/// [`MouseArea`] updates its **content first** and only takes the event
/// itself when nothing below consumed it (`mouse_area.rs:225-241`), so
/// the card's live controls keep their presses exactly as before. What
/// falls through to the boundary is the padding ring, a **disabled**
/// button, and dead space between rows — and those presses stop there
/// instead of reaching the scrim. Clicking a disabled `Ok` therefore
/// cannot dismiss the dialog.
///
/// The wrap is layout-transparent: `MouseArea::size` and `::layout`
/// delegate straight to their content (`mouse_area.rs:185-199`), so the
/// capture area is exactly the visible card, padding included.
///
/// A click outside the card lands on the scrim and emits `on_dismiss`.
#[must_use]
pub fn modal_at<'a, Message>(
    content: Element<'a, Message>,
    position: Point,
    on_absorb: Message,
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
        container(mouse_area(card).on_press(on_absorb))
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

/// Wraps `content` as a context-menu trigger.
///
/// `MouseArea::on_right_press` cannot carry the cursor position
/// (`mouse_area.rs:68-71`), so the full-window content root tracks movement
/// separately and reports a root-space point. The reducer reads that point
/// when it handles this right-press request and stores it in
/// [`OverlaySlot::ContextMenu { anchor }`].
#[must_use]
pub fn context_menu_trigger<'a, Message>(
    content: impl Into<Element<'a, Message>>,
    on_right_press: Message,
) -> MouseArea<'a, Message>
where
    Message: 'a,
{
    mouse_area(content).on_right_press(on_right_press)
}
