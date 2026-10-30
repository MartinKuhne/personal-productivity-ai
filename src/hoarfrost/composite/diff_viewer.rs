//! The visual diff viewer composite widget supporting unified and side-by-side modes (`UIF-COMP-015`, `UIF-PRIN-005`, `UIF-PRIN-006`).
//!
//! §3B.15 specifies an interactive diff inspection container featuring:
//! - Visual styling & redundant cues (`UIF-PRIN-005`, `UIF-PRIN-006`):
//!   - Added lines: leading `+` glyph ([`GLYPH_ADDED`]), 2px `status.success` left bar
//!     ([`INDICATOR_BAR_WIDTH`]), `text.strong` (14.29:1 contrast).
//!   - Deleted lines: leading `-` glyph ([`GLYPH_DELETED`]), 2px `status.danger.text` left bar
//!     ([`INDICATOR_BAR_WIDTH`]), `text.primary` (10.02:1 contrast).
//!   - Context lines: leading `' '` glyph ([`GLYPH_CONTEXT`]), `text.muted` (6.78:1 contrast).
//! - Hunk header bar: 28px height ([`HEADER_HEIGHT`]), `surface.raised` fill, bottom `1px border.subtle`,
//!   with `[Accept hunk]` ([`LABEL_ACCEPT_HUNK`]) and `[Reject hunk]` ([`LABEL_REJECT_HUNK`]) action buttons.
//! - Display modes ([`DiffDisplayMode`]):
//!   - `Unified`: single pane with 40px gutter ([`GUTTER_WIDTH`]).
//!   - `SideBySide`: two equal 50%/50% panes (old left, new right), each with 40px gutter
//!     ([`GUTTER_WIDTH`]), `surface.raised`, `text.disabled`, `1px border.subtle` right seam,
//!     separated by `1px border.strong` ([`DIVIDER_WIDTH`], 4.17:1 on `surface.code`) center divider.
//!
//! Unit tests live in the sibling `diff_viewer_tests.rs` sidecar.

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

/// Height of the hunk header bar — `size::CONTROL_SM` (28px, §3B.15, `UIF-COMP-015`).
pub const HEADER_HEIGHT: f32 = size::CONTROL_SM;

/// Line number gutter width — 40px (§3B.15, `UIF-COMP-015`).
pub const GUTTER_WIDTH: f32 = 40.0;

/// Status indicator bar width on added/deleted lines — `shape::BORDER_FOCUS` (2px, §3B.15, `UIF-PRIN-005`).
pub const INDICATOR_BAR_WIDTH: f32 = shape::BORDER_FOCUS;

/// Corner radius of the container and controls — `shape::RADIUS_WIDGET` (4px, §3B.15, `UIF-PRIN-006`).
pub const RADIUS: f32 = shape::RADIUS_WIDGET;

/// Hairline border width — `shape::BORDER_HAIRLINE` (1px, §3B.15, `UIF-PRIN-006`).
pub const BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Center divider width separating side-by-side panes — `shape::BORDER_HAIRLINE` (1px, §3B.15).
pub const DIVIDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Height for action buttons in the hunk header — `size::HIT_TARGET_MIN` (24px, §3B.15).
pub const BUTTON_HEIGHT: f32 = size::HIT_TARGET_MIN;

/// Horizontal padding for hunk action buttons — `space::SM` (8px, §3B.15).
pub const BUTTON_PADDING_X: f32 = space::SM;

/// Gap between action buttons in the header — `space::XS` (6px, §3B.15).
pub const BUTTON_GAP: f32 = space::XS;

/// Line height for mono diff text — `typography::UI_COMPACT.line_height` (18px, §3B.15).
pub const LINE_HEIGHT: f32 = typography::UI_COMPACT.line_height;

/// Label for accepting a hunk — `"Accept hunk"` (§3B.15).
pub const LABEL_ACCEPT_HUNK: &str = "Accept hunk";

/// Label for rejecting a hunk — `"Reject hunk"` (§3B.15).
pub const LABEL_REJECT_HUNK: &str = "Reject hunk";

/// Label for Unified mode toggle — `"Unified"` (§3B.15).
pub const LABEL_UNIFIED: &str = "Unified";

/// Label for Side-by-Side mode toggle — `"Side-by-Side"` (§3B.15).
pub const LABEL_SIDE_BY_SIDE: &str = "Side-by-Side";

/// Glyph prefix for added lines — `"+"` (§3B.15, `UIF-PRIN-005`).
pub const GLYPH_ADDED: &str = "+";

/// Glyph prefix for deleted lines — `"-"` (§3B.15, `UIF-PRIN-005`).
pub const GLYPH_DELETED: &str = "-";

/// Glyph prefix for unchanged context lines — `" "` (§3B.15).
pub const GLYPH_CONTEXT: &str = " ";

// ---------------------------------------------------------------------------
// Model Types (§3B.15, `UIF-COMP-015`)
// ---------------------------------------------------------------------------

/// Classification of an individual line within a diff hunk (§3B.15, `UIF-COMP-015`, `UIF-PRIN-005`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiffLineKind {
    /// Unchanged context line.
    Context,
    /// Inserted or added line.
    Added,
    /// Deleted or removed line.
    Deleted,
}

/// An individual line of diff content (§3B.15, `UIF-COMP-015`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    /// The line kind (Context, Added, or Deleted).
    pub kind: DiffLineKind,
    /// Line number in the original file (if applicable).
    pub old_line: Option<u32>,
    /// Line number in the modified file (if applicable).
    pub new_line: Option<u32>,
    /// Text content of the line.
    pub text: String,
}

impl DiffLine {
    /// Construct an unchanged context line (§3B.15, `UIF-COMP-015`).
    pub fn context(old_line: u32, new_line: u32, text: impl Into<String>) -> Self {
        Self {
            kind: DiffLineKind::Context,
            old_line: Some(old_line),
            new_line: Some(new_line),
            text: text.into(),
        }
    }

    /// Construct an added line (§3B.15, `UIF-COMP-015`).
    pub fn added(new_line: u32, text: impl Into<String>) -> Self {
        Self {
            kind: DiffLineKind::Added,
            old_line: None,
            new_line: Some(new_line),
            text: text.into(),
        }
    }

    /// Construct a deleted line (§3B.15, `UIF-COMP-015`).
    pub fn deleted(old_line: u32, text: impl Into<String>) -> Self {
        Self {
            kind: DiffLineKind::Deleted,
            old_line: Some(old_line),
            new_line: None,
            text: text.into(),
        }
    }
}

/// A cohesive hunk of changed lines with a header and identifier (§3B.15, `UIF-COMP-015`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk<HunkId> {
    /// Unique identifier for this hunk.
    pub id: HunkId,
    /// Description or hunk header line (e.g. `"@@ -1,5 +1,6 @@"`).
    pub header: String,
    /// Sequence of diff lines in this hunk.
    pub lines: Vec<DiffLine>,
}

impl<HunkId> DiffHunk<HunkId> {
    /// Construct a new diff hunk (§3B.15, `UIF-COMP-015`).
    pub fn new(id: HunkId, header: impl Into<String>, lines: Vec<DiffLine>) -> Self {
        Self {
            id,
            header: header.into(),
            lines,
        }
    }
}

/// Display mode for the diff viewer (§3B.15, `UIF-COMP-015`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DiffDisplayMode {
    /// Single continuous pane combining old and new changes.
    #[default]
    Unified,
    /// Two side-by-side 50%/50% panes (old left, new right) separated by a center divider.
    SideBySide,
}

/// Model driving the [`DiffViewer`] composite widget (§3B.15, `UIF-COMP-015`).
#[derive(Debug, Clone, PartialEq)]
pub struct DiffViewerModel<HunkId> {
    /// List of diff hunks to display.
    pub hunks: Vec<DiffHunk<HunkId>>,
    /// Current display mode.
    pub mode: DiffDisplayMode,
}

impl<HunkId> DiffViewerModel<HunkId> {
    /// Create a new diff viewer model with the provided hunks in Unified mode (§3B.15, `UIF-COMP-015`).
    pub fn new(hunks: Vec<DiffHunk<HunkId>>) -> Self {
        Self {
            hunks,
            mode: DiffDisplayMode::Unified,
        }
    }

    /// Set the display mode.
    #[must_use]
    pub fn with_mode(mut self, mode: DiffDisplayMode) -> Self {
        self.mode = mode;
        self
    }
}

// ---------------------------------------------------------------------------
// Content Builder (§3B.15, `UIF-COMP-015`, `UIF-PRIN-005`, `UIF-PRIN-006`)
// ---------------------------------------------------------------------------

fn build_diff_viewer_content<'a, Message: 'a + Clone, HunkId: 'a + Clone + PartialEq>(
    model: &DiffViewerModel<HunkId>,
    on_accept_hunk: Option<&(dyn Fn(Commit<HunkId>) -> Message + 'a)>,
    on_reject_hunk: Option<&(dyn Fn(Commit<HunkId>) -> Message + 'a)>,
    on_mode_change: Option<&(dyn Fn(Commit<DiffDisplayMode>) -> Message + 'a)>,
) -> Element<'a, Message> {
    let palette = Palette::dark();
    let mut hunks_col = iced::widget::column![]
        .spacing(space::MD)
        .width(Length::Fill);

    for hunk in &model.hunks {
        // 1. Hunk header bar (28px height, surface.raised, bottom 1px border.subtle)
        let header_title = iced::widget::text(hunk.header.clone())
            .size(typography::CAPTION.size)
            .line_height(typography::CAPTION.iced_line_height())
            .font(typography::FAMILY_MONO.regular())
            .color(palette.text_muted);

        let mut actions_row = iced::widget::row![]
            .spacing(BUTTON_GAP)
            .align_y(alignment::Vertical::Center);

        // Mode change toggle button (if callback present)
        if let Some(on_mode) = on_mode_change {
            let next_mode = match model.mode {
                DiffDisplayMode::Unified => DiffDisplayMode::SideBySide,
                DiffDisplayMode::SideBySide => DiffDisplayMode::Unified,
            };
            let mode_label = match model.mode {
                DiffDisplayMode::Unified => LABEL_SIDE_BY_SIDE,
                DiffDisplayMode::SideBySide => LABEL_UNIFIED,
            };
            let mode_btn = iced::widget::button(
                iced::widget::text(mode_label)
                    .size(typography::CAPTION.size)
                    .line_height(typography::CAPTION.iced_line_height())
                    .color(palette.text_muted),
            )
            .height(BUTTON_HEIGHT)
            .padding(iced::Padding {
                top: 0.0,
                bottom: 0.0,
                left: BUTTON_PADDING_X,
                right: BUTTON_PADDING_X,
            })
            .style(move |_theme: &iced::Theme, status| {
                let bg = match status {
                    iced::widget::button::Status::Hovered => {
                        Some(iced::Background::Color(palette.surface_hover))
                    }
                    _ => None,
                };
                iced::widget::button::Style {
                    background: bg,
                    text_color: palette.text_muted,
                    border: iced::Border {
                        color: palette.border_subtle,
                        width: BORDER_WIDTH,
                        radius: RADIUS.into(),
                    },
                    shadow: iced::Shadow::default(),
                    snap: true,
                }
            })
            .on_press((on_mode)(Commit::new(next_mode)));

            actions_row = actions_row.push(mode_btn);
        }

        // Accept hunk button
        if let Some(on_accept) = on_accept_hunk {
            let accept_btn = iced::widget::button(
                iced::widget::text(LABEL_ACCEPT_HUNK)
                    .size(typography::CAPTION.size)
                    .line_height(typography::CAPTION.iced_line_height())
                    .color(palette.status_success),
            )
            .height(BUTTON_HEIGHT)
            .padding(iced::Padding {
                top: 0.0,
                bottom: 0.0,
                left: BUTTON_PADDING_X,
                right: BUTTON_PADDING_X,
            })
            .style(move |_theme: &iced::Theme, status| {
                let bg = match status {
                    iced::widget::button::Status::Hovered => {
                        Some(iced::Background::Color(palette.surface_hover))
                    }
                    _ => None,
                };
                iced::widget::button::Style {
                    background: bg,
                    text_color: palette.status_success,
                    border: iced::Border {
                        color: palette.border_subtle,
                        width: BORDER_WIDTH,
                        radius: RADIUS.into(),
                    },
                    shadow: iced::Shadow::default(),
                    snap: true,
                }
            })
            .on_press((on_accept)(Commit::new(hunk.id.clone())));

            actions_row = actions_row.push(accept_btn);
        }

        // Reject hunk button
        if let Some(on_reject) = on_reject_hunk {
            let reject_btn = iced::widget::button(
                iced::widget::text(LABEL_REJECT_HUNK)
                    .size(typography::CAPTION.size)
                    .line_height(typography::CAPTION.iced_line_height())
                    .color(palette.status_danger_text),
            )
            .height(BUTTON_HEIGHT)
            .padding(iced::Padding {
                top: 0.0,
                bottom: 0.0,
                left: BUTTON_PADDING_X,
                right: BUTTON_PADDING_X,
            })
            .style(move |_theme: &iced::Theme, status| {
                let bg = match status {
                    iced::widget::button::Status::Hovered => {
                        Some(iced::Background::Color(palette.surface_hover))
                    }
                    _ => None,
                };
                iced::widget::button::Style {
                    background: bg,
                    text_color: palette.status_danger_text,
                    border: iced::Border {
                        color: palette.border_subtle,
                        width: BORDER_WIDTH,
                        radius: RADIUS.into(),
                    },
                    shadow: iced::Shadow::default(),
                    snap: true,
                }
            })
            .on_press((on_reject)(Commit::new(hunk.id.clone())));

            actions_row = actions_row.push(reject_btn);
        }

        let header_row = iced::widget::row![
            header_title,
            iced::widget::Space::new().width(Length::Fill),
            actions_row
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

        // 2. Lines rendering based on mode
        let lines_view: Element<'a, Message> = match model.mode {
            DiffDisplayMode::Unified => {
                let mut lines_col = iced::widget::column![].width(Length::Fill);

                for line in &hunk.lines {
                    let line_num = match line.kind {
                        DiffLineKind::Deleted => {
                            line.old_line.map(|n| n.to_string()).unwrap_or_default()
                        }
                        _ => line.new_line.map(|n| n.to_string()).unwrap_or_default(),
                    };

                    let gutter =
                        iced::widget::container(
                            iced::widget::text(line_num)
                                .size(typography::UI_COMPACT.size)
                                .line_height(typography::UI_COMPACT.iced_line_height())
                                .font(typography::FAMILY_MONO.regular())
                                .color(palette.text_disabled)
                                .align_x(alignment::Horizontal::Right),
                        )
                        .width(GUTTER_WIDTH)
                        .padding(iced::Padding {
                            top: 0.0,
                            bottom: 0.0,
                            left: 0.0,
                            right: space::XS,
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

                    let indicator_color = match line.kind {
                        DiffLineKind::Added => palette.status_success,
                        DiffLineKind::Deleted => palette.status_danger_text,
                        DiffLineKind::Context => Color::TRANSPARENT,
                    };

                    let indicator =
                        iced::widget::container(
                            iced::widget::Space::new().width(INDICATOR_BAR_WIDTH),
                        )
                        .width(INDICATOR_BAR_WIDTH)
                        .height(LINE_HEIGHT)
                        .style(move |_theme: &iced::Theme| iced::widget::container::Style {
                            background: Some(iced::Background::Color(indicator_color)),
                            ..Default::default()
                        });

                    let (glyph, text_color) = match line.kind {
                        DiffLineKind::Added => (GLYPH_ADDED, palette.text_strong),
                        DiffLineKind::Deleted => (GLYPH_DELETED, palette.text_primary),
                        DiffLineKind::Context => (GLYPH_CONTEXT, palette.text_muted),
                    };

                    let glyph_label = iced::widget::text(glyph)
                        .size(typography::UI_COMPACT.size)
                        .line_height(typography::UI_COMPACT.iced_line_height())
                        .font(typography::FAMILY_MONO.regular())
                        .color(indicator_color);

                    let content_label = iced::widget::text(line.text.clone())
                        .size(typography::UI_COMPACT.size)
                        .line_height(typography::UI_COMPACT.iced_line_height())
                        .font(typography::FAMILY_MONO.regular())
                        .color(text_color);

                    let row = iced::widget::row![
                        gutter,
                        indicator,
                        iced::widget::Space::new().width(space::XXS),
                        glyph_label,
                        iced::widget::Space::new().width(space::XS),
                        content_label
                    ]
                    .height(LINE_HEIGHT)
                    .align_y(alignment::Vertical::Center);

                    lines_col = lines_col.push(row);
                }

                lines_col.into()
            }
            DiffDisplayMode::SideBySide => {
                let mut old_col = iced::widget::column![].width(Length::FillPortion(1));
                let mut new_col = iced::widget::column![].width(Length::FillPortion(1));

                for line in &hunk.lines {
                    // Left pane (Old)
                    let old_row = match line.kind {
                        DiffLineKind::Added => {
                            // Blank placeholder on old side
                            let empty_gutter = iced::widget::container(
                                iced::widget::Space::new().width(GUTTER_WIDTH),
                            )
                            .width(GUTTER_WIDTH)
                            .style(move |_theme: &iced::Theme| iced::widget::container::Style {
                                background: Some(iced::Background::Color(palette.surface_raised)),
                                border: iced::Border {
                                    color: palette.border_subtle,
                                    width: BORDER_WIDTH,
                                    radius: iced::border::Radius::default(),
                                },
                                ..Default::default()
                            });
                            iced::widget::row![empty_gutter].height(LINE_HEIGHT)
                        }
                        DiffLineKind::Deleted | DiffLineKind::Context => {
                            let line_num = line.old_line.map(|n| n.to_string()).unwrap_or_default();
                            let gutter = iced::widget::container(
                                iced::widget::text(line_num)
                                    .size(typography::UI_COMPACT.size)
                                    .line_height(typography::UI_COMPACT.iced_line_height())
                                    .font(typography::FAMILY_MONO.regular())
                                    .color(palette.text_disabled)
                                    .align_x(alignment::Horizontal::Right),
                            )
                            .width(GUTTER_WIDTH)
                            .padding(iced::Padding {
                                top: 0.0,
                                bottom: 0.0,
                                left: 0.0,
                                right: space::XS,
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

                            let (ind_color, glyph, text_color) =
                                if line.kind == DiffLineKind::Deleted {
                                    (
                                        palette.status_danger_text,
                                        GLYPH_DELETED,
                                        palette.text_primary,
                                    )
                                } else {
                                    (Color::TRANSPARENT, GLYPH_CONTEXT, palette.text_muted)
                                };

                            let indicator = iced::widget::container(
                                iced::widget::Space::new().width(INDICATOR_BAR_WIDTH),
                            )
                            .width(INDICATOR_BAR_WIDTH)
                            .height(LINE_HEIGHT)
                            .style(move |_theme: &iced::Theme| iced::widget::container::Style {
                                background: Some(iced::Background::Color(ind_color)),
                                ..Default::default()
                            });

                            let glyph_label = iced::widget::text(glyph)
                                .size(typography::UI_COMPACT.size)
                                .line_height(typography::UI_COMPACT.iced_line_height())
                                .font(typography::FAMILY_MONO.regular())
                                .color(ind_color);

                            let text_label = iced::widget::text(line.text.clone())
                                .size(typography::UI_COMPACT.size)
                                .line_height(typography::UI_COMPACT.iced_line_height())
                                .font(typography::FAMILY_MONO.regular())
                                .color(text_color);

                            iced::widget::row![
                                gutter,
                                indicator,
                                iced::widget::Space::new().width(space::XXS),
                                glyph_label,
                                iced::widget::Space::new().width(space::XS),
                                text_label
                            ]
                            .height(LINE_HEIGHT)
                            .align_y(alignment::Vertical::Center)
                        }
                    };
                    old_col = old_col.push(old_row);

                    // Right pane (New)
                    let new_row = match line.kind {
                        DiffLineKind::Deleted => {
                            // Blank placeholder on new side
                            let empty_gutter = iced::widget::container(
                                iced::widget::Space::new().width(GUTTER_WIDTH),
                            )
                            .width(GUTTER_WIDTH)
                            .style(move |_theme: &iced::Theme| iced::widget::container::Style {
                                background: Some(iced::Background::Color(palette.surface_raised)),
                                border: iced::Border {
                                    color: palette.border_subtle,
                                    width: BORDER_WIDTH,
                                    radius: iced::border::Radius::default(),
                                },
                                ..Default::default()
                            });
                            iced::widget::row![empty_gutter].height(LINE_HEIGHT)
                        }
                        DiffLineKind::Added | DiffLineKind::Context => {
                            let line_num = line.new_line.map(|n| n.to_string()).unwrap_or_default();
                            let gutter = iced::widget::container(
                                iced::widget::text(line_num)
                                    .size(typography::UI_COMPACT.size)
                                    .line_height(typography::UI_COMPACT.iced_line_height())
                                    .font(typography::FAMILY_MONO.regular())
                                    .color(palette.text_disabled)
                                    .align_x(alignment::Horizontal::Right),
                            )
                            .width(GUTTER_WIDTH)
                            .padding(iced::Padding {
                                top: 0.0,
                                bottom: 0.0,
                                left: 0.0,
                                right: space::XS,
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

                            let (ind_color, glyph, text_color) = if line.kind == DiffLineKind::Added
                            {
                                (palette.status_success, GLYPH_ADDED, palette.text_strong)
                            } else {
                                (Color::TRANSPARENT, GLYPH_CONTEXT, palette.text_muted)
                            };

                            let indicator = iced::widget::container(
                                iced::widget::Space::new().width(INDICATOR_BAR_WIDTH),
                            )
                            .width(INDICATOR_BAR_WIDTH)
                            .height(LINE_HEIGHT)
                            .style(move |_theme: &iced::Theme| iced::widget::container::Style {
                                background: Some(iced::Background::Color(ind_color)),
                                ..Default::default()
                            });

                            let glyph_label = iced::widget::text(glyph)
                                .size(typography::UI_COMPACT.size)
                                .line_height(typography::UI_COMPACT.iced_line_height())
                                .font(typography::FAMILY_MONO.regular())
                                .color(ind_color);

                            let text_label = iced::widget::text(line.text.clone())
                                .size(typography::UI_COMPACT.size)
                                .line_height(typography::UI_COMPACT.iced_line_height())
                                .font(typography::FAMILY_MONO.regular())
                                .color(text_color);

                            iced::widget::row![
                                gutter,
                                indicator,
                                iced::widget::Space::new().width(space::XXS),
                                glyph_label,
                                iced::widget::Space::new().width(space::XS),
                                text_label
                            ]
                            .height(LINE_HEIGHT)
                            .align_y(alignment::Vertical::Center)
                        }
                    };
                    new_col = new_col.push(new_row);
                }

                // Center divider separating old and new sides (1px border.strong, 4.17:1 on surface.code)
                let center_divider =
                    iced::widget::container(iced::widget::Space::new().width(DIVIDER_WIDTH))
                        .width(DIVIDER_WIDTH)
                        .height(Length::Fill)
                        .style(move |_theme: &iced::Theme| iced::widget::container::Style {
                            background: Some(iced::Background::Color(palette.border_strong)),
                            ..Default::default()
                        });

                iced::widget::row![old_col, center_divider, new_col]
                    .width(Length::Fill)
                    .into()
            }
        };

        let body_container = iced::widget::container(lines_view)
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

        let hunk_box = iced::widget::column![header_container, body_container].width(Length::Fill);
        hunks_col = hunks_col.push(hunk_box);
    }

    hunks_col.into()
}

// ---------------------------------------------------------------------------
// Widget Implementation (§3B.15, `UIF-COMP-015`)
// ---------------------------------------------------------------------------

/// The visual diff viewer composite widget (§3B.15, `UIF-COMP-015`).
pub struct DiffViewer<'a, Message, HunkId, Theme = iced::Theme, Renderer = iced::Renderer> {
    model: DiffViewerModel<HunkId>,
    on_accept_hunk: Option<Box<dyn Fn(Commit<HunkId>) -> Message + 'a>>,
    on_reject_hunk: Option<Box<dyn Fn(Commit<HunkId>) -> Message + 'a>>,
    on_mode_change: Option<Box<dyn Fn(Commit<DiffDisplayMode>) -> Message + 'a>>,
    content: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message: 'a + Clone, HunkId: 'a + Clone + PartialEq>
    DiffViewer<'a, Message, HunkId, iced::Theme, iced::Renderer>
{
    /// Construct a new [`DiffViewer`] widget with the provided model (§3B.15, `UIF-COMP-015`).
    #[must_use]
    pub fn new(model: DiffViewerModel<HunkId>) -> Self {
        let content = build_diff_viewer_content(&model, None, None, None);
        Self {
            model,
            on_accept_hunk: None,
            on_reject_hunk: None,
            on_mode_change: None,
            content,
        }
    }

    fn rebuild(mut self) -> Self {
        self.content = build_diff_viewer_content(
            &self.model,
            self.on_accept_hunk.as_deref(),
            self.on_reject_hunk.as_deref(),
            self.on_mode_change.as_deref(),
        );
        self
    }

    /// Set callback emitted when accepting a diff hunk.
    #[must_use]
    pub fn on_accept_hunk(mut self, f: impl Fn(Commit<HunkId>) -> Message + 'a) -> Self {
        self.on_accept_hunk = Some(Box::new(f));
        self.rebuild()
    }

    /// Set callback emitted when rejecting a diff hunk.
    #[must_use]
    pub fn on_reject_hunk(mut self, f: impl Fn(Commit<HunkId>) -> Message + 'a) -> Self {
        self.on_reject_hunk = Some(Box::new(f));
        self.rebuild()
    }

    /// Set callback emitted when switching the display mode.
    #[must_use]
    pub fn on_mode_change(mut self, f: impl Fn(Commit<DiffDisplayMode>) -> Message + 'a) -> Self {
        self.on_mode_change = Some(Box::new(f));
        self.rebuild()
    }

    /// Access the underlying model.
    #[must_use]
    pub fn model(&self) -> &DiffViewerModel<HunkId> {
        &self.model
    }
}

impl<Message, HunkId, Theme, Renderer> Widget<Message, Theme, Renderer>
    for DiffViewer<'_, Message, HunkId, Theme, Renderer>
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

impl<'a, Message: 'a + Clone, HunkId: 'a + Clone + PartialEq> From<DiffViewer<'a, Message, HunkId>>
    for Element<'a, Message>
{
    fn from(widget: DiffViewer<'a, Message, HunkId>) -> Self {
        Element::new(widget)
    }
}

/// Helper to construct a [`DiffViewer`] composite widget (§3B.15, `UIF-COMP-015`).
#[must_use]
pub fn diff_viewer<'a, Message: 'a + Clone, HunkId: 'a + Clone + PartialEq>(
    model: DiffViewerModel<HunkId>,
) -> DiffViewer<'a, Message, HunkId> {
    DiffViewer::new(model)
}

#[cfg(test)]
#[path = "diff_viewer_tests.rs"]
mod diff_viewer_tests;
