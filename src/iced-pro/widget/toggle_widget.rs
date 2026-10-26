//! Live `iced::Widget`s for Boolean & Choice Controls: `Checkbox`, `RadioGroup`,
//! and `ToggleSwitch` (`UIF-ATOM-003`, §3.3).
//!
//! Unit tests live in the sibling `toggle_widget_tests.rs` sidecar.

use std::marker::PhantomData;

use crate::binding::{Commit, Controlled};
use crate::bridge::widget_id;
use crate::focus::FocusId;
use crate::state::{BaseState, VisualState};
use crate::token::color::Palette;
use crate::token::shape;
use crate::token::size;
use crate::token::space;
use crate::widget::focusable::FocusableState;
use crate::widget::toggle::{self, ToggleControl, TogglePolarity};
use crate::widget::toggle_model::{
    CheckState, CheckboxModel, ChildSummary, RadioGroupModel, RadioOption, RadioOrientation,
    SwitchModel, ToggleKey,
};

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text::{self as text_core, LineHeight, Paragraph as _};
use iced::advanced::widget::operation::{self, Operation};
use iced::advanced::widget::tree;
use iced::advanced::widget::{Id as WidgetId, Tree, Widget};
use iced::advanced::{Clipboard, Shell};
use iced::alignment;
use iced::keyboard;
use iced::keyboard::Key;
use iced::keyboard::key::Named;
use iced::mouse;
use iced::{Background, Element, Event, Length, Pixels, Point, Rectangle, Size};

/// The `✓` checkmark glyph rendered in a checked `Checkbox` (`UIF-ATOM-003`, §3.3).
pub const CHECK_MARK: &str = "✓";

/// The `−` (U+2212) minus/dash glyph rendered in an indeterminate `Checkbox` (`UIF-ATOM-003`, §3.3).
pub const INDETERMINATE_MARK: &str = "\u{2212}";

/// Optical size of the checkbox mark glyph (`UIF-ATOM-003`).
const CHECKBOX_MARK_SIZE: f32 = size::ICON_SM;

/// Closure type emitting a message on checkbox commit (`UIF-ATOM-003`).
pub type CheckboxEmit<'a, Message> = Box<dyn Fn(Commit<CheckState>) -> Option<Message> + 'a>;

/// Closure type emitting a message on standalone checkbox toggle (`UIF-ATOM-003`).
pub type CheckboxToggle<'a, Message> = Box<dyn Fn(CheckState) -> Option<Message> + 'a>;

/// Closure type emitting a message on radio option selection commit (`UIF-ATOM-003`).
pub type RadioEmit<'a, Message, Id> = Box<dyn Fn(Commit<Id>) -> Option<Message> + 'a>;

/// Closure type emitting a message on switch commit (`UIF-ATOM-003`).
pub type SwitchEmit<'a, Message> = Box<dyn Fn(Commit<bool>) -> Option<Message> + 'a>;

/// Closure type emitting a message on standalone switch toggle (`UIF-ATOM-003`).
pub type SwitchToggle<'a, Message> = Box<dyn Fn(bool) -> Option<Message> + 'a>;

/// Measure intrinsic label width using the text renderer (`UIF-ATOM-003`).
fn measure_text_width<Renderer: text_core::Renderer>(text: &str, renderer: &Renderer) -> f32 {
    if text.is_empty() {
        return 0.0;
    }
    let paragraph = <Renderer::Paragraph as text_core::Paragraph>::with_text(text_core::Text {
        content: text,
        bounds: Size::INFINITE,
        size: renderer.default_size(),
        line_height: text_core::LineHeight::default(),
        font: renderer.default_font(),
        align_x: text_core::Alignment::Left,
        align_y: alignment::Vertical::Top,
        shaping: text_core::Shaping::default(),
        wrapping: text_core::Wrapping::None,
    });
    let measured = paragraph.min_bounds().width;
    if measured > 0.0 {
        measured
    } else {
        text.len() as f32 * size::INDICATOR_DOT
    }
}

// ===========================================================================
// Checkbox (`UIF-ATOM-003`, §3.3)
// ===========================================================================

/// Ephemeral interaction and focus state of a [`Checkbox`] (`UIF-ATOM-003`).
#[derive(Debug, Default, PartialEq)]
pub struct CheckboxState {
    /// Focus router tracking.
    pub focusable: FocusableState,
    /// Whether pointer is hovering over bounds.
    pub hovered: bool,
    /// Whether left mouse button is pressed over bounds.
    pub pressed: bool,
}

/// The live, keyboard-focusable checkbox widget (`UIF-ATOM-003`, §3.3).
pub struct Checkbox<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    focus_id: FocusId,
    widget_id: WidgetId,
    palette: Palette,
    label: String,
    state: CheckState,
    enabled: bool,
    children: ChildSummary,
    model: Option<Controlled<'a, CheckboxModel>>,
    emit: Option<CheckboxEmit<'a, Message>>,
    on_toggle: Option<CheckboxToggle<'a, Message>>,
    _phantom: PhantomData<(Theme, Renderer)>,
}

impl<'a, Message, Theme, Renderer> Checkbox<'a, Message, Theme, Renderer> {
    /// Build a controlled checkbox over a host-owned [`CheckboxModel`] (`UIF-ATOM-003`, §3.3).
    #[must_use]
    pub fn controlled(
        model: Controlled<'a, CheckboxModel>,
        focus_id: impl Into<FocusId>,
        emit: impl Fn(Commit<CheckState>) -> Option<Message> + 'a,
    ) -> Self {
        let focus_id = focus_id.into();
        let widget_id = widget_id(&focus_id);
        let m = model.get();
        Self {
            focus_id,
            widget_id,
            palette: Palette::dark(),
            label: m.label.clone(),
            state: m.state,
            enabled: !m.disabled,
            children: ChildSummary::LEAF,
            model: Some(model),
            emit: Some(Box::new(emit)),
            on_toggle: None,
            _phantom: PhantomData,
        }
    }

    /// Build a standalone checkbox (`UIF-ATOM-003`, §3.3).
    #[must_use]
    pub fn standalone(
        label: impl Into<String>,
        state: CheckState,
        focus_id: impl Into<FocusId>,
        on_toggle: impl Fn(CheckState) -> Option<Message> + 'a,
    ) -> Self {
        let focus_id = focus_id.into();
        let widget_id = widget_id(&focus_id);
        Self {
            focus_id,
            widget_id,
            palette: Palette::dark(),
            label: label.into(),
            state,
            enabled: true,
            children: ChildSummary::LEAF,
            model: None,
            emit: None,
            on_toggle: Some(Box::new(on_toggle)),
            _phantom: PhantomData,
        }
    }

    /// Set child summary for tri-state transitions (`UIF-ATOM-003`).
    #[must_use]
    pub fn with_children(mut self, children: ChildSummary) -> Self {
        self.children = children;
        self
    }

    /// Set custom palette for paint resolution (`UIF-ATOM-003`).
    #[must_use]
    pub fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// Mark standalone checkbox as inert (`UIF-ATOM-003`).
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.enabled = !disabled;
        self
    }

    /// Whether this checkbox is interactive (`UIF-ATOM-003`).
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.model
            .as_ref()
            .map_or(self.enabled, |m| !m.get().disabled)
    }

    /// Current tri-state of the checkbox (`UIF-ATOM-003`).
    #[must_use]
    pub fn current_state(&self) -> CheckState {
        self.model.as_ref().map_or(self.state, |m| m.get().state)
    }

    /// The [`FocusId`] this checkbox answers to (`UIF-ATOM-003`).
    #[must_use]
    pub fn focus_id(&self) -> &FocusId {
        &self.focus_id
    }

    /// The iced [`WidgetId`] this checkbox registers with the router (`UIF-ATOM-003`).
    #[must_use]
    pub fn widget_id(&self) -> WidgetId {
        self.widget_id.clone()
    }

    /// The inline label string (`UIF-ATOM-003`).
    #[must_use]
    pub fn label_text(&self) -> &str {
        self.model.as_ref().map_or(&self.label, |m| &m.get().label)
    }

    fn perform_toggle(&self, shell: &mut Shell<'_, Message>) {
        if !self.is_enabled() {
            return;
        }

        if let Some(model) = &self.model {
            if let Some(commit) = model.get().toggle(self.children)
                && let Some(emit) = &self.emit
                && let Some(msg) = emit(commit)
            {
                shell.publish(msg);
            }
        } else if let Some(on_toggle) = &self.on_toggle {
            let next = self.state.next_with(self.children);
            if let Some(msg) = on_toggle(next) {
                shell.publish(msg);
            }
        }
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Checkbox<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: renderer::Renderer + text_core::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<CheckboxState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(CheckboxState::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![]
    }

    fn diff(&self, _tree: &mut Tree) {}

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Fixed(size::HIT_TARGET_MIN))
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let label_w = measure_text_width(self.label_text(), renderer);
        let (w, h) = toggle::hit_size(ToggleControl::Checkbox, label_w);
        let size = limits.resolve(Length::Shrink, Length::Fixed(h), Size::new(w, h));
        layout::Node::new(size)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_mut::<CheckboxState>();
        if self.is_enabled() {
            operation.focusable(Some(&self.widget_id), layout.bounds(), &mut state.focusable);
        } else {
            <FocusableState as operation::Focusable>::unfocus(&mut state.focusable);
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if shell.is_event_captured() {
            return;
        }

        let state = tree.state.downcast_mut::<CheckboxState>();
        let is_over = cursor.is_over(layout.bounds());
        state.hovered = is_over;

        if !self.is_enabled() {
            state.pressed = false;
            return;
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if is_over => {
                state.pressed = true;
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if state.pressed => {
                state.pressed = false;
                if is_over {
                    self.perform_toggle(shell);
                }
                shell.capture_event();
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: Key::Named(Named::Space),
                ..
            }) if state.focusable.is_focused() => {
                self.perform_toggle(shell);
                shell.capture_event();
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
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.is_enabled() && cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<CheckboxState>();
        let is_enabled = self.is_enabled();
        let base = if !is_enabled {
            BaseState::Disabled
        } else if state.pressed {
            BaseState::Pressed
        } else if state.hovered {
            BaseState::Hovered
        } else {
            BaseState::Rest
        };
        let visual = VisualState {
            base,
            focused: state.focusable.is_focused(),
        };

        let current_state = self.current_state();
        let polarity = current_state.polarity();
        let style = toggle::resolve_style(polarity, visual, &self.palette);
        let bounds = layout.bounds();

        let (ind_w, ind_h) = toggle::indicator_size(ToggleControl::Checkbox);
        let ind_origin = Point::new(bounds.x, bounds.y + (bounds.height - ind_h) / 2.0);
        let indicator = Rectangle::new(ind_origin, Size::new(ind_w, ind_h));

        // 1. Indicator box layer
        let ind_layer = toggle::indicator_layer(indicator, ToggleControl::Checkbox, &style);
        renderer.fill_quad(ind_layer.quad(), Background::Color(ind_layer.fill));

        // 2. Mark glyph layer
        let mark_glyph = match current_state {
            CheckState::Checked => Some(CHECK_MARK),
            CheckState::Indeterminate => Some(INDETERMINATE_MARK),
            CheckState::Unchecked => None,
        };
        if let Some(glyph) = mark_glyph {
            renderer.fill_text(
                text_core::Text {
                    content: glyph.to_owned(),
                    bounds: indicator.size(),
                    size: Pixels(CHECKBOX_MARK_SIZE),
                    line_height: LineHeight::Absolute(Pixels(indicator.height)),
                    font: renderer.default_font(),
                    align_x: text_core::Alignment::Center,
                    align_y: alignment::Vertical::Center,
                    shaping: text_core::Shaping::default(),
                    wrapping: text_core::Wrapping::None,
                },
                indicator.position(),
                style.mark,
                *viewport,
            );
        }

        // 3. Focus ring
        if let Some(ring) = toggle::focus_ring_layer(indicator, ToggleControl::Checkbox, &style) {
            renderer.fill_quad(ring.quad(), Background::Color(ring.fill));
        }

        // 4. Label text
        let label = self.label_text();
        if !label.is_empty() {
            let label_x = indicator.x + ind_w + toggle::LABEL_GAP;
            let label_y = bounds.y;
            let label_w = (bounds.width - (ind_w + toggle::LABEL_GAP)).max(0.0);
            let label_h = bounds.height;
            renderer.fill_text(
                text_core::Text {
                    content: label.to_owned(),
                    bounds: Size::new(label_w, label_h),
                    size: renderer.default_size(),
                    line_height: LineHeight::Absolute(Pixels(label_h)),
                    font: renderer.default_font(),
                    align_x: text_core::Alignment::Left,
                    align_y: alignment::Vertical::Center,
                    shaping: text_core::Shaping::default(),
                    wrapping: text_core::Wrapping::None,
                },
                Point::new(label_x, label_y),
                style.label,
                *viewport,
            );
        }
    }
}

impl<'a, Message, Theme, Renderer> From<Checkbox<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a,
    Renderer: 'a + renderer::Renderer + text_core::Renderer,
{
    fn from(checkbox: Checkbox<'a, Message, Theme, Renderer>) -> Self {
        Element::new(checkbox)
    }
}

// ===========================================================================
// RadioGroup (`UIF-ATOM-003`, §3.3)
// ===========================================================================

/// Ephemeral interaction and focus state of a [`RadioGroup`] (`UIF-ATOM-003`).
#[derive(Debug, Default, PartialEq)]
pub struct RadioGroupState {
    /// Focus router tracking.
    pub focusable: FocusableState,
    /// Currently hovered option index, if any.
    pub hovered: Option<usize>,
    /// Currently pressed option index, if any.
    pub pressed: Option<usize>,
    /// Focused option index within roving group, if any.
    pub focused_index: Option<usize>,
}

/// The live, roving-focus radio group widget (`UIF-ATOM-003`, §3.3).
pub struct RadioGroup<'a, Message, Id = String, Theme = iced::Theme, Renderer = iced::Renderer> {
    model: Controlled<'a, RadioGroupModel<Id>>,
    focus_id: FocusId,
    widget_id: WidgetId,
    emit: RadioEmit<'a, Message, Id>,
    palette: Palette,
    _phantom: PhantomData<(Theme, Renderer)>,
}

impl<'a, Message, Id, Theme, Renderer> RadioGroup<'a, Message, Id, Theme, Renderer> {
    /// Build a controlled radio group over a host-owned [`RadioGroupModel`] (`UIF-ATOM-003`, §3.3).
    #[must_use]
    pub fn controlled(
        model: Controlled<'a, RadioGroupModel<Id>>,
        focus_id: impl Into<FocusId>,
        emit: impl Fn(Commit<Id>) -> Option<Message> + 'a,
    ) -> Self {
        let focus_id = focus_id.into();
        let widget_id = widget_id(&focus_id);
        Self {
            model,
            focus_id,
            widget_id,
            emit: Box::new(emit),
            palette: Palette::dark(),
            _phantom: PhantomData,
        }
    }

    /// Set custom palette for paint resolution (`UIF-ATOM-003`).
    #[must_use]
    pub fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// The [`FocusId`] this radio group answers to (`UIF-ATOM-003`).
    #[must_use]
    pub fn focus_id(&self) -> &FocusId {
        &self.focus_id
    }

    /// The iced [`WidgetId`] this radio group registers with the router (`UIF-ATOM-003`).
    #[must_use]
    pub fn widget_id(&self) -> WidgetId {
        self.widget_id.clone()
    }
}

/// Calculate rectangles for each radio option (`UIF-ATOM-003`).
fn option_rectangles<Id>(
    bounds: Rectangle,
    orientation: RadioOrientation,
    options: &[RadioOption<Id>],
    label_widths: &[f32],
) -> Vec<Rectangle> {
    let mut rects = Vec::with_capacity(options.len());
    match orientation {
        RadioOrientation::Vertical => {
            for i in 0..options.len() {
                let y = bounds.y + i as f32 * size::HIT_TARGET_MIN;
                let lw = label_widths.get(i).copied().unwrap_or(0.0);
                let (opt_w, _) = toggle::hit_size(ToggleControl::Radio, lw);
                let w = opt_w.max(bounds.width);
                rects.push(Rectangle::new(
                    Point::new(bounds.x, y),
                    Size::new(w, size::HIT_TARGET_MIN),
                ));
            }
        }
        RadioOrientation::HorizontalWrap => {
            let mut x = bounds.x;
            let mut y = bounds.y;
            let max_x = bounds.x + bounds.width;
            for i in 0..options.len() {
                let lw = label_widths.get(i).copied().unwrap_or(0.0);
                let (opt_w, _) = toggle::hit_size(ToggleControl::Radio, lw);
                if x > bounds.x && max_x > bounds.x && x + opt_w > max_x {
                    x = bounds.x;
                    y += size::HIT_TARGET_MIN;
                }
                rects.push(Rectangle::new(
                    Point::new(x, y),
                    Size::new(opt_w, size::HIT_TARGET_MIN),
                ));
                x += opt_w + space::MD;
            }
        }
    }
    rects
}

impl<Message, Id, Theme, Renderer> Widget<Message, Theme, Renderer>
    for RadioGroup<'_, Message, Id, Theme, Renderer>
where
    Message: Clone,
    Id: Clone + PartialEq + 'static,
    Renderer: renderer::Renderer + text_core::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<RadioGroupState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(RadioGroupState::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![]
    }

    fn diff(&self, _tree: &mut Tree) {}

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Shrink)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let model = self.model.get();
        let label_widths: Vec<f32> = model
            .options
            .iter()
            .map(|o| measure_text_width(&o.label, renderer))
            .collect();

        let intrinsic = match model.orientation {
            RadioOrientation::Vertical => {
                let max_w = model
                    .options
                    .iter()
                    .zip(label_widths.iter())
                    .map(|(_, &lw)| toggle::hit_size(ToggleControl::Radio, lw).0)
                    .fold(size::HIT_TARGET_MIN, f32::max);
                let total_h =
                    (model.options.len() as f32 * size::HIT_TARGET_MIN).max(size::HIT_TARGET_MIN);
                Size::new(max_w, total_h)
            }
            RadioOrientation::HorizontalWrap => {
                let total_w = model
                    .options
                    .iter()
                    .zip(label_widths.iter())
                    .map(|(_, &lw)| toggle::hit_size(ToggleControl::Radio, lw).0)
                    .sum::<f32>()
                    + (model.options.len().saturating_sub(1) as f32 * space::MD);
                Size::new(total_w.max(size::HIT_TARGET_MIN), size::HIT_TARGET_MIN)
            }
        };

        let size = limits.resolve(Length::Shrink, Length::Shrink, intrinsic);
        layout::Node::new(size)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_mut::<RadioGroupState>();
        let model = self.model.get();
        if !model.disabled && !model.options.is_empty() {
            operation.focusable(Some(&self.widget_id), layout.bounds(), &mut state.focusable);
        } else {
            <FocusableState as operation::Focusable>::unfocus(&mut state.focusable);
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if shell.is_event_captured() {
            return;
        }

        let state = tree.state.downcast_mut::<RadioGroupState>();
        let model = self.model.get();

        if model.disabled {
            state.hovered = None;
            state.pressed = None;
            return;
        }

        let bounds = layout.bounds();
        let label_widths: Vec<f32> = model
            .options
            .iter()
            .map(|o| measure_text_width(&o.label, renderer))
            .collect();
        let option_rects =
            option_rectangles(bounds, model.orientation, &model.options, &label_widths);

        let hovered = cursor
            .position_over(bounds)
            .and_then(|pos| option_rects.iter().position(|rect| rect.contains(pos)));
        state.hovered = hovered;

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(idx) = hovered
                    && model.option_enabled(idx)
                {
                    state.pressed = Some(idx);
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if let Some(pressed_idx) = state.pressed.take() {
                    if hovered == Some(pressed_idx) && model.option_enabled(pressed_idx) {
                        state.focused_index = Some(pressed_idx);
                        let option = &model.options[pressed_idx];
                        if let Some(commit) = model.select(&option.id)
                            && let Some(msg) = (self.emit)(commit)
                        {
                            shell.publish(msg);
                        }
                    }
                    shell.capture_event();
                }
            }
            Event::Keyboard(keyboard::Event::KeyPressed { key, .. })
                if state.focusable.is_focused() =>
            {
                let current_focus = state
                    .focused_index
                    .or_else(|| model.selected_index())
                    .or_else(|| model.enabled_indices().first().copied());

                match key {
                    Key::Named(Named::ArrowUp) | Key::Named(Named::ArrowLeft) => {
                        if let Some(roving) =
                            model.rove_and_select(current_focus, ToggleKey::Backward)
                        {
                            state.focused_index = Some(roving.focus);
                            if let Some(msg) = (self.emit)(roving.selected) {
                                shell.publish(msg);
                            }
                            shell.capture_event();
                        }
                    }
                    Key::Named(Named::ArrowDown) | Key::Named(Named::ArrowRight) => {
                        if let Some(roving) =
                            model.rove_and_select(current_focus, ToggleKey::Forward)
                        {
                            state.focused_index = Some(roving.focus);
                            if let Some(msg) = (self.emit)(roving.selected) {
                                shell.publish(msg);
                            }
                            shell.capture_event();
                        }
                    }
                    Key::Named(Named::Space) => {
                        if let Some(idx) = current_focus
                            && model.option_enabled(idx)
                        {
                            let option = &model.options[idx];
                            if let Some(commit) = model.select(&option.id)
                                && let Some(msg) = (self.emit)(commit)
                            {
                                shell.publish(msg);
                            }
                            shell.capture_event();
                        }
                    }
                    _ => {}
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
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        let model = self.model.get();
        if !model.disabled && cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<RadioGroupState>();
        let model = self.model.get();
        let bounds = layout.bounds();

        let label_widths: Vec<f32> = model
            .options
            .iter()
            .map(|o| measure_text_width(&o.label, renderer))
            .collect();
        let option_rects =
            option_rectangles(bounds, model.orientation, &model.options, &label_widths);

        let current_focused = state
            .focused_index
            .or_else(|| model.selected_index())
            .or_else(|| model.enabled_indices().first().copied());

        for (i, option) in model.options.iter().enumerate() {
            let opt_bounds = option_rects[i];
            let opt_enabled = model.option_enabled(i);
            let is_selected = model.selected.as_ref() == Some(&option.id);
            let is_focused = state.focusable.is_focused() && current_focused == Some(i);

            let base = if !opt_enabled {
                BaseState::Disabled
            } else if state.pressed == Some(i) {
                BaseState::Pressed
            } else if state.hovered == Some(i) {
                BaseState::Hovered
            } else {
                BaseState::Rest
            };
            let visual = VisualState {
                base,
                focused: is_focused,
            };

            let polarity = if is_selected {
                TogglePolarity::On
            } else {
                TogglePolarity::Off
            };
            let style = toggle::resolve_style(polarity, visual, &self.palette);

            let (ind_w, ind_h) = toggle::indicator_size(ToggleControl::Radio);
            let ind_origin = Point::new(
                opt_bounds.x,
                opt_bounds.y + (opt_bounds.height - ind_h) / 2.0,
            );
            let indicator = Rectangle::new(ind_origin, Size::new(ind_w, ind_h));

            // 1. Circular indicator layer
            let ind_layer = toggle::indicator_layer(indicator, ToggleControl::Radio, &style);
            renderer.fill_quad(ind_layer.quad(), Background::Color(ind_layer.fill));

            // 2. Centered inner dot mark layer when selected
            if let Some(mark) =
                toggle::mark_layer(indicator, ToggleControl::Radio, polarity, &style)
            {
                let mut quad = mark.quad();
                quad.border = quad.border.rounded(shape::RADIUS_PILL);
                renderer.fill_quad(quad, Background::Color(mark.fill));
            }

            // 3. Focus ring on option indicator
            if let Some(ring) = toggle::focus_ring_layer(indicator, ToggleControl::Radio, &style) {
                renderer.fill_quad(ring.quad(), Background::Color(ring.fill));
            }

            // 4. Label text
            if !option.label.is_empty() {
                let label_x = indicator.x + ind_w + toggle::LABEL_GAP;
                let label_y = opt_bounds.y;
                let label_w = (opt_bounds.width - (ind_w + toggle::LABEL_GAP)).max(0.0);
                let label_h = opt_bounds.height;
                renderer.fill_text(
                    text_core::Text {
                        content: option.label.clone(),
                        bounds: Size::new(label_w, label_h),
                        size: renderer.default_size(),
                        line_height: LineHeight::Absolute(Pixels(label_h)),
                        font: renderer.default_font(),
                        align_x: text_core::Alignment::Left,
                        align_y: alignment::Vertical::Center,
                        shaping: text_core::Shaping::default(),
                        wrapping: text_core::Wrapping::None,
                    },
                    Point::new(label_x, label_y),
                    style.label,
                    *viewport,
                );
            }
        }
    }
}

impl<'a, Message, Id, Theme, Renderer> From<RadioGroup<'a, Message, Id, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Id: 'a + Clone + PartialEq + 'static,
    Theme: 'a,
    Renderer: 'a + renderer::Renderer + text_core::Renderer,
{
    fn from(group: RadioGroup<'a, Message, Id, Theme, Renderer>) -> Self {
        Element::new(group)
    }
}

// ===========================================================================
// ToggleSwitch (`UIF-ATOM-003`, §3.3)
// ===========================================================================

/// Ephemeral interaction and focus state of a [`ToggleSwitch`] (`UIF-ATOM-003`).
#[derive(Debug, Default, PartialEq)]
pub struct SwitchState {
    /// Focus router tracking.
    pub focusable: FocusableState,
    /// Whether pointer is hovering over bounds.
    pub hovered: bool,
    /// Whether left mouse button is pressed over bounds.
    pub pressed: bool,
}

/// The live, keyboard-focusable toggle switch widget (`UIF-ATOM-003`, §3.3).
pub struct ToggleSwitch<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    focus_id: FocusId,
    widget_id: WidgetId,
    palette: Palette,
    label: String,
    on: bool,
    busy: bool,
    enabled: bool,
    on_text: Option<String>,
    off_text: Option<String>,
    model: Option<Controlled<'a, SwitchModel>>,
    emit: Option<SwitchEmit<'a, Message>>,
    on_toggle: Option<SwitchToggle<'a, Message>>,
    _phantom: PhantomData<(Theme, Renderer)>,
}

impl<'a, Message, Theme, Renderer> ToggleSwitch<'a, Message, Theme, Renderer> {
    /// Build a controlled switch over a host-owned [`SwitchModel`] (`UIF-ATOM-003`, §3.3).
    #[must_use]
    pub fn controlled(
        model: Controlled<'a, SwitchModel>,
        focus_id: impl Into<FocusId>,
        emit: impl Fn(Commit<bool>) -> Option<Message> + 'a,
    ) -> Self {
        let focus_id = focus_id.into();
        let widget_id = widget_id(&focus_id);
        let m = model.get();
        Self {
            focus_id,
            widget_id,
            palette: Palette::dark(),
            label: m.label.clone(),
            on: m.on,
            busy: m.busy,
            enabled: !m.disabled,
            on_text: m.on_text.clone(),
            off_text: m.off_text.clone(),
            model: Some(model),
            emit: Some(Box::new(emit)),
            on_toggle: None,
            _phantom: PhantomData,
        }
    }

    /// Build a standalone toggle switch (`UIF-ATOM-003`, §3.3).
    #[must_use]
    pub fn standalone(
        label: impl Into<String>,
        on: bool,
        focus_id: impl Into<FocusId>,
        on_toggle: impl Fn(bool) -> Option<Message> + 'a,
    ) -> Self {
        let focus_id = focus_id.into();
        let widget_id = widget_id(&focus_id);
        Self {
            focus_id,
            widget_id,
            palette: Palette::dark(),
            label: label.into(),
            on,
            busy: false,
            enabled: true,
            on_text: None,
            off_text: None,
            model: None,
            emit: None,
            on_toggle: Some(Box::new(on_toggle)),
            _phantom: PhantomData,
        }
    }

    /// Set custom palette for paint resolution (`UIF-ATOM-003`).
    #[must_use]
    pub fn with_palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// The [`FocusId`] this toggle switch answers to (`UIF-ATOM-003`).
    #[must_use]
    pub fn focus_id(&self) -> &FocusId {
        &self.focus_id
    }

    /// The iced [`WidgetId`] this toggle switch registers with the router (`UIF-ATOM-003`).
    #[must_use]
    pub fn widget_id(&self) -> WidgetId {
        self.widget_id.clone()
    }

    /// Mark standalone switch as inert (`UIF-ATOM-003`).
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.enabled = !disabled;
        self
    }

    /// Mark standalone switch as busy (`UIF-ATOM-003`).
    #[must_use]
    pub fn busy(mut self, busy: bool) -> Self {
        self.busy = busy;
        self
    }

    /// Set on/off text captions beside switch track (`UIF-ATOM-003`).
    #[must_use]
    pub fn with_texts(mut self, on_text: impl Into<String>, off_text: impl Into<String>) -> Self {
        self.on_text = Some(on_text.into());
        self.off_text = Some(off_text.into());
        self
    }

    /// Whether this switch is interactive (`UIF-ATOM-003`).
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.model
            .as_ref()
            .map_or(self.enabled, |m| !m.get().disabled)
    }

    /// Whether a toggle operation is in flight (`UIF-ATOM-003`).
    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.model.as_ref().map_or(self.busy, |m| m.get().busy)
    }

    /// Whether the switch is currently on (`UIF-ATOM-003`).
    #[must_use]
    pub fn is_on(&self) -> bool {
        self.model.as_ref().map_or(self.on, |m| m.get().on)
    }

    /// The inline label string (`UIF-ATOM-003`).
    #[must_use]
    pub fn label_text(&self) -> &str {
        self.model.as_ref().map_or(&self.label, |m| &m.get().label)
    }

    /// Auxiliary text for the current on/off position (`UIF-ATOM-003`).
    #[must_use]
    pub fn state_text(&self) -> Option<&str> {
        if self.is_on() {
            self.model
                .as_ref()
                .and_then(|m| m.get().on_text.as_deref())
                .or(self.on_text.as_deref())
        } else {
            self.model
                .as_ref()
                .and_then(|m| m.get().off_text.as_deref())
                .or(self.off_text.as_deref())
        }
    }

    fn perform_toggle(&self, shell: &mut Shell<'_, Message>) {
        if !self.is_enabled() || self.is_busy() {
            return;
        }

        if let Some(model) = &self.model {
            if let Some(commit) = model.get().toggle()
                && let Some(emit) = &self.emit
                && let Some(msg) = emit(commit)
            {
                shell.publish(msg);
            }
        } else if let Some(on_toggle) = &self.on_toggle {
            let next = !self.on;
            if let Some(msg) = on_toggle(next) {
                shell.publish(msg);
            }
        }
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for ToggleSwitch<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: renderer::Renderer + text_core::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<SwitchState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(SwitchState::default())
    }

    fn children(&self) -> Vec<Tree> {
        vec![]
    }

    fn diff(&self, _tree: &mut Tree) {}

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Fixed(size::HIT_TARGET_MIN))
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let label = self.label_text();
        let aux = self.state_text().unwrap_or("");
        let display = match (label.is_empty(), aux.is_empty()) {
            (false, false) => format!("{label} ({aux})"),
            (false, true) => label.to_owned(),
            (true, false) => aux.to_owned(),
            (true, true) => String::new(),
        };

        let label_w = measure_text_width(&display, renderer);
        let (w, h) = toggle::hit_size(ToggleControl::Switch, label_w);
        let size = limits.resolve(Length::Shrink, Length::Fixed(h), Size::new(w, h));
        layout::Node::new(size)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_mut::<SwitchState>();
        if self.is_enabled() {
            operation.focusable(Some(&self.widget_id), layout.bounds(), &mut state.focusable);
        } else {
            <FocusableState as operation::Focusable>::unfocus(&mut state.focusable);
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        if shell.is_event_captured() {
            return;
        }

        let state = tree.state.downcast_mut::<SwitchState>();
        let is_over = cursor.is_over(layout.bounds());
        state.hovered = is_over;

        if !self.is_enabled() || self.is_busy() {
            state.pressed = false;
            return;
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if is_over => {
                state.pressed = true;
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if state.pressed => {
                state.pressed = false;
                if is_over {
                    self.perform_toggle(shell);
                }
                shell.capture_event();
            }
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: Key::Named(Named::Space),
                ..
            }) if state.focusable.is_focused() => {
                self.perform_toggle(shell);
                shell.capture_event();
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
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if self.is_enabled() && !self.is_busy() && cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<SwitchState>();
        let is_enabled = self.is_enabled();
        let on = self.is_on();

        let base = if !is_enabled {
            BaseState::Disabled
        } else if state.pressed {
            BaseState::Pressed
        } else if state.hovered {
            BaseState::Hovered
        } else {
            BaseState::Rest
        };
        let visual = VisualState {
            base,
            focused: state.focusable.is_focused(),
        };

        let polarity = if on {
            TogglePolarity::On
        } else {
            TogglePolarity::Off
        };
        let style = toggle::resolve_style(polarity, visual, &self.palette);
        let bounds = layout.bounds();

        let (track_w, track_h) = toggle::indicator_size(ToggleControl::Switch);
        let track_origin = Point::new(bounds.x, bounds.y + (bounds.height - track_h) / 2.0);
        let indicator = Rectangle::new(track_origin, Size::new(track_w, track_h));

        // 1. Pill track layer
        let track_layer = toggle::indicator_layer(indicator, ToggleControl::Switch, &style);
        renderer.fill_quad(track_layer.quad(), Background::Color(track_layer.fill));

        // 2. Sliding thumb layer
        if let Some(thumb) = toggle::mark_layer(indicator, ToggleControl::Switch, polarity, &style)
        {
            let mut quad = thumb.quad();
            quad.border = quad.border.rounded(shape::RADIUS_PILL);
            renderer.fill_quad(quad, Background::Color(thumb.fill));
        }

        // 3. Focus ring around track
        if let Some(ring) = toggle::focus_ring_layer(indicator, ToggleControl::Switch, &style) {
            renderer.fill_quad(ring.quad(), Background::Color(ring.fill));
        }

        // 4. Label text & optional position text
        let label = self.label_text();
        let aux = self.state_text().unwrap_or("");
        let display = match (label.is_empty(), aux.is_empty()) {
            (false, false) => format!("{label} ({aux})"),
            (false, true) => label.to_owned(),
            (true, false) => aux.to_owned(),
            (true, true) => String::new(),
        };

        if !display.is_empty() {
            let label_x = indicator.x + track_w + toggle::LABEL_GAP;
            let label_y = bounds.y;
            let label_w = (bounds.width - (track_w + toggle::LABEL_GAP)).max(0.0);
            let label_h = bounds.height;
            renderer.fill_text(
                text_core::Text {
                    content: display,
                    bounds: Size::new(label_w, label_h),
                    size: renderer.default_size(),
                    line_height: LineHeight::Absolute(Pixels(label_h)),
                    font: renderer.default_font(),
                    align_x: text_core::Alignment::Left,
                    align_y: alignment::Vertical::Center,
                    shaping: text_core::Shaping::default(),
                    wrapping: text_core::Wrapping::None,
                },
                Point::new(label_x, label_y),
                style.label,
                *viewport,
            );
        }
    }
}

impl<'a, Message, Theme, Renderer> From<ToggleSwitch<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a,
    Renderer: 'a + renderer::Renderer + text_core::Renderer,
{
    fn from(switch: ToggleSwitch<'a, Message, Theme, Renderer>) -> Self {
        Element::new(switch)
    }
}

#[cfg(test)]
#[path = "toggle_widget_tests.rs"]
mod toggle_widget_tests;
