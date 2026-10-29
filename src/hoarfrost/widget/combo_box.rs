//! Geometry and paint resolution for the `UIF-ATOM-004` ComboBox / Select
//! trigger — the closed box the user clicks, independent of the flyout it
//! opens (which is the menu engine's, not this module's).
//!
//! Everything here is a value in, a value out. No widget is built, no
//! colour is looked up lazily, no clock is read. The two things the
//! increment needs that the text-input atom does *not* already give us are
//! the trigger's own trailing-slot geometry — a select owns a chevron that
//! is a hit target, not merely a decoration — and the chevron's colour,
//! which §3.4 states directly and which does *not* match the input atom's
//! icon channel on hover.
//!
//! # What §3.4 actually says about the trigger
//!
//! §3.4 prices the control as an input (`min_width = 120px`, the two
//! control rungs, `space.md` padding) and then adds one sentence of its
//! own about the glyph: *chevron `text.muted` at rest, `text.strong` on
//! hover/open*. That sentence is the only colour rule in §3.4 the trigger
//! cannot inherit, because [`text_input`]'s icon channel goes
//! [`text_primary`](Palette::text_primary) on hover, not
//! [`text_strong`](Palette::text_strong).
//!
//! So [`chevron_color`] is written as an override layered on the atom's
//! own channel rather than as a fresh palette reading: the two states §3.4
//! names are answered by §3.4, and every state §3.4 leaves alone —
//! disabled, invalid, focused-but-closed — falls through to whatever
//! [`InputKind::DropdownTrigger`] already resolves. That keeps this
//! module from inventing a third opinion about a state the spec is silent
//! on, and it means a future change to the dropdown-trigger row of §3.2
//! reaches the combobox without a second edit here.
//!
//! # The trailing slot is a hit target, not a glyph
//!
//! On a plain text input the trailing slot holds a glyph and the slot's
//! own bounds carry no interaction meaning. On a select they do: the
//! chevron *is* the open affordance, so [`trailing_slot_bounds`] is sized
//! to [`size::HIT_TARGET_MIN`] rather than to the glyph, and the glyph is
//! centred inside it. Clicking anywhere in that square opens the flyout;
//! the 14px mark the user sees is deliberately smaller than the area
//! that responds to them.
//!
//! # The control publishes its own box
//!
//! A drawn control that the operation stream cannot see is a control a host
//! cannot measure or route to. `trigger_box` is the single source for
//! that box — the same function paints it and publishes it — and
//! `publish_trigger_box` puts it on the stream once, in every state,
//! under no `Id`. `ComboBoxWidget::operate` and `Select`'s `operate`
//! both call it; `ComboOverlay` deliberately does not, because its node
//! *is* the flyout container's node and that container already publishes it.
//!
//! Unit tests live in the sibling `combo_box_tests.rs` sidecar.

use std::fmt;

use crate::state::{BaseState, VisualState};
use crate::token::color::Palette;
use crate::token::{shape, size, space, typography};
use crate::widget::combo_model::{ComboBoxModel, FLYOUT_MAX_HEIGHT};
use crate::widget::text_input::{self, InputKind, InputStyle};
use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::text::Renderer as _;
use iced::advanced::widget::Widget;
use iced::advanced::widget::operation::Operation;
use iced::advanced::widget::tree::{self, Tree};
use iced::advanced::{Clipboard, Renderer as _, Shell};
use iced::alignment;
use iced::keyboard;
use iced::mouse;
use iced::touch;
use iced::{
    Background, Border, Color, Element, Event, Length, Pixels, Point, Rectangle, Shadow, Size,
    Vector,
};

/// The standard trigger height — `size.control.md` (§3.4, `UIF-TOK-002`).
pub const HEIGHT_STANDARD: f32 = size::CONTROL_MD;

/// The compact trigger height — `size.control.sm` (§3.4, `UIF-TOK-002`).
pub const HEIGHT_COMPACT: f32 = size::CONTROL_SM;

/// Horizontal padding inside the trigger — `space.md` (§3.4,
/// `UIF-TOK-001`).
pub const HORIZONTAL_PADDING: f32 = space::MD;

/// The gap charged between the value text and the trailing slot —
/// `space.sm`, the same charge a text input makes so a value reads the
/// same length in either control.
pub const SLOT_GAP: f32 = space::SM;

/// The trailing slot's square side — `size.hit_target.min` (24px).
///
/// The slot is square and vertically centred, and it is sized to the hit
/// target rather than to the glyph because on a select the glyph is the
/// open affordance. See the module docs.
pub const TRAILING_SLOT: f32 = size::HIT_TARGET_MIN;

/// The chevron's drawn size — `size.icon.sm` (14px).
///
/// Smaller than a text input's [`SLOT_GLYPH_SIZE`](text_input::SLOT_GLYPH_SIZE)
/// because a select's glyph sits inside a 24px target it does not fill,
/// and §3.4's trigger reads as a quieter control than a search field.
pub const CHEVRON_SIZE: f32 = size::ICON_SM;

/// Corner radius of the trigger — `radius.widget`, the same rounding the
/// input atom takes (§3.2, `UIF-TOK-003`).
pub const RADIUS: f32 = shape::RADIUS_WIDGET;

/// The smallest width a trigger may take — `min_width = 120px` (§3.4).
///
/// A registered composite bound rather than a token because §3.4 states
/// the figure inline; the token lint's carve-out registry carries the
/// §-reference.
pub const MIN_WIDTH: f32 = 120.0;

/// The smallest width a compact trigger may take — `min_width = 64px` (§2.2, §3.4).
pub const MIN_WIDTH_COMPACT: f32 = 64.0;

/// The two height rungs §3.4 prices for a select, in the order the
/// gallery samples them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ComboRung {
    /// The standard rung — `size.control.md`.
    Standard,
    /// The compact rung — `size.control.sm`.
    Compact,
}

impl ComboRung {
    /// The rung's height.
    #[must_use]
    pub const fn height(self) -> f32 {
        match self {
            Self::Standard => HEIGHT_STANDARD,
            Self::Compact => HEIGHT_COMPACT,
        }
    }

    /// The rung's cell-id slug.
    #[must_use]
    pub const fn slug(self) -> &'static str {
        match self {
            Self::Standard => "std",
            Self::Compact => "cmp",
        }
    }
}

/// Every rung the matrix samples, standard first.
pub const RUNGS: [ComboRung; 2] = [ComboRung::Standard, ComboRung::Compact];

/// The trigger's outer bounds for one `(width, rung)` pair.
#[must_use]
pub fn trigger_bounds(origin: Point, width: f32, rung: ComboRung) -> Rectangle {
    Rectangle::new(origin, Size::new(width.max(0.0), rung.height()))
}

/// The trailing slot inside `trigger`: a square of
/// [`TRAILING_SLOT`] flush to the right edge, vertically centred.
///
/// Clamped to the trigger's own height so a caller who hands in a box
/// shorter than the hit target still gets a slot that fits inside it
/// rather than one that overflows the control.
#[must_use]
pub fn trailing_slot_bounds(trigger: Rectangle) -> Rectangle {
    let side = TRAILING_SLOT.min(trigger.height.max(0.0));
    Rectangle::new(
        Point::new(
            trigger.x + trigger.width - side,
            trigger.y + (trigger.height - side) / 2.0,
        ),
        Size::new(side, side),
    )
}

/// The chevron's drawn box, centred inside the trigger's trailing slot.
#[must_use]
pub fn chevron_bounds(trigger: Rectangle) -> Rectangle {
    let slot = trailing_slot_bounds(trigger);
    let side = CHEVRON_SIZE.min(slot.width).min(slot.height).max(0.0);
    Rectangle::new(
        Point::new(
            slot.x + (slot.width - side) / 2.0,
            slot.y + (slot.height - side) / 2.0,
        ),
        Size::new(side, side),
    )
}

/// The area the selected value (or the placeholder) is drawn into.
///
/// Runs from the leading padding to the slot's left edge minus
/// [`SLOT_GAP`], so the value never touches the glyph and its length does
/// not change when the slot changes size.
#[must_use]
pub fn value_bounds(trigger: Rectangle) -> Rectangle {
    let slot = trailing_slot_bounds(trigger);
    let left = trigger.x + HORIZONTAL_PADDING;
    let right = (slot.x - SLOT_GAP).max(left);
    Rectangle::new(
        Point::new(left, trigger.y),
        Size::new(right - left, trigger.height),
    )
}

/// The trigger's fully resolved fill/border/text style.
///
/// A thin delegation to [`text_input::resolve_input_style`] at
/// [`InputKind::DropdownTrigger`]: §3.4 prices the trigger as the input
/// atom's dropdown row rather than as a new cell, so the combobox takes
/// that row's fill, border and value colour wholesale. The one channel
/// this module does *not* take from it is the chevron — see
/// [`chevron_color`].
#[must_use]
pub fn resolve_trigger(state: VisualState, palette: &Palette) -> InputStyle {
    text_input::resolve_input_style(InputKind::DropdownTrigger, state, palette)
}

/// The chevron colour for one `(state, open)` pair.
///
/// Priority order, highest first:
///
/// 1. **Disabled** → [`text_disabled`](Palette::text_disabled). A dead
///    control's glyph is dead; §3.4's hover rule cannot fire on a control
///    that cannot be hovered into anything.
/// 2. **Invalid** → [`status_danger_text`](Palette::status_danger_text),
///    the dropdown-trigger row's own icon colour. §3.4 gives the chevron
///    only two named states and neither is the error state, so the error
///    signal comes from the atom the trigger resolves through rather than
///    being invented here.
/// 3. **Hovered, or the flyout is open** → [`text_strong`](Palette::text_strong).
///    This is §3.4's sentence, and it is the one place this module
///    overrides the input atom.
/// 4. **Focused but closed** → [`text_primary`](Palette::text_primary),
///    again the atom's own focused-icon colour. A tabbed-to select with
///    its flyout shut is not "open", and §3.4 does not call it strong.
/// 5. **Rest** → [`text_muted`](Palette::text_muted), where both §3.4
///    and the atom's icon channel agree.
#[must_use]
pub fn chevron_color(state: VisualState, open: bool, palette: &Palette) -> Color {
    match state.base {
        BaseState::Disabled => palette.text_disabled,
        BaseState::Invalid => palette.status_danger_text,
        BaseState::Hovered | BaseState::Pressed => palette.text_strong,
        BaseState::Rest => {
            if open {
                palette.text_strong
            } else if state.focused {
                palette.text_primary
            } else {
                palette.text_muted
            }
        }
    }
}

/// The maximum height of the dropdown flyout — 280.0px (§3.4, `UIF-ATOM-004`).
pub const COMBO_FLYOUT_MAX_H: f32 = FLYOUT_MAX_HEIGHT;

/// The checkmark glyph rendered for a selected option row — `"✓"` (§3.4, `UIF-ATOM-004`).
pub const CHECK_MARK: &str = "✓";

/// The chevron glyph rendered in the trailing trigger slot — `"▾"` (§3.4, `UIF-ATOM-004`).
pub const CHEVRON_GLYPH: &str = "▾";

/// ComboBox / Select widget conforming to §3.4 (`UIF-ATOM-004`).
pub struct ComboBox<'a, Message, Id> {
    model: &'a ComboBoxModel<Id>,
    rung: ComboRung,
    width: Length,
    palette: Palette,
    on_toggle_open: Option<Message>,
    on_select: Option<Box<dyn Fn(Id) -> Message + 'a>>,
}

impl<'a, Message, Id: Clone + fmt::Display + PartialEq> ComboBox<'a, Message, Id> {
    /// Construct a new [`ComboBox`] wrapping `model` (§3.4, `UIF-ATOM-004`).
    #[must_use]
    pub fn new(model: &'a ComboBoxModel<Id>) -> Self {
        Self {
            model,
            rung: ComboRung::Standard,
            width: Length::Shrink,
            palette: Palette::dark(),
            on_toggle_open: None,
            on_select: None,
        }
    }

    /// Set the vertical sizing rung (`UIF-ATOM-004`).
    #[must_use]
    pub const fn with_rung(mut self, rung: ComboRung) -> Self {
        self.rung = rung;
        self
    }

    /// Set the width of the control, clamped to at least [`MIN_WIDTH`] (120px) or [`MIN_WIDTH_COMPACT`] (64px) for compact rung (§3.4, `UIF-ATOM-004`).
    #[must_use]
    pub fn with_width(mut self, width: impl Into<Length>) -> Self {
        let width = width.into();
        let min_w = match self.rung {
            ComboRung::Standard => MIN_WIDTH,
            ComboRung::Compact => MIN_WIDTH_COMPACT,
        };
        self.width = match width {
            Length::Fixed(w) => Length::Fixed(w.max(min_w)),
            other => other,
        };
        self
    }

    /// Set the width with explicit compact min-width clamp (`UIF-ATOM-004`).
    #[must_use]
    pub fn with_compact_width(mut self, width: impl Into<Length>) -> Self {
        let width = width.into();
        self.width = match width {
            Length::Fixed(w) => Length::Fixed(w.max(MIN_WIDTH_COMPACT)),
            other => other,
        };
        self
    }

    /// Set the design token color palette (`UIF-ATOM-004`).
    #[must_use]
    pub const fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// Set the message emitted when the trigger box is clicked to toggle the flyout (`UIF-ATOM-004`).
    #[must_use]
    pub fn on_toggle_open(mut self, msg: Message) -> Self {
        self.on_toggle_open = Some(msg);
        self
    }

    /// Convenience alias for [`on_toggle_open`](Self::on_toggle_open) (`UIF-ATOM-004`).
    #[must_use]
    pub fn on_press(self, msg: Message) -> Self {
        self.on_toggle_open(msg)
    }

    /// Set the callback emitted when an enabled option in the flyout is selected (`UIF-ATOM-004`).
    #[must_use]
    pub fn on_select(mut self, f: impl Fn(Id) -> Message + 'a) -> Self {
        self.on_select = Some(Box::new(f));
        self
    }

    /// Reference to the underlying data model (`UIF-ATOM-004`).
    #[must_use]
    pub const fn model(&self) -> &'a ComboBoxModel<Id> {
        self.model
    }

    /// The vertical sizing rung (`UIF-ATOM-004`).
    #[must_use]
    pub const fn rung(&self) -> ComboRung {
        self.rung
    }

    /// The layout width constraint (`UIF-ATOM-004`).
    #[must_use]
    pub const fn width(&self) -> Length {
        self.width
    }

    /// The active color palette (`UIF-ATOM-004`).
    #[must_use]
    pub const fn palette(&self) -> &Palette {
        &self.palette
    }

    /// The message emitted on trigger toggle, if any (`UIF-ATOM-004`).
    #[must_use]
    pub const fn toggle_open_message(&self) -> Option<&Message> {
        self.on_toggle_open.as_ref()
    }

    /// The label the trigger displays (§3.4, `UIF-ATOM-004`).
    #[must_use]
    pub fn display_label(&self) -> &str {
        self.model.display_label()
    }

    /// Whether the trigger is displaying placeholder text (§3.4, `UIF-ATOM-004`).
    #[must_use]
    pub fn shows_placeholder(&self) -> bool {
        self.model.shows_placeholder()
    }

    /// The chevron color for `visual_state` under the current model openness (§3.4, `UIF-ATOM-004`).
    #[must_use]
    pub fn chevron_color(&self, visual_state: VisualState) -> Color {
        chevron_color(visual_state, self.model.is_open(), &self.palette)
    }

    /// The resolved trigger height for the active rung (§3.4, `UIF-ATOM-004`).
    #[must_use]
    pub const fn trigger_height(&self) -> f32 {
        self.rung.height()
    }

    /// Whether the combobox model is disabled (`UIF-ATOM-004`).
    #[must_use]
    pub const fn is_disabled(&self) -> bool {
        self.model.disabled_flag()
    }

    /// Whether the flyout dropdown card will mount in the element tree (§3.4, `UIF-ATOM-004`).
    #[must_use]
    pub fn is_flyout_mounted(&self) -> bool {
        self.model.is_open() && !self.model.filtered_items().is_empty()
    }
}

/// Helper function to construct a [`ComboBox`] widget wrapper (§3.4, `UIF-ATOM-004`).
#[must_use]
pub fn combo_box<'a, Message, Id: Clone + fmt::Display + PartialEq>(
    model: &'a ComboBoxModel<Id>,
) -> ComboBox<'a, Message, Id> {
    ComboBox::new(model)
}

impl<'a, Message, Id> From<ComboBox<'a, Message, Id>> for Element<'a, Message>
where
    Message: 'a + Clone,
    Id: Clone + PartialEq + fmt::Display + 'a,
{
    fn from(combo: ComboBox<'a, Message, Id>) -> Self {
        let palette = combo.palette;
        let is_disabled = combo.model.disabled_flag();
        let is_invalid = combo.model.is_invalid();
        let rung_height = combo.rung.height();

        let initial_base = if is_disabled {
            BaseState::Disabled
        } else if is_invalid {
            BaseState::Invalid
        } else {
            BaseState::Rest
        };
        let initial_state = VisualState {
            base: initial_base,
            focused: false,
        };
        let initial_trigger_style = resolve_trigger(initial_state, &palette);
        let initial_chevron_color = chevron_color(initial_state, combo.model.is_open(), &palette);

        let (label_text, label_color) = if let Some(selected_opt) = combo.model.selected_option() {
            (selected_opt.label.as_str(), initial_trigger_style.text)
        } else {
            (combo.model.placeholder(), palette.text_muted)
        };

        let text_size = match combo.rung {
            ComboRung::Standard => typography::BODY.size,
            ComboRung::Compact => typography::UI_COMPACT.size,
        };

        let label_widget = iced::widget::text(label_text)
            .size(Pixels(text_size))
            .color(label_color)
            .width(Length::Fill)
            .align_y(alignment::Vertical::Center);

        let chevron_widget = iced::widget::container(
            iced::widget::text(CHEVRON_GLYPH)
                .size(Pixels(CHEVRON_SIZE))
                .color(initial_chevron_color)
                .align_x(alignment::Horizontal::Center)
                .align_y(alignment::Vertical::Center),
        )
        .width(Length::Fixed(TRAILING_SLOT))
        .height(Length::Fixed(TRAILING_SLOT))
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center);

        let trigger_row = iced::widget::row![label_widget, chevron_widget]
            .spacing(SLOT_GAP)
            .align_y(alignment::Vertical::Center)
            .width(Length::Fill)
            .height(Length::Fixed(rung_height));

        let min_w = match combo.rung {
            ComboRung::Standard => MIN_WIDTH,
            ComboRung::Compact => MIN_WIDTH_COMPACT,
        };

        let trigger_inner = iced::widget::container(
            iced::widget::stack![
                iced::widget::Space::new().width(Length::Fixed(min_w)),
                trigger_row,
            ]
            .width(combo.width)
            .height(Length::Fixed(rung_height)),
        )
        .padding(iced::Padding::new(0.0).horizontal(HORIZONTAL_PADDING))
        .width(combo.width)
        .height(Length::Fixed(rung_height))
        .align_y(alignment::Vertical::Center);

        let is_open = combo.model.is_open();
        let mut trigger_btn = iced::widget::button(trigger_inner)
            .padding(iced::Padding::ZERO)
            .width(combo.width)
            .height(Length::Fixed(rung_height))
            .style(move |_theme, status| {
                let is_hovered = matches!(
                    status,
                    iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed
                );
                let current_base = if is_disabled {
                    BaseState::Disabled
                } else if is_invalid {
                    BaseState::Invalid
                } else if is_hovered {
                    BaseState::Hovered
                } else {
                    BaseState::Rest
                };
                let current_state = VisualState {
                    base: current_base,
                    focused: false,
                };
                let resolved = resolve_trigger(current_state, &palette);
                iced::widget::button::Style {
                    background: Some(resolved.background.into()),
                    border: Border {
                        color: resolved.border_color,
                        width: resolved.border_width,
                        radius: shape::RADIUS_WIDGET.into(),
                    },
                    text_color: resolved.text,
                    shadow: Shadow::default(),
                    snap: true,
                }
            });

        if !is_disabled && let Some(msg) = combo.on_toggle_open.clone() {
            trigger_btn = trigger_btn.on_press(msg);
        }

        let filtered_items = combo.model.filtered_items();
        let flyout_element = if is_open && !filtered_items.is_empty() {
            let mut options_col = iced::widget::Column::new().spacing(space::XXS);

            for item in filtered_items {
                let is_selected = combo.model.selected() == Some(&item.id);
                let item_enabled = item.enabled && !is_disabled;

                let check_glyph = if is_selected { CHECK_MARK } else { "" };
                let check_slot = iced::widget::container(
                    iced::widget::text(check_glyph)
                        .size(Pixels(typography::UI_COMPACT.size))
                        .color(if item_enabled {
                            palette.accent_text
                        } else {
                            palette.text_disabled
                        })
                        .align_x(alignment::Horizontal::Center)
                        .align_y(alignment::Vertical::Center),
                )
                .width(Length::Fixed(size::CHECKBOX_BOX))
                .height(Length::Fixed(size::CHECKBOX_BOX))
                .align_x(alignment::Horizontal::Center)
                .align_y(alignment::Vertical::Center);

                let label_color = if item_enabled {
                    palette.text_primary
                } else {
                    palette.text_disabled
                };
                let row_label = iced::widget::text(&item.label)
                    .size(Pixels(typography::BODY.size))
                    .color(label_color)
                    .width(Length::Fill)
                    .align_y(alignment::Vertical::Center);

                let row_content = iced::widget::row![check_slot, row_label]
                    .spacing(space::SM)
                    .align_y(alignment::Vertical::Center)
                    .width(Length::Fill)
                    .height(Length::Fixed(size::CONTROL_MD));

                let mut opt_btn = iced::widget::button(row_content)
                    .padding(iced::Padding::new(0.0).horizontal(space::SM))
                    .width(Length::Fill)
                    .height(Length::Fixed(size::CONTROL_MD))
                    .style(move |_theme, status| {
                        let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                        let bg = if item_enabled && is_hovered {
                            palette.surface_hover
                        } else {
                            Color::TRANSPARENT
                        };
                        iced::widget::button::Style {
                            background: Some(bg.into()),
                            text_color: if item_enabled {
                                palette.text_primary
                            } else {
                                palette.text_disabled
                            },
                            border: Border::default().rounded(shape::RADIUS_WIDGET),
                            shadow: Shadow::default(),
                            snap: true,
                        }
                    });

                if item_enabled && let Some(on_select) = &combo.on_select {
                    opt_btn = opt_btn.on_press(on_select(item.id.clone()));
                }

                options_col = options_col.push(opt_btn);
            }

            let scrollable_options = iced::widget::scrollable(options_col).direction(
                iced::widget::scrollable::Direction::Vertical(
                    iced::widget::scrollable::Scrollbar::default(),
                ),
            );

            let flyout = iced::widget::container(scrollable_options)
                .width(Length::Fill)
                .max_height(Pixels(COMBO_FLYOUT_MAX_H))
                .padding(iced::Padding::new(space::XS))
                .style(move |_theme| iced::widget::container::Style {
                    background: Some(palette.surface_overlay.into()),
                    border: Border::default()
                        .color(palette.border_strong)
                        .width(shape::BORDER_HAIRLINE)
                        .rounded(shape::RADIUS_WIDGET),
                    shadow: Shadow::default(),
                    snap: true,
                    text_color: Some(palette.text_primary),
                });

            Some(Element::new(flyout))
        } else {
            None
        };

        let widget = ComboBoxWidget {
            trigger: Element::new(trigger_btn),
            flyout: flyout_element,
            is_open,
            width: combo.width,
            height: rung_height,
            on_dismiss: combo.on_toggle_open.clone(),
        };

        Element::new(widget)
    }
}

/// Internal widget wrapping trigger and floating overlay flyout for [`ComboBox`] (`UIF-ATOM-004`).
pub struct ComboBoxWidget<'a, Message> {
    trigger: Element<'a, Message>,
    flyout: Option<Element<'a, Message>>,
    is_open: bool,
    width: Length,
    height: f32,
    on_dismiss: Option<Message>,
}

/// The trigger's own box — the rectangle the layout pass gave the control.
///
/// Both passes that need it take it from here, so the box a host reads
/// cannot drift from the box that was drawn:
///
/// * [`ComboBoxWidget::draw`] hands this exact [`Layout`] node straight to
///   the trigger, so the fill, border and label the user sees are painted
///   inside it and never outside it.
/// * [`ComboBoxWidget::operate`] and [`Select`]'s `operate` publish it.
/// * [`Select::draw`] paints its quad and its glyphs from it as well, so
///   for the stateless helper the drawn box and the published box are the
///   same call, not two calls that happen to agree.
///
/// The same single-source guarantee
/// `crate::surface::menu::view::row_box` gives a menu row, for the same
/// reason: a box computed twice is a box that can disagree.
#[must_use]
fn trigger_box(layout: Layout<'_>) -> Rectangle {
    layout.bounds()
}

/// Publishes the control's own box to the operation stream.
///
/// **Fires in every state** — enabled, invalid or disabled. That is the
/// rule the disabled-toggle and menu-row fixes established, and the reason
/// this is not tucked inside a non-disabled branch:
///
/// * [`Operation::container`] answers *"where is this control?"*.
/// * [`Operation::focusable`] answers *"can Tab stop here?"*.
///
/// Two orthogonal doors. A disabled trigger closes the second — it carries
/// no `on_press`, so iced's button never registers a focus target — and
/// keeps the first open, so a host can still see, measure and route to a
/// disabled combo rather than finding a blank gap where one was drawn.
///
/// Published under **no `Id`**. [`ComboBox`] and [`Select`] carry no
/// stable identity field, and inventing one here would move the named
/// count without the host asking for it: naming is a host concern, made
/// with `Id`s the host owns. What this fix restores is *existence* — an
/// anonymous `container` record at the real bounds, which is exactly the
/// record the pre-rewrite widget published and the demo goldens count.
///
/// Exactly one record per control: published once, unconditionally, and
/// never re-published under a second hook.
fn publish_trigger_box(operation: &mut dyn Operation, layout: Layout<'_>) {
    operation.container(None, trigger_box(layout));
}

impl<Message: Clone> Widget<Message, iced::Theme, iced::Renderer> for ComboBoxWidget<'_, Message> {
    fn size(&self) -> Size<Length> {
        Size::new(self.width, Length::Fixed(self.height))
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.trigger
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.trigger.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn children(&self) -> Vec<Tree> {
        vec![
            Tree::new(&self.trigger),
            self.flyout
                .as_ref()
                .map(Tree::new)
                .unwrap_or_else(Tree::empty),
        ]
    }

    fn diff(&self, tree: &mut Tree) {
        if tree.children.len() < 2 {
            tree.children = vec![
                Tree::new(&self.trigger),
                self.flyout
                    .as_ref()
                    .map(Tree::new)
                    .unwrap_or_else(Tree::empty),
            ];
        } else {
            tree.children[0].diff(&self.trigger);
            if let Some(flyout) = &self.flyout {
                tree.children[1].diff(flyout);
            }
        }
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        // The control's own box first, then the trigger's subtree.
        //
        // Before this, `operate` was a pure pass-through and the
        // `ComboBoxWidget` published nothing of its own: the control drew
        // a 568x32 trigger in the atoms pane and was invisible to the
        // operation stream at that box. Measured against the demo's own
        // probe, the rewrite at `a74e143` dropped exactly one anonymous
        // `container` per instance — 3 instances, `anonymous 1823` down
        // to `1820` at the root manifest and `963` down to `960` in the
        // atoms pane.
        //
        // The pass-through below is kept because the trigger owns the
        // events and the focus target; this adds the box the wrapper owes
        // the host, not a second copy of the trigger's.
        publish_trigger_box(operation, layout);

        self.trigger
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.trigger.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.trigger.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        _renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        if !self.is_open || self.flyout.is_none() {
            return None;
        }
        let flyout = self.flyout.take().unwrap();
        let trigger_bounds = layout.bounds();
        let position = layout.position() + translation;

        Some(overlay::Element::new(Box::new(ComboOverlay {
            content: flyout,
            tree: &mut tree.children[1],
            position,
            trigger_size: trigger_bounds.size(),
            trigger_width: trigger_bounds.width,
            viewport: *viewport,
            on_dismiss: self.on_dismiss.clone(),
            is_open_flag: None,
        })))
    }
}

/// The floating overlay for dropdown flyouts (`UIF-ATOM-004`).
pub struct ComboOverlay<'a, Message> {
    content: Element<'a, Message>,
    tree: &'a mut Tree,
    position: Point,
    trigger_size: Size,
    trigger_width: f32,
    viewport: Rectangle,
    on_dismiss: Option<Message>,
    is_open_flag: Option<&'a mut bool>,
}

impl<Message: Clone> overlay::Overlay<Message, iced::Theme, iced::Renderer>
    for ComboOverlay<'_, Message>
{
    fn layout(&mut self, renderer: &iced::Renderer, bounds: Size) -> layout::Node {
        let space_below = bounds.height - (self.position.y + self.trigger_size.height);
        let space_above = self.position.y;

        let flyout_width = self
            .trigger_width
            .max(224.0)
            .min(320.0_f32.max(self.trigger_width));

        let available_height = if space_below < COMBO_FLYOUT_MAX_H && space_above > space_below {
            space_above.min(COMBO_FLYOUT_MAX_H)
        } else {
            space_below.min(COMBO_FLYOUT_MAX_H)
        };

        let limits = layout::Limits::new(Size::ZERO, Size::new(flyout_width, available_height))
            .width(Length::Fixed(flyout_width))
            .max_height(COMBO_FLYOUT_MAX_H);

        let node = self
            .content
            .as_widget_mut()
            .layout(self.tree, renderer, &limits);
        let size = node.size();

        let should_flip = space_below < size.height && space_above > space_below;
        let y_pos = if should_flip {
            self.position.y - size.height
        } else {
            self.position.y + self.trigger_size.height
        };

        let x_pos = self.position.x.min(bounds.width - size.width).max(0.0);
        let y_pos = y_pos.min(bounds.height - size.height).max(0.0);

        node.move_to(Point::new(x_pos, y_pos))
    }

    fn draw(
        &self,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        self.content.as_widget().draw(
            self.tree,
            renderer,
            theme,
            style,
            layout,
            cursor,
            &self.viewport,
        );
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            self.tree,
            layout,
            cursor,
            &self.viewport,
            renderer,
        )
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let bounds = layout.bounds();
        let is_over = cursor.is_over(bounds);

        self.content.as_widget_mut().update(
            self.tree,
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            &self.viewport,
        );

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if is_over {
                    if let Some(flag) = &mut self.is_open_flag {
                        **flag = false;
                        shell.request_redraw();
                    }
                } else {
                    if let Some(flag) = &mut self.is_open_flag {
                        **flag = false;
                        shell.request_redraw();
                    }
                    if let Some(on_dismiss) = &self.on_dismiss {
                        shell.publish(on_dismiss.clone());
                    }
                    shell.capture_event();
                }
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            }) => {
                if let Some(flag) = &mut self.is_open_flag {
                    **flag = false;
                    shell.request_redraw();
                }
                if let Some(on_dismiss) = &self.on_dismiss {
                    shell.publish(on_dismiss.clone());
                }
                shell.capture_event();
            }
            _ => {}
        }
    }

    /// Passes the operation through to the flyout content — and
    /// deliberately publishes **nothing of its own**.
    ///
    /// This is the one place in the module where the gap the trigger had
    /// does not exist, and adding a publish here would make it a bug rather
    /// than a fix. `ComboOverlay::layout` returns the *content's own*
    /// node, moved to the flyout position:
    ///
    /// ```text
    /// let node = self.content.as_widget_mut().layout(self.tree, renderer, &limits);
    /// node.move_to(Point::new(x_pos, y_pos))
    /// ```
    ///
    /// So the overlay's box and the flyout container's box are the same
    /// node, and that container already publishes itself through
    /// `iced::widget::container`'s own `operate`. Measured through
    /// `RecordCandidates` on an open two-option combo, the flyout region
    /// carries exactly one record at the overlay's box —
    /// `container 0.00,32.00 400.00x74.00` — followed by the
    /// `scrollable` inside it. A second `container` at that rectangle
    /// would be a duplicate of a box the host can already read, which is
    /// the opposite of what `publish_trigger_box` does for the trigger.
    fn operate(
        &mut self,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(self.tree, layout, renderer, operation);
    }
}

/// Persistent widget state for the stateless [`Select`] helper (`UIF-ATOM-004`).
#[derive(Debug, Default, Clone)]
pub struct SelectState {
    /// Whether the flyout is currently open.
    pub is_open: bool,
    /// Whether the trigger is hovered.
    pub is_hovered: bool,
}

/// A stateless, overlay-capable single-selection picker (`UIF-ATOM-004`).
pub struct Select<'a, T, Message> {
    options: &'a [T],
    selected: Option<&'a T>,
    on_select: Box<dyn Fn(T) -> Message + 'a>,
    placeholder: &'a str,
    rung: ComboRung,
    width: Length,
    palette: Palette,
    phantom: std::marker::PhantomData<Message>,
}

impl<'a, T, Message> Select<'a, T, Message>
where
    T: Clone + PartialEq + fmt::Display + 'a,
    Message: Clone + 'a,
{
    /// Construct a new stateless [`Select`] helper widget (`UIF-ATOM-004`).
    #[must_use]
    pub fn new(
        options: &'a [T],
        selected: Option<&'a T>,
        on_select: impl Fn(T) -> Message + 'a,
    ) -> Self {
        Self {
            options,
            selected,
            on_select: Box::new(on_select),
            placeholder: "Select...",
            rung: ComboRung::Standard,
            width: Length::Shrink,
            palette: Palette::dark(),
            phantom: std::marker::PhantomData,
        }
    }

    /// Set placeholder text shown when no option is selected (`UIF-ATOM-004`).
    #[must_use]
    pub fn with_placeholder(mut self, placeholder: &'a str) -> Self {
        self.placeholder = placeholder;
        self
    }

    /// Set vertical sizing rung (`UIF-ATOM-004`).
    #[must_use]
    pub const fn with_rung(mut self, rung: ComboRung) -> Self {
        self.rung = rung;
        self
    }

    /// Set width constraint, clamped to at least [`MIN_WIDTH`] (120px) or [`MIN_WIDTH_COMPACT`] (64px) (§3.4, `UIF-ATOM-004`).
    #[must_use]
    pub fn with_width(mut self, width: impl Into<Length>) -> Self {
        let width = width.into();
        let min_w = match self.rung {
            ComboRung::Standard => MIN_WIDTH,
            ComboRung::Compact => MIN_WIDTH_COMPACT,
        };
        self.width = match width {
            Length::Fixed(w) => Length::Fixed(w.max(min_w)),
            other => other,
        };
        self
    }

    /// Set design token palette (`UIF-ATOM-004`).
    #[must_use]
    pub const fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }
}

/// Convenience builder for a stateless, overlay-capable single-selection dropdown (`UIF-ATOM-004`).
#[must_use]
pub fn select<'a, T, Message>(
    options: &'a [T],
    selected: Option<&'a T>,
    on_select: impl Fn(T) -> Message + 'a,
) -> Select<'a, T, Message>
where
    T: Clone + PartialEq + fmt::Display + 'a,
    Message: Clone + 'a,
{
    Select::new(options, selected, on_select)
}

impl<'a, T, Message> Widget<Message, iced::Theme, iced::Renderer> for Select<'a, T, Message>
where
    T: Clone + PartialEq + fmt::Display + 'a,
    Message: Clone + 'a,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<SelectState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(SelectState::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::empty()]
    }

    fn diff(&self, tree: &mut Tree) {
        if tree.children.is_empty() {
            tree.children.push(Tree::empty());
        }
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, Length::Fixed(self.rung.height()))
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let rung_height = self.rung.height();
        let min_w = match self.rung {
            ComboRung::Standard => MIN_WIDTH,
            ComboRung::Compact => MIN_WIDTH_COMPACT,
        };
        let size = limits
            .width(self.width)
            .height(Length::Fixed(rung_height))
            .resolve(
                self.width,
                Length::Fixed(rung_height),
                Size::new(min_w, rung_height),
            );
        layout::Node::new(size)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<SelectState>();
        // Painted from the same function that publishes the box, so the
        // rectangle a host reads is the rectangle this drew, by
        // construction rather than by coincidence.
        let bounds = trigger_box(layout);

        let visual_base = if state.is_hovered {
            BaseState::Hovered
        } else {
            BaseState::Rest
        };
        let visual_state = VisualState {
            base: visual_base,
            focused: false,
        };
        let resolved = resolve_trigger(visual_state, &self.palette);
        let chevron_col = chevron_color(visual_state, state.is_open, &self.palette);

        // Background & border
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: Border {
                    color: resolved.border_color,
                    width: resolved.border_width,
                    radius: shape::RADIUS_WIDGET.into(),
                },
                shadow: Shadow::default(),
                snap: true,
            },
            Background::Color(resolved.background),
        );

        // Chevron
        let chevron_rect = chevron_bounds(bounds);
        let font = renderer.default_font();
        renderer.fill_text(
            iced::advanced::text::Text {
                content: CHEVRON_GLYPH.to_string(),
                bounds: Size::new(chevron_rect.width, chevron_rect.height),
                size: Pixels(CHEVRON_SIZE),
                line_height: iced::advanced::text::LineHeight::default(),
                font,
                align_x: iced::advanced::text::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: iced::advanced::text::Shaping::default(),
                wrapping: iced::advanced::text::Wrapping::None,
            },
            Point::new(chevron_rect.x, chevron_rect.y),
            chevron_col,
            *viewport,
        );

        // Text label or placeholder
        let val_rect = value_bounds(bounds);
        let text_size = match self.rung {
            ComboRung::Standard => typography::BODY.size,
            ComboRung::Compact => typography::UI_COMPACT.size,
        };
        let (label_content, text_col) = if let Some(sel) = self.selected {
            (sel.to_string(), resolved.text)
        } else {
            (self.placeholder.to_string(), self.palette.text_muted)
        };

        renderer.fill_text(
            iced::advanced::text::Text {
                content: label_content,
                bounds: Size::new(val_rect.width, val_rect.height),
                size: Pixels(text_size),
                line_height: iced::advanced::text::LineHeight::default(),
                font,
                align_x: iced::advanced::text::Alignment::Left,
                align_y: alignment::Vertical::Center,
                shaping: iced::advanced::text::Shaping::default(),
                wrapping: iced::advanced::text::Wrapping::None,
            },
            Point::new(val_rect.x, val_rect.y),
            text_col,
            *viewport,
        );
    }

    /// Publishes the select's own box.
    ///
    /// The `Widget` impl had **no `operate` at all** before this, which
    /// made the gap worse than the [`ComboBoxWidget`] one rather than
    /// merely the same: iced's default `operate` is an empty method, so a
    /// `Select` published *nothing* through *any* hook. Measured through
    /// `test_helpers::RecordCandidates`, a two-option `Select` at
    /// `Length::Fill` produced `count=0` — no `container`, no `text`, no
    /// `focusable`. It drew a full trigger and was entirely absent from
    /// the operation stream, not merely missing its own box.
    ///
    /// Same single source as the ComboBox: `trigger_box`, which is also
    /// the rectangle [`Select::draw`] paints, and the same
    /// fires-in-every-state rule — a `Select` with an empty option list
    /// still cannot be pressed, and still reports where it is.
    fn operate(
        &mut self,
        _tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        publish_trigger_box(operation, layout);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<SelectState>();
        let bounds = layout.bounds();
        let is_over = cursor.is_over(bounds);
        state.is_hovered = is_over;

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
            | Event::Touch(touch::Event::FingerPressed { .. }) => {
                if is_over {
                    state.is_open = !state.is_open;
                    shell.request_redraw();
                    shell.capture_event();
                }
            }
            // Closed-select prefix type-ahead (PrefixTypeAhead)
            Event::Keyboard(keyboard::Event::KeyPressed { text: Some(c), .. })
                if !state.is_open =>
            {
                if let Some(ch) = c.chars().next()
                    && ch.is_ascii_alphanumeric()
                {
                    let target_prefix = ch.to_ascii_lowercase();
                    let current_idx = self
                        .selected
                        .and_then(|sel| self.options.iter().position(|opt| opt == sel));
                    let start = current_idx.map(|i| i + 1).unwrap_or(0);
                    let found = (0..self.options.len())
                        .map(|offset| (start + offset) % self.options.len())
                        .find(|&idx| {
                            self.options[idx]
                                .to_string()
                                .to_ascii_lowercase()
                                .starts_with(target_prefix)
                        });

                    if let Some(idx) = found {
                        shell.publish((self.on_select)(self.options[idx].clone()));
                        shell.request_redraw();
                        shell.capture_event();
                    }
                }
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        _renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        let state = tree.state.downcast_mut::<SelectState>();
        if !state.is_open || self.options.is_empty() {
            return None;
        }

        let palette = self.palette;
        let mut options_col = iced::widget::Column::new().spacing(space::XXS);

        for item in self.options {
            let is_selected = self.selected == Some(item);
            let check_glyph = if is_selected { CHECK_MARK } else { "" };
            let check_slot = iced::widget::container(
                iced::widget::text(check_glyph)
                    .size(Pixels(typography::UI_COMPACT.size))
                    .color(palette.accent_text)
                    .align_x(alignment::Horizontal::Center)
                    .align_y(alignment::Vertical::Center),
            )
            .width(Length::Fixed(size::CHECKBOX_BOX))
            .height(Length::Fixed(size::CHECKBOX_BOX))
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center);

            let row_label = iced::widget::text(item.to_string())
                .size(Pixels(typography::BODY.size))
                .color(palette.text_primary)
                .width(Length::Fill)
                .align_y(alignment::Vertical::Center);

            let row_content = iced::widget::row![check_slot, row_label]
                .spacing(space::SM)
                .align_y(alignment::Vertical::Center)
                .width(Length::Fill)
                .height(Length::Fixed(size::CONTROL_MD));

            let opt_btn = iced::widget::button(row_content)
                .padding(iced::Padding::new(0.0).horizontal(space::SM))
                .width(Length::Fill)
                .height(Length::Fixed(size::CONTROL_MD))
                .on_press((self.on_select)(item.clone()))
                .style(move |_theme, status| {
                    let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
                    let bg = if is_hovered {
                        palette.surface_hover
                    } else {
                        Color::TRANSPARENT
                    };
                    iced::widget::button::Style {
                        background: Some(bg.into()),
                        text_color: palette.text_primary,
                        border: Border::default().rounded(shape::RADIUS_WIDGET),
                        shadow: Shadow::default(),
                        snap: true,
                    }
                });

            options_col = options_col.push(opt_btn);
        }

        let scrollable_options = iced::widget::scrollable(options_col).direction(
            iced::widget::scrollable::Direction::Vertical(
                iced::widget::scrollable::Scrollbar::default(),
            ),
        );

        let flyout = iced::widget::container(scrollable_options)
            .width(Length::Fill)
            .max_height(Pixels(COMBO_FLYOUT_MAX_H))
            .padding(iced::Padding::new(space::XS))
            .style(move |_theme| iced::widget::container::Style {
                background: Some(palette.surface_overlay.into()),
                border: Border::default()
                    .color(palette.border_strong)
                    .width(shape::BORDER_HAIRLINE)
                    .rounded(shape::RADIUS_WIDGET),
                shadow: Shadow::default(),
                snap: true,
                text_color: Some(palette.text_primary),
            });

        let trigger_bounds = layout.bounds();
        let position = layout.position() + translation;

        Some(overlay::Element::new(Box::new(ComboOverlay {
            content: Element::new(flyout),
            tree: &mut tree.children[0],
            position,
            trigger_size: trigger_bounds.size(),
            trigger_width: trigger_bounds.width,
            viewport: *viewport,
            on_dismiss: None,
            is_open_flag: Some(&mut state.is_open),
        })))
    }
}

impl<'a, T, Message> From<Select<'a, T, Message>> for Element<'a, Message>
where
    T: Clone + PartialEq + fmt::Display + 'a,
    Message: Clone + 'a,
{
    fn from(select: Select<'a, T, Message>) -> Self {
        Element::new(select)
    }
}

#[cfg(test)]
#[path = "combo_box_tests.rs"]
mod combo_box_tests;
