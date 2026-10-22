//! The rendering, keyboard-focusable button — the first atom in this
//! framework that turns the pure layers into pixels (`UIF-ATOM-001`,
//! `UIF-PRIN-005`).
//!
//! # What this increment proves
//!
//! Everything before this was resolution without a renderer: [`state`]
//! decides *what state a control is in*, [`button::resolve_style`] decides
//! *what that state looks like*, [`crate::widget::focus`] decides what a
//! ring can actually be seen against, and [`focusable::Focusable`](crate::widget::focusable::Focusable) proved a custom
//! widget can hold keyboard focus at all. None of it painted anything.
//!
//! [`Button`] is where those layers meet, and the proof this module owes is
//! that **they are sufficient** — that a real, drawing, keyboard-reachable
//! button needs nothing from them beyond composition. It does. There is no
//! state logic here that is not [`state::resolve`], no colour here that is not a
//! [`Palette`] token read through [`button::resolve_style`], no ring rule
//! here that is not the shared adaptive one, and no focus mechanism that is
//! not the [`FocusableState`] `INC-10` proved. What is new here is only
//! the part that could not have existed before: geometry, and the order in
//! which quads are laid down.
//!
//! # The button paints layers, not a style
//!
//! [`draw`](Widget::draw) does not interpret [`ButtonStyle`] inline. It
//! asks two pure functions for the layers it is about to paint —
//! [`background_layer`] and [`focus_ring_layer`] — and feeds each straight
//! to [`renderer::Renderer::fill_quad`]. That split exists because a
//! focus ring is the one part of a button the renderer must place
//! *outside* the box it fills, and a test has to be able to ask what was
//! painted without a screen. A style struct alone cannot answer "where is
//! the ring"; a layer list answers it exactly.
//!
//! Draw order is background, then ring, then content. The ring is drawn
//! between the two rather than last so a label can never be overpainted by
//! it, and the background is drawn first because every cell of §3.1 is a
//! fill with a stroke on top of it.
//!
//! # The ring is a concentric annulus, not a fatter border
//!
//! iced draws a [`Border`] on the *inside* of a quad, so a ring around a
//! button cannot be that button's own border — it would eat into the fill
//! and move nothing but still read as a thicker edge. [`focus_ring_layer`]
//! instead takes the button's bounds, expands them by the ring width, and
//! strokes the *outside* of the grown quad with a transparent fill. The
//! ring therefore occupies exactly the annulus between the button's edge
//! and one ring-width out, and its corner radius is `RADIUS + ring width`
//! so the two curves stay concentric instead of the ring cutting the
//! corners harder than the box does.
//!
//! Because the ring lives outside the button's own bounds, a focused
//! control grows its painted footprint without growing its layout — which
//! is what `UIF-PRIN-003` (nothing shifts on state change) actually needs.
//!
//! # The Danger hot fill, end to end
//!
//! The hardest cell of §3.1 is a focused Danger button under the cursor.
//! Its hover fill is [`status_danger_fill`](Palette::status_danger_fill),
//! and the ring §3.1 names for that variant is
//! [`accent_primary`](Palette::accent_primary) — indigo on red, `1.1779:1`,
//! an invisible focus indicator at the moment a keyboard user most needs
//! one (gap **G-9**).
//!
//! The button resolves that without knowing any of it happened. The chain,
//! in the order it runs inside [`draw`](Widget::draw):
//!
//! 1. [`ButtonState::visual`] folds the raw observations into a
//!    [`VisualState`] through [`state::resolve`] — here
//!    `base: Hovered, focused: true`.
//! 2. [`button::resolve_style`] paints the base triple from the Danger
//!    column and then asks [`crate::widget::focus`] for a ring *against
//!    the background it just resolved*, not against the variant.
//!    `accent_primary` cannot clear the `3.0:1` chrome floor on that red,
//!    so the resolver returns [`text_strong`](Palette::text_strong).
//! 3. [`focus_ring_layer`] turns that returned colour into geometry.
//!
//! The button never mentions Danger's ring, never special-cases a hot fill,
//! and cannot get it wrong in one state and right in another: it asks the
//! same resolver every frame and draws whatever comes back. A test asserts
//! the widget's ring equals the resolver's output rather than a colour
//! literal, so retuning the palette moves both together.
//!
//! # Disabled means absent, and that is the whole gate
//!
//! A disabled button does not refuse to register with the focus router — it
//! is simply not there. [`operate`](Widget::operate) skips the
//! `operation.focusable` call and clears any focus the widget was holding,
//! exactly mirroring [`focusable::Focusable`](crate::widget::focusable::Focusable): the router cannot see the control, so
//! it cannot traverse to it, focus it, or take it back. Clearing the
//! focus in the same pass is not redundant. Without it a button focused
//! while enabled and disabled afterwards would keep a lit ring on a
//! control nothing can reach — and although [`state::resolve`] zeroes the ring
//! for a disabled control, the *focus* would still be held, so the next
//! `Tab` would compute its answer from a model that no longer matched the
//! screen.
//!
//! The pointer side is gated the same way: a disabled button records where
//! the cursor is but never sets `pressed`, never publishes and never
//! captures the event. Recording the hover is deliberate — the widget
//! reports raw observations and [`state::resolve`] judges them, which is the
//! contract [`InteractionInput`] is built on. A button that stopped
//! recording hover would be silently disagreeing with the cursor.
//!
//! # A press is committed inside the box or it is nothing
//!
//! [`Widget::update`] fires on release, not on press, and only when the
//! pointer is still inside the button's bounds. Press, drag out, release:
//! nothing happens, and the pressed skin goes away. That is the desktop
//! convention, and it is the behaviour `UIF-ATOM-001`'s separate
//! `Pressed` and `Hovered` cells only make sense under — if a press fired
//! on button-down, the `Pressed` state would exist for one frame and the
//! cell would be unreachable by a real user.
//!
//! # What later widgets will find hard here
//!
//! * **The bridge is a push, and it stays one-way.** This button never
//!   reads focus back out of iced's router; [`crate::bridge`] projects
//!   [`FocusState`](crate::focus::FocusState) onto it and nothing runs the
//!   other way. `INC-10` pinned why: iced's own `focus_next` *loses* focus
//!   at the ends instead of wrapping, so a menu that delegated `Tab` to
//!   the router would drop focus off the bottom of the list. Every keyboard
//!   move computes its target from the model and pushes it down. Menus and
//!   dialogs inherit that constraint unchanged — they add traps, not a
//!   second direction.
//! * **The bridge is not this widget's.** It was written here in `INC-11`
//!   because this was the first widget that needed it, and lifted into
//!   [`crate::bridge`] in `INC-11b` because it is generic over the whole
//!   framework rather than over buttons. What this file keeps is the
//!   *consumer* side: [`Button::widget_id`] is
//!   [`crate::bridge::widget_id`]'s answer for this widget's [`FocusId`],
//!   so the widget registers at exactly the address the bridge pushes to.
//! * **A menu row is not a button.** [`Button`] owns its content box and
//!   centres it. A menu row sits on a shared grid with an aligned trailing
//!   column, and a submenu parent must keep its own row open while a child
//!   card takes focus — a state this widget has no notion of. What the row
//!   reuses is the resolver chain and the layer shape, not this widget.
//! * **A dialog needs the trap pushed before it paints.** Confinement is
//!   [`FocusState::push_trap`](crate::focus::FocusState::push_trap), and a
//!   dialog that paints before pushing has one frame in which `Tab`
//!   reaches the background under the scrim.
//! * **Overlay draw order is not solved here.** The ring is drawn outside
//!   the button's bounds, which is fine inside a laid-out column and needs
//!   rethinking once it can be clipped by a scrollable viewport or a
//!   modal's own card edge.
//!
//! Unit tests live in the sibling `button_widget_tests.rs` sidecar.

use crate::focus::FocusId;
use crate::state;
use crate::state::InteractionInput;
use crate::state::VisualState;
use crate::token::color::Palette;
use crate::widget::button;
use crate::widget::button::ButtonStyle;
use crate::widget::button::ButtonVariant;
use crate::widget::focusable::FocusableState;

use iced::Alignment;
use iced::Background;
use iced::Border;
use iced::Color;
use iced::Element;
use iced::Event;
use iced::Length;
use iced::Padding;
use iced::Rectangle;
use iced::Shadow;
use iced::Size;
use iced::advanced::Clipboard;
use iced::advanced::Shell;
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::Id;
use iced::advanced::widget::Tree;
use iced::advanced::widget::Widget;
use iced::advanced::widget::operation::{self, Operation};
use iced::advanced::widget::tree;
use iced::mouse;

/// A stroke width that draws no stroke at all.
///
/// The *absence* of a property rather than a visual magnitude, which is
/// what [`button`] allows a widget file to carry — and named, so
/// [`background_layer`] and [`focus_ring_layer`] read as "no stroke"
/// rather than "zero point zero".
const NO_STROKE_WIDTH: f32 = 0.0;

/// A fill that paints nothing behind what it covers.
///
/// Distinct from a transparent *background* token: this is the layer
/// shape for "only the stroke is painted", which is what a focus ring is.
const NO_FILL: Color = Color::TRANSPARENT;

/// The absence of available space, used when a box may be smaller than
/// its own padding.
const NO_SPACE: f32 = 0.0;

/// Snap painted quads to the device pixel grid.
///
/// A [`button::BORDER_WIDTH`] hairline that lands between pixels renders
/// as a blurry two-pixel edge, so the framework's hairlines are snapped.
const SNAP_TO_PIXEL_GRID: bool = true;

/// One painted layer of a [`Button`], in draw order.
///
/// A layer is a rectangle, what is painted inside it, and the stroke drawn
/// on its inside edge — the exact shape
/// [`renderer::Renderer::fill_quad`] takes. Keeping the button's paint as
/// an ordered list of these rather than as inline draw calls is what
/// makes "what did this state actually paint" answerable without a
/// screen, and what lets the focus ring be placed outside the box the
/// fill covers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layer {
    /// The rectangle the layer covers.
    pub bounds: Rectangle,
    /// What is painted inside [`bounds`](Self::bounds). A fully transparent
    /// `fill` paints nothing behind the stroke.
    pub fill: Color,
    /// The stroke drawn on the inside of [`bounds`](Self::bounds).
    pub border: Border,
}

impl Layer {
    /// The [`renderer::Quad`] this layer becomes.
    ///
    /// No shadow: §3.1 gives a button none, and a raised button would read
    /// as a different elevation class than the token ladder defines.
    #[must_use]
    pub fn quad(&self) -> renderer::Quad {
        renderer::Quad {
            bounds: self.bounds,
            border: self.border,
            shadow: Shadow::default(),
            snap: SNAP_TO_PIXEL_GRID,
        }
    }
}

/// The button's fill and border for a resolved style.
///
/// Returns `None` when neither would be painted — a
/// [`Ghost`](ButtonVariant::Ghost) at rest, whose fill *and* border are
/// `transparent` — so the content sits directly on whatever the button is
/// mounted to. The box itself still exists: [`Button`] lays out the same
/// geometry either way, because `UIF-PRIN-003` forbids a state change
/// from moving a label.
#[must_use]
pub fn background_layer(bounds: Rectangle, style: &ButtonStyle) -> Option<Layer> {
    if style.is_transparent_background() && style.border_width == NO_STROKE_WIDTH {
        return None;
    }

    Some(Layer {
        bounds,
        fill: style.background,
        border: Border::default()
            .color(style.border_color)
            .width(style.border_width)
            .rounded(button::RADIUS),
    })
}

/// The focus ring drawn around a button's bounds, if one is due.
///
/// The ring is *outside* the button rather than a thicker border, because
/// iced strokes a quad on its inside: a ring drawn as the button's own
/// border would eat into the fill the state just resolved. Growing the
/// bounds by the ring width and stroking the grown quad leaves the fill
/// untouched and puts the ring in the annulus one ring-width out.
///
/// The corner radius grows by the same amount so the ring's curve stays
/// concentric with the button's. A ring left at `RADIUS` would cut the
/// corners harder than the box it surrounds and read as a mistake at
/// every corner.
///
/// The colour is whatever [`button::resolve_style`] resolved against this
/// button's own background — see the [module docs](self) for why that is
/// the only way a ring on a Danger hot fill stays visible.
#[must_use]
pub fn focus_ring_layer(bounds: Rectangle, style: &ButtonStyle) -> Option<Layer> {
    if style.focus_ring_width == NO_STROKE_WIDTH {
        return None;
    }

    Some(Layer {
        bounds: bounds.expand(style.focus_ring_width),
        fill: NO_FILL,
        border: Border::default()
            .color(style.focus_ring_color)
            .width(style.focus_ring_width)
            .rounded(button::RADIUS + style.focus_ring_width),
    })
}

/// What the button has observed about itself, before any judgement.
///
/// Every field is a raw signal from a different source — the pointer, the
/// focus router — and none of them is reconciled here.
/// [`ButtonState::visual`] hands them to [`state::resolve`], which owns the
/// precedence. That split is what keeps a widget from inventing its own
/// state rules, and it is why the 32-row table in `state_tests.rs` covers
/// what this widget paints.
///
/// The focus half is not a `bool` of this widget's own. It is the
/// [`FocusableState`] `INC-10` proved, so the router's protocol is the
/// only way in and out of focus here as well, and a button cannot be
/// focused by a path the framework has not gated.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ButtonState {
    focusable: FocusableState,
    hovered: bool,
    pressed: bool,
}

impl ButtonState {
    /// A resting, unfocused, unpressed button state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            focusable: FocusableState::new(),
            hovered: false,
            pressed: false,
        }
    }

    /// The focus state the router drives.
    #[must_use]
    pub const fn focusable(&self) -> &FocusableState {
        &self.focusable
    }

    /// Whether the pointer is inside the button's bounds.
    #[must_use]
    pub const fn is_hovered(&self) -> bool {
        self.hovered
    }

    /// Whether a button is currently held down on this button.
    #[must_use]
    pub const fn is_pressed(&self) -> bool {
        self.pressed
    }

    /// Resolve the raw observations into the state this button paints.
    ///
    /// `enabled` and `invalid` come from the widget rather than the state
    /// because they are properties of what the button *is* this frame, not
    /// of what has happened to it — the host sets them, the pointer and
    /// the keyboard set the rest.
    #[must_use]
    pub fn visual(&self, enabled: bool, invalid: bool) -> VisualState {
        state::resolve(InteractionInput {
            enabled,
            hovered: self.hovered,
            pressed: self.pressed,
            focused: self.focusable.is_focused(),
            invalid,
        })
    }
}

/// A themed, keyboard-focusable button (`UIF-ATOM-001`).
///
/// [`ButtonVariant`] selects the column of §3.1, the widget's own state
/// selects the row, and [`button::resolve_style`] turns the pair into
/// paint. The content is any [`Element`] — a label, or a `row` a host has
/// already spaced with [`button::ICON_LABEL_GAP`] — and the button owns
/// the box around it: [`HORIZONTAL_PADDING`](button::HORIZONTAL_PADDING)
/// on the sides, a fixed control height, and the content centred inside.
///
/// The widget is keyboard-focusable under a [`FocusId`] and disabled by
/// the same flag that removes it from the focus router, so "greyed out"
/// and "cannot be tabbed to" are one fact rather than two that can drift.
pub struct Button<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    variant: ButtonVariant,
    content: Element<'a, Message, Theme, Renderer>,
    on_press: Message,
    focus_id: FocusId,
    widget_id: Id,
    palette: Palette,
    height: f32,
    padding: Padding,
    enabled: bool,
    invalid: bool,
}

impl<'a, Message, Theme, Renderer> Button<'a, Message, Theme, Renderer> {
    /// Build a button for `variant` that publishes `on_press` when it is
    /// pressed and released inside its own bounds.
    ///
    /// `focus_id` is the identity the focus router knows this button by —
    /// the same [`FocusId`] the host registers in
    /// [`FocusState`](crate::focus::FocusState), so a model transition and
    /// a click address the same control.
    #[must_use]
    pub fn new(
        variant: ButtonVariant,
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        on_press: Message,
        focus_id: impl Into<FocusId>,
    ) -> Self {
        let focus_id = focus_id.into();
        let widget_id = widget_id(&focus_id);

        Self {
            variant,
            content: content.into(),
            on_press,
            focus_id,
            widget_id,
            palette: Palette::dark(),
            height: button::HEIGHT_STANDARD,
            padding: Padding::ZERO.horizontal(button::HORIZONTAL_PADDING),
            enabled: true,
            invalid: false,
        }
    }

    /// Use the compact control height — navbar and dense toolbars
    /// (`size.control.sm`).
    #[must_use]
    pub fn compact(mut self) -> Self {
        self.height = button::HEIGHT_COMPACT;
        self
    }

    /// Use the standard control height — dialog CTAs and forms
    /// (`size.control.md`). This is what [`new`](Self::new) builds.
    #[must_use]
    pub fn standard(mut self) -> Self {
        self.height = button::HEIGHT_STANDARD;
        self
    }

    /// Make the button inert: the disabled skin, no message, and absent
    /// from the focus router. See the [module docs](self) for what the
    /// absence does and does not gate.
    #[must_use]
    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Make the button live again.
    #[must_use]
    pub fn enabled(mut self) -> Self {
        self.enabled = true;
        self
    }

    /// Mark the button's value invalid.
    ///
    /// §3.1 gives a button no invalid skin — validation is a §3.2 input
    /// concern and no button in the spec validates anything — so this
    /// paints exactly as `Rest`. The flag exists so the widget feeds the
    /// full [`InteractionInput`] rather than hard-coding `false`, and the
    /// sidecar pins the gap so an invalid-button requirement has to be
    /// added deliberately.
    #[must_use]
    pub fn invalid(mut self) -> Self {
        self.invalid = true;
        self
    }

    /// Paint from a palette other than [`Palette::dark`].
    #[must_use]
    pub fn palette(mut self, palette: Palette) -> Self {
        self.palette = palette;
        self
    }

    /// The variant whose column of §3.1 this button paints.
    #[must_use]
    pub fn variant(&self) -> ButtonVariant {
        self.variant
    }

    /// The framework-level identity this button answers to.
    #[must_use]
    pub fn focus_id(&self) -> &FocusId {
        &self.focus_id
    }

    /// The iced [`Id`] this button registers with the focus router.
    #[must_use]
    pub fn widget_id(&self) -> &Id {
        &self.widget_id
    }

    /// Whether this button accepts interaction.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// The box the button occupies around a content size: the content plus
    /// padding, floored at the icon-only minimum of §3.1 and capped by
    /// the space the parent offered.
    ///
    /// The floor is [`ICON_ONLY_MIN_WIDTH`](button::ICON_ONLY_MIN_WIDTH)
    /// rather than the general `size.hit_target.min` because §3.1's atom
    /// rule is the stricter of the two for a control of this class
    /// (gap **G-2**), and the cap is the viewport because a minimum that
    /// cannot fit is a scroll bar, not a spec requirement.
    fn outer_size(&self, content: Size, limits: &layout::Limits) -> Size {
        let max = limits.max();

        Size::new(
            (content.width + self.padding.x())
                .max(button::ICON_ONLY_MIN_WIDTH)
                .min(max.width),
            self.height
                .max(content.height + self.padding.y())
                .max(button::ICON_ONLY_MIN_HEIGHT)
                .min(max.height),
        )
    }
}

/// The [`Layout`] of a button's content.
///
/// # Panics
/// Never in practice: [`Widget::layout`] always produces exactly one child
/// node for the content, and no other code path builds a button's tree.
fn content_layout(layout: Layout<'_>) -> Layout<'_> {
    layout
        .children()
        .next()
        .expect("a button lays out exactly one content node")
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Button<'_, Message, Theme, Renderer>
where
    Message: Clone,
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<ButtonState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(ButtonState::new())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Shrink, Length::Fixed(self.height))
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let content = self.content.as_widget_mut().layout(
            &mut tree.children[0],
            renderer,
            &limits.shrink(Size::from(self.padding)),
        );

        let size = self.outer_size(content.size(), limits);
        let inner = Size::new(
            (size.width - self.padding.x()).max(NO_SPACE),
            (size.height - self.padding.y()).max(NO_SPACE),
        );

        let content = content
            .move_to((self.padding.left, self.padding.top))
            .align(Alignment::Center, Alignment::Center, inner);

        layout::Node::with_children(size, vec![content])
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_mut::<ButtonState>();

        if self.enabled {
            operation.focusable(Some(&self.widget_id), layout.bounds(), &mut state.focusable);
        } else {
            // Unregistered, so the router cannot take this focus back —
            // it cannot see the widget at all. Drop it here rather than
            // leave a control holding a focus nothing can traverse to.
            // Reached through the trait because `FocusableState` has no
            // inherent `unfocus`: the router's protocol is the only way
            // in and out of focus.
            <FocusableState as operation::Focusable>::unfocus(&mut state.focusable);
        }

        operation.traverse(&mut |operation| {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                layout,
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
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            content_layout(layout),
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );

        if shell.is_event_captured() {
            return;
        }

        let is_over = cursor.is_over(layout.bounds());
        let state = tree.state.downcast_mut::<ButtonState>();

        // Where the pointer is, is recorded whether or not this button may
        // act on it. The widget reports raw observations and `resolve`
        // judges them; a button that stopped recording hover would be
        // silently disagreeing with the cursor.
        state.hovered = is_over;

        if !self.enabled {
            // No press, no message, no capture — a disabled button leaves
            // the event for anything else that wants it.
            state.pressed = false;
            return;
        }

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if is_over => {
                state.pressed = true;
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) if state.pressed => {
                // The press is released either way, so the pressed skin
                // cannot stick. Only a release still inside the box fires.
                state.pressed = false;

                if is_over {
                    shell.publish(self.on_press.clone());
                }

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
        if self.enabled && cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
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
        let state = tree.state.downcast_ref::<ButtonState>();
        let visual = state.visual(self.enabled, self.invalid);
        let paint = button::resolve_style(self.variant, visual, &self.palette);
        let bounds = layout.bounds();

        for layer in [
            background_layer(bounds, &paint),
            focus_ring_layer(bounds, &paint),
        ]
        .into_iter()
        .flatten()
        {
            renderer.fill_quad(layer.quad(), Background::Color(layer.fill));
        }

        // The label inherits nothing: §3.1 fixes the foreground per cell, so
        // the resolved colour replaces whatever text style the parent was
        // passing down rather than blending with it.
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            &renderer::Style {
                text_color: paint.foreground,
            },
            content_layout(layout),
            cursor,
            viewport,
        );
    }
}

impl<'a, Message, Theme, Renderer> From<Button<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a + Clone,
    Theme: 'a,
    Renderer: 'a + renderer::Renderer,
{
    fn from(button: Button<'a, Message, Theme, Renderer>) -> Element<'a, Message, Theme, Renderer> {
        Element::new(button)
    }
}

/// The iced [`Id`] a [`FocusId`] is registered under.
///
/// A re-export of [`crate::bridge::widget_id`], which is the single
/// derivation every focusable widget and every bridge push share. See the
/// bridge's own docs for why both sides must agree.
pub use crate::bridge::widget_id;

#[cfg(test)]
#[path = "button_widget_tests.rs"]
mod button_widget_tests;
