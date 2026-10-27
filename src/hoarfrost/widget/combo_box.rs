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
//! Unit tests live in the sibling `combo_box_tests.rs` sidecar.

use std::fmt;

use crate::state::{BaseState, VisualState};
use crate::token::color::Palette;
use crate::token::{shape, size, space, typography};
use crate::widget::combo_model::{ComboBoxModel, FLYOUT_MAX_HEIGHT};
use crate::widget::text_input::{self, InputKind, InputStyle};
use iced::alignment;
use iced::{Border, Color, Element, Length, Pixels, Point, Rectangle, Shadow, Size};

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

    /// Set the width of the control, clamped to at least [`MIN_WIDTH`] (120px) (§3.4, `UIF-ATOM-004`).
    #[must_use]
    pub fn with_width(mut self, width: impl Into<Length>) -> Self {
        let width = width.into();
        self.width = match width {
            Length::Fixed(w) => Length::Fixed(w.max(MIN_WIDTH)),
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

        let trigger_inner = iced::widget::container(
            iced::widget::stack![
                iced::widget::Space::new().width(Length::Fixed(MIN_WIDTH)),
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

        if !is_disabled && let Some(msg) = combo.on_toggle_open {
            trigger_btn = trigger_btn.on_press(msg);
        }

        let mut root_col = iced::widget::column![trigger_btn].spacing(space::XS);

        let filtered_items = combo.model.filtered_items();
        if is_open && !filtered_items.is_empty() {
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
                .width(combo.width)
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

            root_col = root_col.push(flyout);
        }

        root_col.width(combo.width).into()
    }
}

#[cfg(test)]
#[path = "combo_box_tests.rs"]
mod combo_box_tests;
