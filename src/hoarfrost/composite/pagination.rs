//! The pagination footer bar composite (`UIF-COMP-014`).
//!
//! §3B.14 specifies a 32px-high (`size::CONTROL_MD`) footer bar with `surface.raised`
//! background and `1px border.subtle` top border. It displays tabular numerals in
//! `FontFamily::Mono`:
//! - Left: Range summary (`Showing 1–50 of 1,240`, 12px `text.muted` in `FontFamily::Mono`).
//! - Center: Page size options selector (`25 / 50 / 100`).
//! - Right: Navigation buttons (`[⏮ First]`, `[◀ Prev]`, page indicator `Page 1 of 25`, `[Next ▶]`, `[Last ⏭]`, each 28x28px).
//!
//! Boundary buttons when on page 0 or the last page render as `Disabled` rather than
//! disappearing, maintaining 0px layout shift (`UIF-PRIN-003`).
//!
//! Emits [`Commit`] events for `on_page_change` and `on_page_size_change`.
//!
//! Unit tests live in the sibling `pagination_tests.rs` sidecar.

use crate::binding::Commit;
use crate::token::color::Palette;
use crate::token::{shape, size, space, typography};
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::Operation;
use iced::advanced::widget::tree::Tree;
use iced::advanced::{Clipboard, Shell, Widget};
use iced::mouse;
use iced::{Element, Length, Rectangle, Size, Vector};

/// Height of the pagination bar — `size::CONTROL_MD` (32px, §3B.14).
pub const BAR_HEIGHT: f32 = size::CONTROL_MD;

/// Top border stroke weight — `shape::BORDER_HAIRLINE` (1px, §3B.14).
pub const BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Navigation button size — `size::CONTROL_SM` (28px, §3B.14).
pub const BUTTON_SIZE: f32 = size::CONTROL_SM;

/// Horizontal padding of the bar — `space::MD` (12px, §3B.14).
pub const PADDING_X: f32 = space::MD;

/// Spacing between navigation items — `space::SM` (8px, §3B.14).
pub const NAV_SPACING: f32 = space::SM;

/// Spacing between page size options — `space::XS` (4px, §3B.14).
pub const OPTION_SPACING: f32 = space::XS;

/// Navigation glyph for First page — `⏮`.
pub const GLYPH_FIRST: &str = "⏮";

/// Navigation glyph for Previous page — `◀`.
pub const GLYPH_PREV: &str = "◀";

/// Navigation glyph for Next page — `▶`.
pub const GLYPH_NEXT: &str = "▶";

/// Navigation glyph for Last page — `⏭`.
pub const GLYPH_LAST: &str = "⏭";

/// Format an integer with thousands separator commas (e.g. 1240 -> "1,240").
#[must_use]
pub fn format_thousands(n: usize) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    let bytes = s.as_bytes();
    let len = bytes.len();

    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            result.push(',');
        }
        result.push(b as char);
    }
    result
}

/// Declarative state model for the pagination bar composite (`UIF-COMP-014`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaginationModel {
    /// Zero-based active page index.
    pub page: usize,
    /// Number of items displayed per page.
    pub page_size: usize,
    /// Total number of items across all pages.
    pub total_items: usize,
    /// Available page size options (e.g. `[25, 50, 100]`).
    pub page_size_options: Vec<usize>,
}

impl PaginationModel {
    /// Construct a new pagination model.
    #[must_use]
    pub fn new(
        page: usize,
        page_size: usize,
        total_items: usize,
        page_size_options: Vec<usize>,
    ) -> Self {
        Self {
            page,
            page_size,
            total_items,
            page_size_options,
        }
    }

    /// Calculate the total number of pages.
    #[must_use]
    pub fn total_pages(&self) -> usize {
        if self.total_items == 0 || self.page_size == 0 {
            0
        } else {
            self.total_items.div_ceil(self.page_size)
        }
    }

    /// 1-based index of the first item on the current page, or 0 if empty.
    #[must_use]
    pub fn start_item(&self) -> usize {
        if self.total_items == 0 {
            0
        } else {
            self.page * self.page_size + 1
        }
    }

    /// 1-based index of the last item on the current page, or 0 if empty.
    #[must_use]
    pub fn end_item(&self) -> usize {
        if self.total_items == 0 {
            0
        } else {
            ((self.page + 1) * self.page_size).min(self.total_items)
        }
    }

    /// Whether navigation to the first page is possible.
    #[must_use]
    pub fn can_first(&self) -> bool {
        self.page > 0 && self.total_items > 0
    }

    /// Whether navigation to the previous page is possible.
    #[must_use]
    pub fn can_prev(&self) -> bool {
        self.page > 0 && self.total_items > 0
    }

    /// Whether navigation to the next page is possible.
    #[must_use]
    pub fn can_next(&self) -> bool {
        let total = self.total_pages();
        total > 0 && self.page + 1 < total
    }

    /// Whether navigation to the last page is possible.
    #[must_use]
    pub fn can_last(&self) -> bool {
        let total = self.total_pages();
        total > 0 && self.page + 1 < total
    }

    /// Formatted range summary string with tabular comma numbers (e.g. `"Showing 1–50 of 1,240"`).
    #[must_use]
    pub fn range_summary(&self) -> String {
        if self.total_items == 0 {
            "Showing 0 of 0".to_string()
        } else {
            format!(
                "Showing {}–{} of {}",
                format_thousands(self.start_item()),
                format_thousands(self.end_item()),
                format_thousands(self.total_items)
            )
        }
    }

    /// Formatted page indicator string (e.g. `"Page 1 of 25"`).
    #[must_use]
    pub fn page_indicator(&self) -> String {
        if self.total_pages() == 0 {
            "Page 0 of 0".to_string()
        } else {
            format!("Page {} of {}", self.page + 1, self.total_pages())
        }
    }
}

/// The live pagination bar widget (`UIF-COMP-014`).
pub struct PaginationBar<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    model: PaginationModel,
    content: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message: 'a + Clone> PaginationBar<'a, Message, iced::Theme, iced::Renderer> {
    /// Construct a new pagination bar widget.
    #[must_use]
    pub fn new(
        model: PaginationModel,
        on_page_change: impl Fn(Commit<usize>) -> Message + 'a,
        on_page_size_change: impl Fn(Commit<usize>) -> Message + 'a,
    ) -> Self {
        let palette = Palette::dark();

        // Left section: Range summary
        let summary_text = iced::widget::text(model.range_summary())
            .size(typography::CAPTION.size)
            .line_height(typography::CAPTION.iced_line_height())
            .font(typography::CAPTION.font(typography::FAMILY_MONO, typography::Weight::Regular))
            .color(palette.text_muted);

        let left = iced::widget::container(summary_text)
            .width(Length::Fill)
            .align_y(iced::alignment::Vertical::Center);

        // Center section: Page size options
        let mut center_row = iced::widget::row![]
            .spacing(OPTION_SPACING)
            .align_y(iced::alignment::Vertical::Center);

        for &opt in &model.page_size_options {
            let is_selected = opt == model.page_size;
            let opt_text = iced::widget::text(opt.to_string())
                .size(typography::CAPTION.size)
                .line_height(typography::CAPTION.iced_line_height())
                .font(typography::CAPTION.font(typography::FAMILY_MONO, typography::Weight::Medium))
                .color(if is_selected {
                    palette.text_strong
                } else {
                    palette.text_muted
                });

            let msg = on_page_size_change(Commit::new(opt));
            let btn = iced::widget::button(opt_text)
                .on_press(msg)
                .height(BUTTON_SIZE)
                .padding(iced::Padding {
                    top: space::XXS,
                    bottom: space::XXS,
                    left: space::SM,
                    right: space::SM,
                })
                .style(move |_theme: &iced::Theme, _status| {
                    if is_selected {
                        iced::widget::button::Style {
                            background: Some(iced::Background::Color(palette.surface_overlay)),
                            text_color: palette.text_strong,
                            border: iced::Border {
                                color: palette.border_strong,
                                width: shape::BORDER_HAIRLINE,
                                radius: shape::RADIUS_WIDGET.into(),
                            },
                            shadow: iced::Shadow::default(),
                            snap: true,
                        }
                    } else {
                        iced::widget::button::Style {
                            background: None,
                            text_color: palette.text_muted,
                            border: iced::Border {
                                color: palette.border_subtle,
                                width: shape::BORDER_HAIRLINE,
                                radius: shape::RADIUS_WIDGET.into(),
                            },
                            shadow: iced::Shadow::default(),
                            snap: true,
                        }
                    }
                });

            center_row = center_row.push(btn);
        }

        let center = iced::widget::container(center_row)
            .width(Length::Shrink)
            .align_y(iced::alignment::Vertical::Center);

        // Right section: Navigation buttons (First, Prev, Indicator, Next, Last)
        let total = model.total_pages();
        let page = model.page;

        // First button [⏮]
        let first_btn = {
            let txt = iced::widget::text(GLYPH_FIRST)
                .size(typography::CAPTION.size)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .color(if model.can_first() {
                    palette.text_primary
                } else {
                    palette.text_disabled
                });
            let mut btn = iced::widget::button(txt)
                .width(BUTTON_SIZE)
                .height(BUTTON_SIZE)
                .padding(iced::Padding::new(0.0));
            if model.can_first() {
                btn = btn.on_press(on_page_change(Commit::new(0)));
            }
            btn
        };

        // Prev button [◀]
        let prev_btn = {
            let txt = iced::widget::text(GLYPH_PREV)
                .size(typography::CAPTION.size)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .color(if model.can_prev() {
                    palette.text_primary
                } else {
                    palette.text_disabled
                });
            let mut btn = iced::widget::button(txt)
                .width(BUTTON_SIZE)
                .height(BUTTON_SIZE)
                .padding(iced::Padding::new(0.0));
            if model.can_prev() {
                btn = btn.on_press(on_page_change(Commit::new(page.saturating_sub(1))));
            }
            btn
        };

        // Page indicator text
        let indicator_text = iced::widget::text(model.page_indicator())
            .size(typography::CAPTION.size)
            .line_height(typography::CAPTION.iced_line_height())
            .font(typography::CAPTION.font(typography::FAMILY_MONO, typography::Weight::Regular))
            .color(palette.text_muted);

        // Next button [▶]
        let next_btn = {
            let txt = iced::widget::text(GLYPH_NEXT)
                .size(typography::CAPTION.size)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .color(if model.can_next() {
                    palette.text_primary
                } else {
                    palette.text_disabled
                });
            let mut btn = iced::widget::button(txt)
                .width(BUTTON_SIZE)
                .height(BUTTON_SIZE)
                .padding(iced::Padding::new(0.0));
            if model.can_next() {
                btn = btn.on_press(on_page_change(Commit::new(page + 1)));
            }
            btn
        };

        // Last button [⏭]
        let last_btn = {
            let txt = iced::widget::text(GLYPH_LAST)
                .size(typography::CAPTION.size)
                .align_x(iced::alignment::Horizontal::Center)
                .align_y(iced::alignment::Vertical::Center)
                .color(if model.can_last() {
                    palette.text_primary
                } else {
                    palette.text_disabled
                });
            let mut btn = iced::widget::button(txt)
                .width(BUTTON_SIZE)
                .height(BUTTON_SIZE)
                .padding(iced::Padding::new(0.0));
            if model.can_last() {
                btn = btn.on_press(on_page_change(Commit::new(total.saturating_sub(1))));
            }
            btn
        };

        let right_row = iced::widget::row![first_btn, prev_btn, indicator_text, next_btn, last_btn]
            .spacing(NAV_SPACING)
            .align_y(iced::alignment::Vertical::Center);

        let right = iced::widget::container(right_row)
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Right)
            .align_y(iced::alignment::Vertical::Center);

        let bar_content = iced::widget::row![left, center, right]
            .width(Length::Fill)
            .height(BAR_HEIGHT)
            .padding(iced::Padding {
                top: 0.0,
                bottom: 0.0,
                left: PADDING_X,
                right: PADDING_X,
            })
            .align_y(iced::alignment::Vertical::Center);

        let container = iced::widget::container(bar_content)
            .width(Length::Fill)
            .height(BAR_HEIGHT)
            .style(move |_theme: &iced::Theme| iced::widget::container::Style {
                text_color: None,
                background: Some(iced::Background::Color(palette.surface_raised)),
                border: iced::Border {
                    color: palette.border_subtle,
                    width: BORDER_WIDTH,
                    radius: 0.0.into(),
                },
                shadow: iced::Shadow::default(),
                snap: true,
            });

        Self {
            model,
            content: container.into(),
        }
    }

    /// Access the model driving this pagination bar.
    #[must_use]
    pub fn model(&self) -> &PaginationModel {
        &self.model
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for PaginationBar<'_, Message, Theme, Renderer>
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
    From<PaginationBar<'a, Message, Theme, Renderer>> for Element<'a, Message, Theme, Renderer>
{
    fn from(widget: PaginationBar<'a, Message, Theme, Renderer>) -> Self {
        Element::new(widget)
    }
}

/// Helper function to construct a pagination bar element.
#[must_use]
pub fn pagination_bar<'a, Message: 'a + Clone>(
    model: PaginationModel,
    on_page_change: impl Fn(Commit<usize>) -> Message + 'a,
    on_page_size_change: impl Fn(Commit<usize>) -> Message + 'a,
) -> Element<'a, Message> {
    PaginationBar::new(model, on_page_change, on_page_size_change).into()
}

#[cfg(test)]
#[path = "pagination_tests.rs"]
mod pagination_tests;
