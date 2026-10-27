//! The code block container and syntax viewer composite widget (`UIF-COMP-015`, `UIF-PRIN-003`, `UIF-PRIN-006`).
//!
//! §3B.15 specifies a fenced code block container featuring:
//! - Outer container: `palette.surface_code` fill, `1px border.subtle`, `radius.widget` (4px).
//! - Header bar: 28px height ([`HEADER_HEIGHT`]), `surface.raised` fill, bottom `1px border.subtle`.
//! - Language badge: `12px` ([`typography::CAPTION`]), [`typography::FontFamily::Mono`], `text.muted`.
//! - `[Copy]` action button: `24×24px` hit target ([`COPY_BUTTON_SIZE`]), fixed-width slot (24px)
//!   switching icon `📋` ([`GLYPH_COPY`]) -> `✓` ([`GLYPH_COPIED`]) in `status.success` for 1500ms
//!   ([`HOLD_COPIED_MS`]) on click with 0px shift (`UIF-PRIN-003`).
//! - Body: `13px` ([`typography::UI_COMPACT`]) [`typography::FontFamily::Mono`] text with optional
//!   `40px` right-aligned tabular line-number gutter ([`GUTTER_WIDTH`]) (`text.disabled`,
//!   `1px border.subtle` vertical seam).
//!
//! Unit tests live in the sibling `code_block_tests.rs` sidecar.

use crate::binding::Commit;
use crate::token::color::Palette;
use crate::token::{shape, size, space, typography};
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::Operation;
use iced::advanced::widget::tree::Tree;
use iced::advanced::{Clipboard, Shell, Widget};
use iced::alignment;
use iced::mouse;
use iced::{Color, Element, Length, Rectangle, Size, Vector};

// ---------------------------------------------------------------------------
// Surface Geometry Constants & Tokens (§3B.15, `UIF-COMP-015`)
// ---------------------------------------------------------------------------

/// Height of the header bar — `size::CONTROL_SM` (28px, §3B.15, `UIF-COMP-015`).
pub const HEADER_HEIGHT: f32 = size::CONTROL_SM;

/// Copy action button dimensions — `size::HIT_TARGET_MIN` (24px, §3B.15, `UIF-COMP-015`).
pub const COPY_BUTTON_SIZE: f32 = size::HIT_TARGET_MIN;

/// Line number gutter width — 40px (§3B.15, `UIF-COMP-015`).
pub const GUTTER_WIDTH: f32 = 40.0;

/// Corner radius of the container and controls — `shape::RADIUS_WIDGET` (4px, §3B.15, `UIF-PRIN-006`).
pub const RADIUS: f32 = shape::RADIUS_WIDGET;

/// Hairline border width — `shape::BORDER_HAIRLINE` (1px, §3B.15, `UIF-PRIN-006`).
pub const BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Copied confirmation hold duration in milliseconds — `time.hold.copied` (1500ms, §3B.15, `UIF-COMP-015`).
pub const HOLD_COPIED_MS: u32 = crate::token::time::HOLD_COPIED.ms as u32;

/// Padding around the code text area — `space::SM` (8px, §3B.15).
pub const CODE_PADDING: f32 = space::SM;

/// Horizontal padding inside the line-number gutter — `space::XS` (6px, §3B.15).
pub const GUTTER_PADDING_X: f32 = space::XS;

/// Clipboard glyph displayed on resting copy button — `"📋"` (§3B.15).
pub const GLYPH_COPY: &str = "📋";

/// Success checkmark glyph displayed when copied — `"✓"` (§3B.15).
pub const GLYPH_COPIED: &str = "✓";

// ---------------------------------------------------------------------------
// Model (§3B.15, `UIF-COMP-015`)
// ---------------------------------------------------------------------------

/// Model driving the [`CodeBlock`] composite widget (§3B.15, `UIF-COMP-015`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlockModel {
    /// Optional language identifier badge (e.g. `"rust"`, `"python"`).
    pub language: Option<String>,
    /// Source code content.
    pub code: String,
    /// Whether to display line numbers in the gutter.
    pub show_line_numbers: bool,
    /// Milliseconds remaining to display the copied confirmation state.
    pub copied_hold_remaining_ms: u32,
}

impl CodeBlockModel {
    /// Create a new code block model with the given code string (§3B.15, `UIF-COMP-015`).
    pub fn new(code: impl Into<String>) -> Self {
        Self {
            language: None,
            code: code.into(),
            show_line_numbers: true,
            copied_hold_remaining_ms: 0,
        }
    }

    /// Set the optional language identifier badge.
    #[must_use]
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }

    /// Set whether line numbers are displayed.
    #[must_use]
    pub fn with_line_numbers(mut self, show: bool) -> Self {
        self.show_line_numbers = show;
        self
    }

    /// Mark the code block as copied, initializing the 1500ms hold timer (§3B.15, `UIF-COMP-015`).
    pub fn mark_copied(&mut self) {
        self.copied_hold_remaining_ms = HOLD_COPIED_MS;
    }

    /// Advance the copied hold timer by `delta_ms` milliseconds (§3B.15, `UIF-COMP-015`).
    pub fn tick_copied(&mut self, delta_ms: u32) {
        self.copied_hold_remaining_ms = self.copied_hold_remaining_ms.saturating_sub(delta_ms);
    }

    /// Whether the copied confirmation checkmark is actively displayed (§3B.15, `UIF-COMP-015`).
    #[must_use]
    pub fn is_copied(&self) -> bool {
        self.copied_hold_remaining_ms > 0
    }

    /// Compute the number of lines contained in the code string (§3B.15, `UIF-COMP-015`).
    #[must_use]
    pub fn line_count(&self) -> usize {
        let count = self.code.lines().count();
        if count == 0 { 1 } else { count }
    }
}

// ---------------------------------------------------------------------------
// Content Builder
// ---------------------------------------------------------------------------

fn build_code_block_content<'a, Message: 'a + Clone>(
    model: &CodeBlockModel,
    on_copy: Option<&(dyn Fn(Commit<String>) -> Message + 'a)>,
) -> Element<'a, Message> {
    let palette = Palette::dark();

    // 1. Language badge (left side of header)
    let lang_badge: Element<'a, Message> = if let Some(lang) = &model.language {
        iced::widget::text(lang.clone())
            .size(typography::CAPTION.size)
            .line_height(typography::CAPTION.iced_line_height())
            .font(typography::FAMILY_MONO.regular())
            .color(palette.text_muted)
            .into()
    } else {
        iced::widget::Space::new().width(0).into()
    };

    // 2. Fixed-width copy action button (24×24px, 0px shift)
    let (copy_icon, copy_color) = if model.is_copied() {
        (GLYPH_COPIED, palette.status_success)
    } else {
        (GLYPH_COPY, palette.text_muted)
    };

    let copy_label = iced::widget::text(copy_icon)
        .size(typography::CAPTION.size)
        .line_height(typography::CAPTION.iced_line_height())
        .font(typography::FAMILY_MONO.regular())
        .color(copy_color)
        .align_x(alignment::Horizontal::Center)
        .align_y(alignment::Vertical::Center);

    let mut copy_button = iced::widget::button(copy_label)
        .width(COPY_BUTTON_SIZE)
        .height(COPY_BUTTON_SIZE)
        .padding(0)
        .style(move |_theme: &iced::Theme, status| {
            let bg = match status {
                iced::widget::button::Status::Hovered | iced::widget::button::Status::Pressed => {
                    Some(iced::Background::Color(palette.surface_hover))
                }
                _ => None,
            };
            iced::widget::button::Style {
                background: bg,
                text_color: copy_color,
                border: iced::Border {
                    color: Color::TRANSPARENT,
                    width: 0.0,
                    radius: RADIUS.into(),
                },
                shadow: iced::Shadow::default(),
                snap: true,
            }
        });

    if let Some(on_copy) = on_copy {
        copy_button = copy_button.on_press((on_copy)(Commit::new(model.code.clone())));
    }

    // 3. Header bar (28px height, surface.raised, bottom 1px border.subtle)
    let header_row = iced::widget::row![
        lang_badge,
        iced::widget::Space::new().width(Length::Fill),
        copy_button
    ]
    .height(HEADER_HEIGHT)
    .padding(iced::Padding {
        top: 0.0,
        bottom: 0.0,
        left: space::SM,
        right: space::XS,
    })
    .align_y(alignment::Vertical::Center);

    let header_container = iced::widget::container(header_row)
        .width(Length::Fill)
        .height(HEADER_HEIGHT)
        .style(move |_theme: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(palette.surface_raised)),
            border: iced::Border {
                color: palette.border_subtle,
                width: BORDER_WIDTH,
                radius: iced::border::Radius {
                    top_left: RADIUS,
                    top_right: RADIUS,
                    bottom_left: 0.0,
                    bottom_right: 0.0,
                },
            },
            ..Default::default()
        });

    // 4. Body (gutter + code)
    let mut body_row = iced::widget::row![];

    if model.show_line_numbers {
        let lines_total = model.line_count();
        let mut gutter_text = String::new();
        for i in 1..=lines_total {
            if i > 1 {
                gutter_text.push('\n');
            }
            use std::fmt::Write;
            let _ = write!(gutter_text, "{i}");
        }

        let gutter_label = iced::widget::text(gutter_text)
            .size(typography::UI_COMPACT.size)
            .line_height(typography::UI_COMPACT.iced_line_height())
            .font(typography::FAMILY_MONO.regular())
            .color(palette.text_disabled)
            .align_x(alignment::Horizontal::Right);

        let gutter_container = iced::widget::container(gutter_label)
            .width(GUTTER_WIDTH)
            .padding(iced::Padding {
                top: CODE_PADDING,
                bottom: CODE_PADDING,
                left: 0.0,
                right: GUTTER_PADDING_X,
            })
            .style(move |_theme: &iced::Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(palette.surface_raised)),
                border: iced::Border {
                    color: palette.border_subtle,
                    width: BORDER_WIDTH,
                    radius: iced::border::Radius::default(),
                },
                ..Default::default()
            });

        body_row = body_row.push(gutter_container);
    }

    let code_label = iced::widget::text(model.code.clone())
        .size(typography::UI_COMPACT.size)
        .line_height(typography::UI_COMPACT.iced_line_height())
        .font(typography::FAMILY_MONO.regular())
        .color(palette.text_primary);

    let code_container = iced::widget::container(code_label)
        .width(Length::Fill)
        .padding(iced::Padding {
            top: CODE_PADDING,
            bottom: CODE_PADDING,
            left: CODE_PADDING,
            right: CODE_PADDING,
        });

    body_row = body_row.push(code_container);

    let body_container =
        iced::widget::container(body_row)
            .width(Length::Fill)
            .style(move |_theme: &iced::Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(palette.surface_code)),
                border: iced::Border {
                    color: palette.border_subtle,
                    width: BORDER_WIDTH,
                    radius: iced::border::Radius {
                        top_left: 0.0,
                        top_right: 0.0,
                        bottom_left: RADIUS,
                        bottom_right: RADIUS,
                    },
                },
                ..Default::default()
            });

    iced::widget::column![header_container, body_container]
        .width(Length::Fill)
        .into()
}

// ---------------------------------------------------------------------------
// Widget Implementation (§3B.15, `UIF-COMP-015`)
// ---------------------------------------------------------------------------

/// The code block composite widget (§3B.15, `UIF-COMP-015`).
pub struct CodeBlock<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    model: CodeBlockModel,
    on_copy: Option<Box<dyn Fn(Commit<String>) -> Message + 'a>>,
    content: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message: 'a + Clone> CodeBlock<'a, Message, iced::Theme, iced::Renderer> {
    /// Construct a new [`CodeBlock`] widget with the given model (§3B.15, `UIF-COMP-015`).
    #[must_use]
    pub fn new(model: CodeBlockModel) -> Self {
        let content = build_code_block_content(&model, None);
        Self {
            model,
            on_copy: None,
            content,
        }
    }

    fn rebuild(mut self) -> Self {
        self.content = build_code_block_content(&self.model, self.on_copy.as_deref());
        self
    }

    /// Set the callback emitted when the user clicks the copy button.
    #[must_use]
    pub fn on_copy(mut self, f: impl Fn(Commit<String>) -> Message + 'a) -> Self {
        self.on_copy = Some(Box::new(f));
        self.rebuild()
    }

    /// Access the underlying code block model.
    #[must_use]
    pub fn model(&self) -> &CodeBlockModel {
        &self.model
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for CodeBlock<'_, Message, Theme, Renderer>
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
        );
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

impl<'a, Message: 'a + Clone> From<CodeBlock<'a, Message>> for Element<'a, Message> {
    fn from(widget: CodeBlock<'a, Message>) -> Self {
        Element::new(widget)
    }
}

/// Helper to construct a [`CodeBlock`] composite widget (§3B.15, `UIF-COMP-015`).
#[must_use]
pub fn code_block<'a, Message: 'a + Clone>(model: CodeBlockModel) -> CodeBlock<'a, Message> {
    CodeBlock::new(model)
}

#[cfg(test)]
#[path = "code_block_tests.rs"]
mod code_block_tests;
