//! Shell navigation bar with menu triggers, active latch styling, ellipsis convention, and tooltip integration (`UIF-NAV-001`, `UIF-NAV-002`, `UIF-NAV-003`).
//!
//! The navigation bar anchors the application shell, hosting top-level menu triggers, optional branding,
//! and action items. Each trigger coordinates root menu activation through a toggle latch, holding active
//! background (`surface.active`) and perimeter (`border.strong`) styling while its menu tree remains open,
//! and equips tooltip explanations via framework-owned hover delay.
//!
//! Unit tests live in the sibling `navbar_tests.rs` sidecar.

use iced::alignment;
use iced::{Border, Color, Element, Length, Pixels, Shadow};

use crate::surface::tooltip::Tooltip;
use crate::token::{color::Palette, shape, size, space, typography};

/// Height of a standard navigation bar in pixels (`size.control.lg` - 36px).
pub const NAVBAR_HEIGHT_STANDARD: f32 = size::CONTROL_LG;

/// Height of a compact navigation bar in pixels (`size.control.md` - 32px).
pub const NAVBAR_HEIGHT_COMPACT: f32 = size::CONTROL_MD;

/// Height of a menu trigger button inside a compact navigation bar (28px - `size.control.sm`).
pub const TRIGGER_HEIGHT_COMPACT: f32 = size::CONTROL_SM;

/// Height of a menu trigger button inside a standard navigation bar (32px - `size.control.md`).
pub const TRIGGER_HEIGHT_STANDARD: f32 = size::CONTROL_MD;

/// Horizontal padding of a menu trigger button (8px = `space.sm`).
pub const TRIGGER_PADDING_HORIZONTAL: f32 = space::SM;

/// Visual appearance of a navigation bar trigger button (`UIF-NAV-001`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TriggerStyle {
    /// Fill color of the trigger button.
    pub background: Color,
    /// Perimeter border stroke.
    pub border: Border,
    /// Foreground text color.
    pub text_color: Color,
}

/// Resolve the visual style of a navigation bar menu trigger (`UIF-NAV-001`).
///
/// When `is_open` is true, the trigger latches into `surface.active` fill with a `1px border.strong`
/// perimeter and `text.strong` foreground, holding that state for as long as the root menu remains open.
#[must_use]
pub fn resolve_trigger_style(
    palette: &Palette,
    is_open: bool,
    is_hovered: bool,
    is_focused: bool,
) -> TriggerStyle {
    if is_open {
        TriggerStyle {
            background: palette.surface_active,
            border: Border::default()
                .color(palette.border_strong)
                .width(1.0)
                .rounded(shape::RADIUS_WIDGET),
            text_color: palette.text_strong,
        }
    } else if is_hovered || is_focused {
        TriggerStyle {
            background: palette.surface_hover,
            border: Border::default()
                .color(palette.border_subtle)
                .width(1.0)
                .rounded(shape::RADIUS_WIDGET),
            text_color: palette.text_primary,
        }
    } else {
        TriggerStyle {
            background: Color::TRANSPARENT,
            border: Border::default(),
            text_color: palette.text_primary,
        }
    }
}

/// A menu trigger definition in the navigation bar (`UIF-NAV-001`, `UIF-NAV-002`, `UIF-NAV-003`).
#[derive(Clone, Debug, PartialEq)]
pub struct NavBarMenuTrigger {
    /// Unique identifier for this trigger.
    pub id: String,
    /// Human-readable label displayed on the trigger button.
    pub label: String,
    /// Optional tooltip text describing the menu trigger.
    pub tooltip: Option<String>,
    /// Whether the associated root menu is currently open.
    pub is_open: bool,
    /// Whether this trigger button is disabled.
    pub disabled: bool,
    /// Prompt behaviour describing whether this trigger opens a secondary dialog or prompt (`UIF-NAV-002`, `UIF-PRIN-007`).
    pub prompt_behaviour: Option<crate::microcopy::action_label::PromptBehaviour>,
}

impl NavBarMenuTrigger {
    /// Create a new menu trigger with `id` and `label`.
    #[must_use]
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            tooltip: None,
            is_open: false,
            disabled: false,
            prompt_behaviour: None,
        }
    }

    /// Attach a tooltip text to this trigger (`UIF-NAV-003`).
    #[must_use]
    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Set whether the menu is open.
    #[must_use]
    pub fn with_open(mut self, is_open: bool) -> Self {
        self.is_open = is_open;
        self
    }

    /// Set whether this trigger is disabled.
    #[must_use]
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set the trigger's prompt behaviour (`UIF-NAV-002`, `UIF-PRIN-007`).
    #[must_use]
    pub fn with_prompt_behaviour(
        mut self,
        behaviour: crate::microcopy::action_label::PromptBehaviour,
    ) -> Self {
        self.prompt_behaviour = Some(behaviour);
        self
    }

    /// Set whether the trigger launches a dialog or prompt (`UIF-NAV-002`, `UIF-PRIN-007`).
    #[must_use]
    pub fn with_dialog_prompt(mut self, launches: bool) -> Self {
        self.prompt_behaviour = Some(if launches {
            crate::microcopy::action_label::PromptBehaviour::LaunchesPrompt
        } else {
            crate::microcopy::action_label::PromptBehaviour::Immediate
        });
        self
    }

    /// Validate the trigger's label or icon tooltip against the ellipsis convention (`UIF-NAV-002`, `UIF-PRIN-007`).
    ///
    /// If the trigger has a non-empty label, validates `label` with `check_action_ellipsis(label, behaviour)`.
    /// If the trigger is icon-only (e.g. empty label) and has a tooltip, validates `tooltip` with `check_action_ellipsis(tooltip, behaviour)`.
    pub fn validate_ellipsis(&self) -> Result<(), crate::invariant_gate::InvariantViolation> {
        let behaviour = self
            .prompt_behaviour
            .unwrap_or(crate::microcopy::action_label::PromptBehaviour::Immediate);
        if !self.label.is_empty() {
            crate::invariant_gate::check_action_ellipsis(&self.label, behaviour)
        } else if let Some(tooltip) = &self.tooltip {
            crate::invariant_gate::check_action_ellipsis(tooltip, behaviour)
        } else {
            Ok(())
        }
    }
}

/// Declarative state model for a navigation bar (`UIF-NAV-001`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NavBarModel {
    /// Optional brand or title string displayed on the far left.
    pub brand: Option<String>,
    /// Ordered list of menu triggers.
    pub triggers: Vec<NavBarMenuTrigger>,
    /// Currently open menu trigger id, if any.
    pub active_menu_id: Option<String>,
}

impl NavBarModel {
    /// Create an empty navigation bar model.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the optional brand label.
    #[must_use]
    pub fn with_brand(mut self, brand: impl Into<String>) -> Self {
        self.brand = Some(brand.into());
        self
    }

    /// Add a menu trigger.
    #[must_use]
    pub fn with_trigger(mut self, trigger: NavBarMenuTrigger) -> Self {
        self.triggers.push(trigger);
        self
    }

    /// Toggle a menu trigger by id.
    ///
    /// If the menu is already open, closes it (latch behavior). Otherwise, activates it.
    pub fn toggle_menu(&mut self, id: &str) {
        if self.active_menu_id.as_deref() == Some(id) {
            self.active_menu_id = None;
        } else {
            self.active_menu_id = Some(id.to_string());
        }

        for trigger in &mut self.triggers {
            trigger.is_open = self.active_menu_id.as_deref() == Some(&trigger.id);
        }
    }

    /// Close any open menu.
    pub fn close_menu(&mut self) {
        self.active_menu_id = None;
        for trigger in &mut self.triggers {
            trigger.is_open = false;
        }
    }
}

/// Navigation bar widget rendering the top-level shell bar (`UIF-NAV-001`, `UIF-NAV-003`).
pub struct NavBar<'a, Message> {
    model: &'a NavBarModel,
    palette: Palette,
    height: f32,
    on_toggle_menu: Option<Box<dyn Fn(String) -> Message + 'a>>,
}

impl<'a, Message> NavBar<'a, Message> {
    /// Create a new navigation bar from `model`.
    #[must_use]
    pub fn new(model: &'a NavBarModel) -> Self {
        Self {
            model,
            palette: Palette::dark(),
            height: NAVBAR_HEIGHT_STANDARD,
            on_toggle_menu: None,
        }
    }

    /// Set the color palette.
    #[must_use]
    pub fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// Use compact height (32px).
    #[must_use]
    pub fn compact(mut self) -> Self {
        self.height = NAVBAR_HEIGHT_COMPACT;
        self
    }

    /// Callback invoked when a trigger is pressed.
    #[must_use]
    pub fn on_toggle_menu(mut self, f: impl Fn(String) -> Message + 'a) -> Self {
        self.on_toggle_menu = Some(Box::new(f));
        self
    }
}

/// Build an element representation of a single menu trigger button, wrapping with [`Tooltip`]
/// if a tooltip description is present (`UIF-NAV-003`).
#[must_use]
pub fn navbar_trigger_element<'a, Message: 'a + Clone>(
    trigger: &NavBarMenuTrigger,
    palette: &Palette,
    on_toggle: Option<Message>,
) -> Element<'a, Message> {
    let is_open = trigger.is_open;
    let pal = *palette;
    let style = resolve_trigger_style(&pal, is_open, false, false);
    let label_text = trigger.label.clone();
    let text_color = style.text_color;

    let trigger_btn = iced::widget::button(
        iced::widget::text(label_text)
            .size(Pixels(typography::BODY.size))
            .color(text_color),
    )
    .padding(iced::Padding::new(space::XS).horizontal(TRIGGER_PADDING_HORIZONTAL))
    .style(move |_theme, status| {
        let is_hovered = matches!(status, iced::widget::button::Status::Hovered);
        let is_pressed = matches!(status, iced::widget::button::Status::Pressed);
        let s = resolve_trigger_style(&pal, is_open, is_hovered || is_pressed, false);
        iced::widget::button::Style {
            background: Some(s.background.into()),
            text_color: s.text_color,
            border: s.border,
            shadow: Shadow::default(),
            snap: true,
        }
    });

    let trigger_btn = if let Some(msg) = on_toggle {
        trigger_btn.on_press(msg)
    } else {
        trigger_btn
    };

    let el: Element<'a, Message> = trigger_btn.into();

    if let Some(tooltip_text) = &trigger.tooltip {
        Tooltip::new(el, tooltip_text.clone())
            .with_palette(*palette)
            .into()
    } else {
        el
    }
}

impl<'a, Message: 'a + Clone> From<NavBar<'a, Message>> for Element<'a, Message> {
    fn from(navbar: NavBar<'a, Message>) -> Self {
        let palette = navbar.palette;
        let mut row = iced::widget::Row::new()
            .spacing(space::XS)
            .align_y(alignment::Vertical::Center);

        if let Some(brand) = &navbar.model.brand {
            row = row.push(
                iced::widget::container(
                    iced::widget::text(brand.clone())
                        .size(Pixels(typography::BODY.size))
                        .color(palette.text_strong),
                )
                .padding(iced::Padding::new(0.0).horizontal(space::SM)),
            );
        }

        for trigger in &navbar.model.triggers {
            let msg = navbar
                .on_toggle_menu
                .as_ref()
                .map(|f| f(trigger.id.clone()));
            row = row.push(navbar_trigger_element(trigger, &palette, msg));
        }

        iced::widget::container(row)
            .width(Length::Fill)
            .height(Length::Fixed(navbar.height))
            .padding(iced::Padding::new(0.0).horizontal(space::SM))
            .style(move |_theme| iced::widget::container::Style {
                background: Some(palette.surface_raised.into()),
                border: Border::default().color(palette.border_subtle).width(1.0),
                text_color: Some(palette.text_primary),
                shadow: Shadow::default(),
                snap: true,
            })
            .into()
    }
}

#[cfg(test)]
#[path = "navbar_tests.rs"]
mod navbar_tests;
