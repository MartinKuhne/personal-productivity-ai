//! The pips pager compact pagination and carousel dots composite (`UIF-COMP-011`, `UIF-PRIN-004`).
//!
//! §3B.11 specifies a 24px-high (`size::BAR_STATUS`) indicator strip with previous button `[◀]`
//! (24×24px / `size::HIT_TARGET_MIN`), `N` pip indicators (6×6px `size::INDICATOR_DOT` in `palette.border_strong`
//! when inactive; 16×6px `radius.pill = 999.0` in `palette.accent_border` when active), and next button `[▶]`
//! (24×24px / `size::HIT_TARGET_MIN`).
//!
//! Boundary buttons when on page 0 or the last page render as `Disabled` rather than
//! disappearing, maintaining 0px layout shift per `UIF-PRIN-004`.
//!
//! Emits [`Commit`] events via `on_page_change`.
//!
//! Unit tests live in the sibling `pips_pager_tests.rs` sidecar.

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

/// Previous page navigation glyph — `◀` (§3B.11).
pub const GLYPH_PREV: &str = "◀";

/// Next page navigation glyph — `▶` (§3B.11).
pub const GLYPH_NEXT: &str = "▶";

/// Height of the pips pager strip — `size::BAR_STATUS` (24px, §3B.11).
pub const BAR_HEIGHT: f32 = size::BAR_STATUS;

/// Dimensions of previous and next navigation buttons — `size::HIT_TARGET_MIN` (24px, §3B.11).
pub const BUTTON_SIZE: f32 = size::HIT_TARGET_MIN;

/// Inactive pip indicator dimension — `size::INDICATOR_DOT` (6px, §3B.11).
pub const PIP_SIZE_INACTIVE: f32 = size::INDICATOR_DOT;

/// Active pip indicator width — `space::LG` (16px, §3B.11).
pub const PIP_WIDTH_ACTIVE: f32 = space::LG;

/// Height of pip indicators — `size::INDICATOR_DOT` (6px, §3B.11).
pub const PIP_HEIGHT: f32 = size::INDICATOR_DOT;

/// Gap between pip indicators — `space::XS` (4px, §3B.11).
pub const PIP_GAP: f32 = space::XS;

/// Corner radius of pip indicators — `shape::RADIUS_PILL` (999px, §3B.11).
pub const PIP_RADIUS: f32 = shape::RADIUS_PILL;

/// Declarative state model for the pips pager composite (`UIF-COMP-011`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PipsPagerModel {
    /// Zero-based active page index.
    pub page: usize,
    /// Total number of pages.
    pub total_pages: usize,
}

impl PipsPagerModel {
    /// Construct a new pips pager model.
    #[must_use]
    pub const fn new(page: usize, total_pages: usize) -> Self {
        Self { page, total_pages }
    }

    /// Whether navigation to the previous page is possible.
    #[must_use]
    pub const fn can_prev(&self) -> bool {
        self.page > 0 && self.total_pages > 0
    }

    /// Whether navigation to the next page is possible.
    #[must_use]
    pub const fn can_next(&self) -> bool {
        self.total_pages > 0 && self.page + 1 < self.total_pages
    }

    /// Whether the current page is the first page.
    #[must_use]
    pub const fn is_first_page(&self) -> bool {
        self.page == 0
    }

    /// Whether the current page is the last page.
    #[must_use]
    pub const fn is_last_page(&self) -> bool {
        self.total_pages == 0 || self.page + 1 >= self.total_pages
    }
}

/// The live pips pager widget (`UIF-COMP-011`, `UIF-PRIN-004`).
pub struct PipsPager<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    model: PipsPagerModel,
    content: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message: 'a + Clone> PipsPager<'a, Message, iced::Theme, iced::Renderer> {
    /// Construct a new pips pager widget.
    #[must_use]
    pub fn new(
        model: PipsPagerModel,
        on_page_change: impl Fn(Commit<usize>) -> Message + 'a,
    ) -> Self {
        let palette = Palette::dark();

        // Previous button [◀] (24x24px, disabled on page 0 per UIF-PRIN-004)
        let can_prev = model.can_prev();
        let prev_text = iced::widget::text(GLYPH_PREV)
            .size(typography::CAPTION.size)
            .line_height(typography::CAPTION.iced_line_height())
            .color(if can_prev {
                palette.text_primary
            } else {
                palette.text_disabled
            })
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center);

        let mut prev_btn = iced::widget::button(prev_text)
            .width(BUTTON_SIZE)
            .height(BUTTON_SIZE)
            .padding(iced::Padding::new(0.0))
            .style(
                move |_theme: &iced::Theme, _status| iced::widget::button::Style {
                    background: None,
                    text_color: if can_prev {
                        palette.text_primary
                    } else {
                        palette.text_disabled
                    },
                    border: iced::Border {
                        color: Color::TRANSPARENT,
                        width: 0.0,
                        radius: shape::RADIUS_WIDGET.into(),
                    },
                    shadow: iced::Shadow::default(),
                    snap: true,
                },
            );

        if can_prev {
            let prev_page = model.page.saturating_sub(1);
            prev_btn = prev_btn.on_press(on_page_change(Commit::new(prev_page)));
        }

        // Pips indicators strip
        let mut pips_row = iced::widget::row![]
            .spacing(PIP_GAP)
            .align_y(iced::alignment::Vertical::Center);

        for idx in 0..model.total_pages {
            let is_active = idx == model.page;
            let (pip_width, pip_bg) = if is_active {
                (PIP_WIDTH_ACTIVE, palette.accent_border)
            } else {
                (PIP_SIZE_INACTIVE, palette.border_strong)
            };

            let pip_container = iced::widget::container(
                iced::widget::Space::new()
                    .width(pip_width)
                    .height(PIP_HEIGHT),
            )
            .width(pip_width)
            .height(PIP_HEIGHT)
            .style(move |_theme: &iced::Theme| iced::widget::container::Style {
                text_color: None,
                background: Some(iced::Background::Color(pip_bg)),
                border: iced::Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: PIP_RADIUS.into(),
                },
                shadow: iced::Shadow::default(),
                snap: true,
            });

            // Inactive pips can be clicked to jump directly to page
            let pip_element: Element<'a, Message> = if !is_active {
                iced::widget::button(pip_container)
                    .padding(iced::Padding::new(0.0))
                    .style(
                        |_theme: &iced::Theme, _status| iced::widget::button::Style {
                            background: None,
                            text_color: Color::TRANSPARENT,
                            border: iced::Border::default(),
                            shadow: iced::Shadow::default(),
                            snap: true,
                        },
                    )
                    .on_press(on_page_change(Commit::new(idx)))
                    .into()
            } else {
                pip_container.into()
            };

            pips_row = pips_row.push(pip_element);
        }

        // Next button [▶] (24x24px, disabled on total_pages - 1 per UIF-PRIN-004)
        let can_next = model.can_next();
        let next_text = iced::widget::text(GLYPH_NEXT)
            .size(typography::CAPTION.size)
            .line_height(typography::CAPTION.iced_line_height())
            .color(if can_next {
                palette.text_primary
            } else {
                palette.text_disabled
            })
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center);

        let mut next_btn = iced::widget::button(next_text)
            .width(BUTTON_SIZE)
            .height(BUTTON_SIZE)
            .padding(iced::Padding::new(0.0))
            .style(
                move |_theme: &iced::Theme, _status| iced::widget::button::Style {
                    background: None,
                    text_color: if can_next {
                        palette.text_primary
                    } else {
                        palette.text_disabled
                    },
                    border: iced::Border {
                        color: Color::TRANSPARENT,
                        width: 0.0,
                        radius: shape::RADIUS_WIDGET.into(),
                    },
                    shadow: iced::Shadow::default(),
                    snap: true,
                },
            );

        if can_next {
            let next_page = model.page + 1;
            next_btn = next_btn.on_press(on_page_change(Commit::new(next_page)));
        }

        let content_row = iced::widget::row![prev_btn, pips_row, next_btn]
            .spacing(space::SM)
            .height(BAR_HEIGHT)
            .align_y(iced::alignment::Vertical::Center);

        let container = iced::widget::container(content_row)
            .height(BAR_HEIGHT)
            .align_y(iced::alignment::Vertical::Center);

        Self {
            model,
            content: container.into(),
        }
    }

    /// Access the model driving this pips pager.
    #[must_use]
    pub fn model(&self) -> &PipsPagerModel {
        &self.model
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for PipsPager<'_, Message, Theme, Renderer>
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

impl<'a, Message: 'a + Clone, Theme: 'a, Renderer: 'a + renderer::Renderer>
    From<PipsPager<'a, Message, Theme, Renderer>> for Element<'a, Message, Theme, Renderer>
{
    fn from(widget: PipsPager<'a, Message, Theme, Renderer>) -> Self {
        Element::new(widget)
    }
}

/// Helper function to construct a pips pager element.
#[must_use]
pub fn pips_pager<'a, Message: 'a + Clone>(
    model: PipsPagerModel,
    on_page_change: impl Fn(Commit<usize>) -> Message + 'a,
) -> Element<'a, Message> {
    PipsPager::new(model, on_page_change).into()
}

#[cfg(test)]
#[path = "pips_pager_tests.rs"]
mod pips_pager_tests;
