//! The non-collapsing empty state placeholder composite (`UIF-COMP-014`, `UIF-PRIN-004`).
//!
//! §3B.14 specifies a centered, structured placeholder that fills empty panels,
//! filtered views, or errored states rather than letting them collapse to zero
//! dimensions (`UIF-PRIN-004`). It distinguishes empty collections (`NoItems`),
//! filtered queries yielding zero results (`NoMatches`), and runtime/network
//! failures (`LoadError`), each displaying a 40x40px badge containing a 20x20px
//! icon (`size::ICON_LG`), a 16px SemiBold headline (`typography::TITLE_SM`),
//! a 13px Regular explanation (`typography::UI_COMPACT`), and an optional
//! recovery CTA button emitting a [`Commit`] action.
//!
//! All visual geometry and colors are bound strictly to design tokens (`UIF-PRIN-001`).
//!
//! Unit tests live in the sibling `empty_state_tests.rs` sidecar.

use crate::binding::Commit;
use crate::token::color::Palette;
use crate::token::{shape, size, space, typography};
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::Operation;
use iced::advanced::widget::tree::Tree;
use iced::advanced::{Clipboard, Shell, Widget};
use iced::mouse;
use iced::{Color, Element, Length, Rectangle, Size, Vector};

/// The maximum width of the centered empty state container — 360px (§3B.14, matching `size.toast.width`).
pub const MAX_WIDTH: f32 = crate::composite::toast::WIDTH;

/// Inner padding of the empty state container — `space::XL` (24px, §3B.14).
pub const PADDING: f32 = space::XL;

/// Outer dimension of the icon badge container — 40px (`size::ICON_LG + size::ICON_LG`, §3B.14).
pub const BADGE_SIZE: f32 = size::ICON_LG + size::ICON_LG;

/// Border radius of the icon badge — `radius.card` (6px, §3B.14).
pub const BADGE_RADIUS: f32 = shape::RADIUS_CARD;

/// Border stroke weight of the icon badge — `1px border.subtle` (§3B.14).
pub const BADGE_BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Optical icon glyph size within the badge — `size::ICON_LG` (20px, §3B.14).
pub const ICON_SIZE: f32 = size::ICON_LG;

/// Gap between the badge and the headline text — `space::MD` (12px, §3B.14).
pub const BADGE_GAP: f32 = space::MD;

/// Gap between the headline and the description text — `space::SM` (8px, §3B.14).
pub const TEXT_GAP: f32 = space::SM;

/// Gap between the description and the CTA button — `space::XL` (24px, §3B.14).
pub const ACTION_GAP: f32 = space::XL;

/// The semantic category of the empty state placeholder (`UIF-COMP-014`).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum EmptyStateKind {
    /// A dataset or collection that contains zero items.
    NoItems,
    /// A search query or filter that matched zero results.
    NoMatches {
        /// The query string that produced no matches.
        query: String,
    },
    /// An error occurred preventing items from loading.
    LoadError {
        /// Standardized error code or identifier.
        code: String,
    },
}

impl EmptyStateKind {
    /// Whether this empty state is an error condition.
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self, Self::LoadError { .. })
    }
}

/// An icon identifier or character glyph string for display within the badge.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Icon(pub String);

impl Icon {
    /// Create a new icon from a string representation.
    #[must_use]
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl From<&str> for Icon {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for Icon {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl std::ops::Deref for Icon {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

/// The declarative model driving an empty state composite (`UIF-COMP-014`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EmptyStateModel<ActionId> {
    /// Which kind of empty condition this represents.
    pub kind: EmptyStateKind,
    /// Icon glyph or symbol to display in the 40x40 badge.
    pub icon: Icon,
    /// Primary headline text (16px SemiBold).
    pub title: String,
    /// Secondary explanation text (13px Regular).
    pub description: String,
    /// Optional primary CTA button with (label, action).
    pub primary_action: Option<(String, ActionId)>,
}

impl<ActionId> EmptyStateModel<ActionId> {
    /// Construct a new empty state model without an action.
    #[must_use]
    pub fn new(
        kind: EmptyStateKind,
        icon: impl Into<Icon>,
        title: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            icon: icon.into(),
            title: title.into(),
            description: description.into(),
            primary_action: None,
        }
    }

    /// Attach an optional primary recovery or CTA action.
    #[must_use]
    pub fn with_action(mut self, label: impl Into<String>, action: ActionId) -> Self {
        self.primary_action = Some((label.into(), action));
        self
    }
}

/// The resolved visual styling for an empty state composite.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EmptyStateStyle {
    /// Background color of the 40x40 icon badge (`surface.raised`).
    pub badge_background: Color,
    /// Border color of the icon badge (`border.subtle`).
    pub badge_border_color: Color,
    /// Foreground color of the icon glyph.
    pub icon_color: Color,
    /// Foreground color of the headline text (`text.strong`).
    pub title_color: Color,
    /// Foreground color of the explanation text (`text.muted`).
    pub description_color: Color,
}

/// Resolve empty state colors against a design token palette.
#[must_use]
pub fn resolve_style(kind: &EmptyStateKind, palette: &Palette) -> EmptyStateStyle {
    EmptyStateStyle {
        badge_background: palette.surface_raised,
        badge_border_color: palette.border_subtle,
        icon_color: if kind.is_error() {
            palette.status_danger_text
        } else {
            palette.text_muted
        },
        title_color: palette.text_strong,
        description_color: palette.text_muted,
    }
}

/// The live empty state widget (`UIF-COMP-014`).
pub struct EmptyState<'a, Message, ActionId, Theme = iced::Theme, Renderer = iced::Renderer> {
    model: EmptyStateModel<ActionId>,
    content: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message: 'a + Clone, ActionId: Clone + 'a>
    EmptyState<'a, Message, ActionId, iced::Theme, iced::Renderer>
{
    /// Construct a new empty state widget.
    #[must_use]
    pub fn new(
        model: EmptyStateModel<ActionId>,
        on_action: impl Fn(Commit<ActionId>) -> Message + 'a,
    ) -> Self {
        let palette = Palette::dark();
        let style = resolve_style(&model.kind, &palette);

        let badge_content = iced::widget::text(model.icon.to_string())
            .size(ICON_SIZE)
            .color(style.icon_color)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center);

        let badge = iced::widget::container(badge_content)
            .width(BADGE_SIZE)
            .height(BADGE_SIZE)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center)
            .style(move |_theme: &iced::Theme| iced::widget::container::Style {
                text_color: None,
                background: Some(iced::Background::Color(style.badge_background)),
                border: iced::Border {
                    color: style.badge_border_color,
                    width: BADGE_BORDER_WIDTH,
                    radius: BADGE_RADIUS.into(),
                },
                shadow: iced::Shadow::default(),
                snap: true,
            });

        let title_text = iced::widget::text(model.title.clone())
            .size(typography::TITLE_SM.size)
            .line_height(typography::TITLE_SM.iced_line_height())
            .font(iced::Font {
                weight: iced::font::Weight::Semibold,
                ..iced::Font::with_name("Fira Sans")
            })
            .color(style.title_color)
            .align_x(iced::alignment::Horizontal::Center);

        let desc_text = iced::widget::text(model.description.clone())
            .size(typography::UI_COMPACT.size)
            .line_height(typography::UI_COMPACT.iced_line_height())
            .color(style.description_color)
            .align_x(iced::alignment::Horizontal::Center);

        let mut col = iced::widget::column![badge, title_text, desc_text]
            .spacing(TEXT_GAP)
            .align_x(iced::alignment::Horizontal::Center);

        if let Some((label, action_id)) = &model.primary_action {
            let action_id = action_id.clone();
            let msg = on_action(Commit::new(action_id));
            let btn_text = iced::widget::text(label.clone())
                .size(typography::UI_COMPACT.size)
                .line_height(typography::UI_COMPACT.iced_line_height())
                .font(iced::Font {
                    weight: iced::font::Weight::Medium,
                    ..iced::Font::with_name("Fira Sans")
                });
            let btn = iced::widget::button(btn_text)
                .on_press(msg)
                .padding(iced::Padding {
                    top: space::SM,
                    bottom: space::SM,
                    left: space::LG,
                    right: space::LG,
                });
            col = col.push(btn);
        }

        let container = iced::widget::container(col)
            .width(Length::Fill)
            .height(Length::Fill)
            .max_width(MAX_WIDTH)
            .padding(PADDING)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center);

        Self {
            model,
            content: container.into(),
        }
    }

    /// Access the model driving this empty state.
    #[must_use]
    pub fn model(&self) -> &EmptyStateModel<ActionId> {
        &self.model
    }
}

impl<Message, Theme, Renderer, ActionId> Widget<Message, Theme, Renderer>
    for EmptyState<'_, Message, ActionId, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        )
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
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
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
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
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

impl<'a, Message: 'a + Clone, ActionId: 'a, Theme: 'a, Renderer: 'a + renderer::Renderer>
    From<EmptyState<'a, Message, ActionId, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
{
    fn from(widget: EmptyState<'a, Message, ActionId, Theme, Renderer>) -> Self {
        Element::new(widget)
    }
}

/// Helper function to construct an empty state element.
#[must_use]
pub fn empty_state<'a, Message: 'a + Clone, ActionId: Clone + 'a>(
    model: EmptyStateModel<ActionId>,
    on_action: impl Fn(Commit<ActionId>) -> Message + 'a,
) -> Element<'a, Message> {
    EmptyState::new(model, on_action).into()
}

#[cfg(test)]
#[path = "empty_state_tests.rs"]
mod empty_state_tests;
