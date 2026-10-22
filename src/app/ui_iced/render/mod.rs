//! Iced markdown render path.
//!
//! Maps the **existing, untouched** document model — `crate::markdown`'s
//! [`RenderEvent`] / [`InlineElem`] / [`crate::markdown::model::TextStyle`]
//! — onto iced widgets. The parser is not modified; nothing here reads a
//! file, touches the bus or mutates application state. Every colour comes
//! from a [`Palette`] token directly.
//!
//! # Pure descriptor layer
//!
//! Rendering is split so the mapping is testable without an iced runtime,
//! following the top panel's `MenuEntry` pattern:
//!
//! ```text
//! RenderEvent ──blocks_from_events──▶ RenderBlock { kind: BlockKind, scroll_to_me }
//!                                        │
//!                          BlockKind::{Heading, Paragraph, Code, Table, Spacer, Separator}
//!                                        │
//!                              render_block ──▶ Element<'static, Message>
//! ```
//!
//! Every `BlockKind` carries fully resolved descriptors —
//! [`SpanModel`], [`CodeBlockModel`],
//! [`TableModel`], [`YamlRowModel`](yaml::YamlRowModel)
//! — so a test asserts on data (sizes, colours, solved widths, the exact
//! message a link or copy button carries) rather than on a widget tree.
//!
//! # Sub-modules (by concern, [RUST-051])
//!
//! * [`inline`] — styled runs → iced `Span` / `Rich` (paragraph, list
//!   item and heading leaves), link click → [`RenderMessage::LinkActivated`]
//! * [`heading`] — the H1..H6 size ladder and the blank-heading rule
//! * [`code`] — fenced blocks in `Font::MONOSPACE` on `code_surface`,
//!   copy button → [`RenderMessage::CopyCodeRequested`]
//! * [`table`] — the FTWA-solved grid on `iced::widget::table`
//! * [`yaml`] — front-matter key/value rows
//!
//! # Not implemented
//!
//! * **Viewport culling.** Off-screen events are rendered in full. iced
//!   `scrollable` virtualises nothing by itself, and culling needs live
//!   scroll offsets that the pure view does not have.
//! * **Scroll-to-target execution.** The pure layer still *marks* the
//!   first block matching a search term ([`RenderBlock::scroll_to_me`]),
//!   but performing the scroll needs a `Task` against a `scrollable` `Id`,
//!   which is reducer work.
//! * **Column alignment.** GFM's `|:---|:--:|` row never reaches the
//!   document model — see the [`table`] module docs.
//! * **Per-link hover tooltips.** iced's `Rich` text uses one widget for the
//!   whole paragraph, so a per-span tooltip is not expressible.
//!
//! Unit tests live in the sibling `mod_tests.rs` sidecar.

pub mod code;
pub mod heading;
pub mod inline;
pub mod table;
pub mod yaml;

use crate::markdown::model::{InlineElem, RenderEvent};
use crate::markdown::table_width::DeficitStrategy;
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::message::{Message, RenderMessage};
use crate::ui_iced::table_width::TablePadding;
use crate::ui_iced::table_width::{IcedTextMeasurer, TextMetrics};
use iced::widget::{Column, Space, checkbox, container, rule, text};
use iced::{Alignment, Element, Length};
use std::path::PathBuf;

use code::CodeBlockModel;
use inline::SpanModel;
use table::TableModel;

/// Vertical gap between rendered blocks.
///
/// Mirrors the center panel's own `VIEWER_SPACING`
/// (`src/app/ui_iced/panels/center.rs:177`) so the render path slots
/// into the band's existing rhythm.
pub const BLOCK_SPACING: f32 = 6.0;

/// Vertical gap between the front-matter block and the document body.
pub const DOCUMENT_SPACING: f32 = 8.0;

/// One indentation level of a nested list item, in logical pixels.
pub const INDENT_UNIT: f32 = 20.0;

/// Space around a task-list checkbox.
pub const TASK_CHECKBOX_SPACING: f32 = 4.0;

/// Width of the rendered horizontal rule; 1 px matches the top bar's rule.
pub const SEPARATOR_WIDTH: f32 = 1.0;

/// Point size of the list bullet / ordinal marker.
pub const LIST_MARKER_SIZE: f32 = inline::LIST_MARKER_SIZE;

/// A list-item prefix.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListMarker {
    /// Unordered item — the [`strings::LIST_BULLET`] glyph.
    Bullet,
    /// Ordered item, rendered as `"<n>. "`.
    Ordinal(u64),
}

impl ListMarker {
    /// The text this marker renders as.
    #[must_use]
    pub fn text(self) -> String {
        match self {
            Self::Bullet => format!("{} ", strings::LIST_BULLET),
            Self::Ordinal(n) => format!("{n}{}", strings::ORDERED_LIST_MARKER_SUFFIX),
        }
    }
}

/// A task-list checkbox and its state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskCheckbox {
    /// Zero-based index of the task item within the document, the ordinal
    /// `apply_task_toggle` consumes.
    pub task_index: usize,
    /// Current checked state.
    pub checked: bool,
}

/// One rendered block of a markdown document.
#[derive(Clone, Debug, PartialEq)]
pub enum BlockKind {
    /// A heading at `level` (1..=6).
    Heading {
        /// Heading level.
        level: u32,
        /// Styled heading runs.
        spans: Vec<SpanModel>,
    },
    /// A paragraph or list item.
    Paragraph {
        /// Styled body runs.
        spans: Vec<SpanModel>,
        /// Nesting depth in list-indent units.
        indent: usize,
        /// Bullet or ordinal prefix, when this is a list item.
        marker: Option<ListMarker>,
        /// Task checkbox, when this is a task item.
        task: Option<TaskCheckbox>,
    },
    /// A fenced code block.
    Code(CodeBlockModel),
    /// A solved markdown table.
    Table(TableModel),
    /// A vertical spacer of `f32` logical pixels.
    Spacer(f32),
    /// A horizontal rule.
    Separator,
}

/// A block plus the render path's scroll intent.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderBlock {
    /// What to render.
    pub kind: BlockKind,
    /// First block matching the active search term, following the
    /// first-match-then-clear rule. The pure layer only marks it; executing
    /// the scroll is reducer work.
    pub scroll_to_me: bool,
}

/// Everything the mapping needs that is not in the document itself.
///
/// Built once per render pass. The default [`IcedTextMeasurer`] is the
/// deterministic heuristic; a renderer-backed one can be injected without
/// touching the mapping.
pub struct RenderContext<'a> {
    /// Resolved colour tokens for this pass.
    pub palette: Palette,
    /// FTWA deficit strategy for tables.
    pub strategy: DeficitStrategy,
    /// File the rendered text came from, carried into link and task
    /// intents. `None` for non-file-backed text (agent transcript).
    pub source_file: Option<PathBuf>,
    /// Per-cell table padding folded into the width solve (`TBL-033`).
    pub table_padding: TablePadding,
    /// Text measurer the solver runs against.
    pub measurer: IcedTextMeasurer<'a>,
    /// Active content-search term; marks the first matching block.
    pub search_term: Option<&'a str>,
}

impl RenderContext<'static> {
    /// A context with the FastMD palette, the heuristic measurer and no
    /// search term.
    #[must_use]
    pub fn new(strategy: DeficitStrategy, source_file: Option<PathBuf>) -> Self {
        Self {
            palette: Palette::fastmd_dark(),
            strategy,
            source_file,
            table_padding: TablePadding::ZERO,
            measurer: IcedTextMeasurer::heuristic(TextMetrics::FASTMD),
            search_term: None,
        }
    }
}

impl<'a> RenderContext<'a> {
    /// Overrides the text measurer (renderer-backed path).
    #[must_use]
    pub fn with_measurer(mut self, measurer: IcedTextMeasurer<'a>) -> Self {
        self.measurer = measurer;
        self
    }

    /// Overrides the per-cell table padding.
    #[must_use]
    pub fn with_table_padding(mut self, padding: TablePadding) -> Self {
        self.table_padding = padding;
        self
    }

    /// Sets the content-search term.
    #[must_use]
    pub fn with_search(mut self, term: &'a str) -> Self {
        self.search_term = Some(term);
        self
    }
}

/// Whether one inline element's text carries `term_lower`.
///
/// Images are excluded because their placeholder is not searchable text.
#[must_use]
pub fn inline_elems_contain(elems: &[InlineElem], term_lower: &str) -> bool {
    elems.iter().any(|elem| match elem {
        InlineElem::Text(text, _) => text.to_lowercase().contains(term_lower),
        InlineElem::Link(_, label) => label.to_lowercase().contains(term_lower),
        InlineElem::Html(html) => html.to_lowercase().contains(term_lower),
        InlineElem::Image(_) | InlineElem::SoftBreak => false,
    })
}

/// Whether an event's rendered content contains `term_lower`.
#[must_use]
pub fn event_contains(event: &RenderEvent, term_lower: &str) -> bool {
    match event {
        RenderEvent::FlushInline { elems, .. } => inline_elems_contain(elems, term_lower),
        RenderEvent::CodeBlock { content, .. } => content.to_lowercase().contains(term_lower),
        RenderEvent::Heading { elems, .. } => inline_elems_contain(elems, term_lower),
        RenderEvent::Table(rows) => rows.iter().any(|row| {
            row.iter()
                .any(|cell| inline_elems_contain(cell, term_lower))
        }),
        RenderEvent::Space(_) | RenderEvent::Separator => false,
    }
}

/// Maps the parser's event stream onto render blocks.
///
/// `task_index` counts task items in document order, so the index a toggle
/// carries matches the index `apply_task_toggle` expects. Blank headings are
/// dropped and produce no block.
#[must_use]
pub fn blocks_from_events(
    events: &[RenderEvent],
    available_width: f32,
    ctx: &RenderContext<'_>,
) -> Vec<RenderBlock> {
    let term = ctx.search_term.map(|term| term.to_lowercase());
    let mut blocks = Vec::with_capacity(events.len());
    let mut task_index = 0usize;
    let mut search_matched = false;

    for event in events {
        let kind = match event {
            RenderEvent::FlushInline {
                elems,
                needs_bullet,
                task_checked,
                indent,
                list_ordinal,
            } => {
                let block = BlockKind::Paragraph {
                    spans: inline::spans_from_inline(elems, inline::BODY_SIZE, ctx.palette),
                    indent: *indent,
                    marker: needs_bullet.then(|| match list_ordinal {
                        Some(ordinal) => ListMarker::Ordinal(*ordinal),
                        None => ListMarker::Bullet,
                    }),
                    task: task_checked.map(|checked| TaskCheckbox {
                        task_index,
                        checked,
                    }),
                };
                if task_checked.is_some() {
                    task_index += 1;
                }
                block
            }
            RenderEvent::CodeBlock { language, content } => {
                BlockKind::Code(code::code_block_model(language.as_deref(), content))
            }
            RenderEvent::Heading { level, elems } => {
                if heading::is_blank_heading(elems) {
                    continue;
                }
                BlockKind::Heading {
                    level: *level,
                    spans: heading::heading_spans(elems, *level, ctx.palette),
                }
            }
            RenderEvent::Table(cells) => BlockKind::Table(table::table_model(
                cells,
                available_width,
                ctx.strategy,
                ctx.table_padding,
                &ctx.measurer,
                ctx.palette,
            )),
            RenderEvent::Space(amount) => BlockKind::Spacer(*amount),
            RenderEvent::Separator => BlockKind::Separator,
        };

        let scroll_to_me = term
            .as_deref()
            .is_some_and(|term| !search_matched && event_contains(event, term));
        if scroll_to_me {
            search_matched = true;
        }
        blocks.push(RenderBlock { kind, scroll_to_me });
    }

    blocks
}

/// The message a task checkbox produces, `None` when the rendered text is
/// not backed by a file (nothing to write the toggle back to).
#[must_use]
pub fn task_toggle_message(
    source_file: Option<&PathBuf>,
    task: TaskCheckbox,
    checked: bool,
) -> Option<Message> {
    source_file.map(|document| {
        Message::Render(RenderMessage::TaskToggled {
            document: document.clone(),
            task_index: task.task_index,
            checked,
        })
    })
}

/// Renders a paragraph / list-item block.
#[must_use]
pub fn paragraph_block(
    spans: &[SpanModel],
    indent: usize,
    marker: Option<&ListMarker>,
    task: Option<TaskCheckbox>,
    ctx: &RenderContext<'_>,
) -> Element<'static, Message> {
    let mut row = iced::widget::Row::new()
        .align_y(Alignment::Center)
        .spacing(if task.is_some() {
            TASK_CHECKBOX_SPACING
        } else {
            0.0
        });

    if indent > 0 {
        row = row.push(Space::new().width(Length::Fixed(indent as f32 * INDENT_UNIT)));
    }
    if let Some(marker) = marker {
        row = row.push(
            text(marker.text())
                .size(LIST_MARKER_SIZE)
                .color(ctx.palette.text),
        );
    }
    if let Some(task) = task {
        let toggle = task_toggle_message(ctx.source_file.as_ref(), task, !task.checked);
        let box_widget = match toggle {
            Some(message) => checkbox(task.checked).on_toggle(move |_| message.clone()),
            None => checkbox(task.checked),
        };
        row = row.push(box_widget);
    }

    row.push(inline::rich_text_block(spans, ctx.source_file.as_deref()))
        .into()
}

/// Renders one block into an iced element.
#[must_use]
pub fn render_block(block: &RenderBlock, ctx: &RenderContext<'_>) -> Element<'static, Message> {
    match &block.kind {
        BlockKind::Heading { spans, .. } => {
            heading::heading_block(spans, ctx.source_file.as_deref())
        }
        BlockKind::Paragraph {
            spans,
            indent,
            marker,
            task,
        } => paragraph_block(spans, *indent, marker.as_ref(), *task, ctx),
        BlockKind::Code(model) => code::code_block(model),
        BlockKind::Table(model) => table::table_widget(model),
        BlockKind::Spacer(amount) => Space::new().height(*amount).into(),
        BlockKind::Separator => rule::horizontal(SEPARATOR_WIDTH).into(),
    }
}

/// Renders parsed markdown events into one column.
///
/// `available_width` is the pixel width the viewer area has been given;
/// it drives the table width solve. The returned element borrows nothing,
/// so callers can build it from a temporary event vector.
#[must_use]
pub fn render_markdown(
    events: &[RenderEvent],
    available_width: f32,
    ctx: &RenderContext<'_>,
) -> Element<'static, Message> {
    let blocks = blocks_from_events(events, available_width, ctx);
    Column::with_children(blocks.iter().map(|block| render_block(block, ctx)))
        .spacing(BLOCK_SPACING)
        .width(Length::Fill)
        .into()
}

/// Renders a whole document: front-matter block (when present) above the
/// markdown body.
///
/// This is the shape `panels::center`'s viewer area appends to its
/// always-allocated `Column` — the extension point named in
/// `src/app/ui_iced/panels/center.rs:652-671`.
#[must_use]
pub fn render_document(
    events: &[RenderEvent],
    front_matter: Option<&serde_norway::Value>,
    available_width: f32,
    ctx: &RenderContext<'_>,
) -> Element<'static, Message> {
    let front_matter_rows = front_matter
        .map(|yaml| yaml::yaml_rows_from_value(yaml, available_width))
        .unwrap_or_default();

    let mut children: Vec<Element<'static, Message>> = Vec::with_capacity(2);
    if !front_matter_rows.is_empty() {
        children.push(yaml::yaml_block(&front_matter_rows));
    }
    children.push(render_markdown(events, available_width, ctx));

    container(Column::with_children(children).spacing(DOCUMENT_SPACING))
        .width(Length::Fill)
        .into()
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
