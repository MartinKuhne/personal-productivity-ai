//! The multi-step workflow header and wizard stepper composite (`UIF-COMP-011`, `UIF-PRIN-005`, `UIF-PRIN-004`).
//!
//! §3B.11 specifies a horizontal (height 36px / `size::CONTROL_LG`) or vertical step sequence
//! connected by `1px border.subtle` (Pending/Error) or `1px accent.border` (Completed) connector
//! lines. Each step node renders a 24×24px (`size::HIT_TARGET_MIN`, `radius.pill = 999.0`) indicator
//! encoding status by both glyph and color (`UIF-PRIN-005`):
//! - `Completed`: `accent.fill` background, `text.strong` foreground (`4.80:1`), glyph `'✓'`.
//! - `Current`: `surface.active` background, `2px accent.border` (`4.79:1`), `text.strong` foreground (`14.29:1`), step number `1..N`.
//! - `Pending`: `surface.raised` background, `1px border.strong` (`4.24:1`), `text.muted` foreground (`7.16:1`), step number `1..N`.
//! - `Error`: `status.danger.fill` background, `text.strong` foreground (`5.13:1`), glyph `'!'`.
//!
//! The non-shifting wizard footer renders `[← Previous]` as `Disabled` on step 0 rather than
//! omitting it, guaranteeing that button coordinates never shift across steps (0px shift per `UIF-PRIN-004`).
//!
//! Unit tests live in the sibling `stepper_tests.rs` sidecar.

use crate::binding::{Cancel, Commit};
use crate::token::color::Palette;
use crate::token::{shape, size, space, typography};
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::Operation;
use iced::advanced::widget::tree::Tree;
use iced::advanced::{Clipboard, Shell, Widget};
use iced::mouse;
use iced::{Color, Element, Length, Rectangle, Size, Vector};

/// Checkmark glyph for completed steps — `'✓'` (`UIF-PRIN-005`).
pub const GLYPH_COMPLETED: &str = "✓";

/// Exclamation mark glyph for error steps — `'!'` (`UIF-PRIN-005`).
pub const GLYPH_ERROR: &str = "!";

/// Default previous button label — `"← Previous"` (`UIF-PRIN-004`).
pub const LABEL_PREVIOUS: &str = "← Previous";

/// Default next button label — `"Next →"`.
pub const LABEL_NEXT: &str = "Next →";

/// Default finish button label — `"Finish ✓"`.
pub const LABEL_FINISH: &str = "Finish ✓";

/// Default cancel button label — `"Cancel"`.
pub const LABEL_CANCEL: &str = "Cancel";

/// Height of the horizontal stepper bar — `size::CONTROL_LG` (36px, §3B.11).
pub const BAR_HEIGHT: f32 = size::CONTROL_LG;

/// Step node indicator dimensions — `size::HIT_TARGET_MIN` (24px, §3B.11).
pub const NODE_SIZE: f32 = size::HIT_TARGET_MIN;

/// Step node corner radius — `shape::RADIUS_PILL` (999px, §3B.11).
pub const NODE_RADIUS: f32 = shape::RADIUS_PILL;

/// Stroke weight for connector lines — `shape::BORDER_HAIRLINE` (1px, §3B.11).
pub const CONNECTOR_STROKE: f32 = shape::BORDER_HAIRLINE;

/// Border width for the active/current step node — `shape::BORDER_FOCUS` (2px, §3B.11).
pub const CURRENT_BORDER_WIDTH: f32 = shape::BORDER_FOCUS;

/// Spacing between node and label — `space::SM` (8px, §3B.11).
pub const LABEL_GAP: f32 = space::SM;

/// Spacing between steps — `space::MD` (12px, §3B.11).
pub const STEP_GAP: f32 = space::MD;

/// Height for wizard footer action buttons — `size::CONTROL_MD` (32px).
pub const FOOTER_BUTTON_HEIGHT: f32 = size::CONTROL_MD;

/// The lifecycle status of an individual step in a workflow (`UIF-COMP-011`, `UIF-PRIN-005`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StepStatus {
    /// Step has not yet been started.
    Pending,
    /// Step is actively selected and being worked on.
    Current,
    /// Step has been successfully validated and completed.
    Completed,
    /// Step failed validation or encountered an error.
    Error,
}

/// Layout orientation for the stepper sequence (`UIF-COMP-011`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StepperOrientation {
    /// Steps arranged horizontally in a row with connecting lines.
    #[default]
    Horizontal,
    /// Steps arranged vertically in a column with connecting lines.
    Vertical,
}

/// Description of a single step item within [`StepperModel`] (`UIF-COMP-011`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepItem<StepId> {
    /// Unique identifier for this step.
    pub id: StepId,
    /// Descriptive human-readable label.
    pub label: String,
    /// Current completion or execution status.
    pub status: StepStatus,
    /// Whether the step is disabled from manual user selection.
    pub disabled: bool,
}

impl<StepId> StepItem<StepId> {
    /// Create a new step item.
    pub fn new(id: StepId, label: impl Into<String>, status: StepStatus) -> Self {
        Self {
            id,
            label: label.into(),
            status,
            disabled: false,
        }
    }

    /// Set whether this step is disabled.
    #[must_use]
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

/// Controlled data model for the stepper composite (`UIF-COMP-011`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepperModel<StepId> {
    /// Ordered list of steps in the sequence.
    pub steps: Vec<StepItem<StepId>>,
    /// Identifier of the currently active step.
    pub current_step: StepId,
    /// When true, navigation requires sequential completion.
    pub linear: bool,
}

impl<StepId: PartialEq> StepperModel<StepId> {
    /// Construct a new stepper model.
    pub fn new(steps: Vec<StepItem<StepId>>, current_step: StepId, linear: bool) -> Self {
        Self {
            steps,
            current_step,
            linear,
        }
    }

    /// Index of the current step within `steps`, if present.
    #[must_use]
    pub fn current_index(&self) -> Option<usize> {
        self.steps.iter().position(|s| s.id == self.current_step)
    }

    /// Whether a target step can be navigated to by direct user selection (`UIF-COMP-011`).
    ///
    /// If `linear` is false, any non-disabled step can be selected.
    /// If `linear` is true, only completed steps or the immediate next step (`current_index + 1`) can be selected.
    #[must_use]
    pub fn can_select(&self, target_id: &StepId) -> bool {
        let Some(target_idx) = self.steps.iter().position(|s| &s.id == target_id) else {
            return false;
        };
        let target = &self.steps[target_idx];
        if target.disabled {
            return false;
        }
        if !self.linear {
            return true;
        }
        if target.status == StepStatus::Completed {
            return true;
        }
        self.current_index()
            .is_some_and(|curr_idx| target_idx == curr_idx + 1)
    }

    /// Whether navigation to the previous step is possible.
    #[must_use]
    pub fn can_previous(&self) -> bool {
        self.current_index().is_some_and(|idx| idx > 0)
    }

    /// Whether navigation to the next step is possible.
    #[must_use]
    pub fn can_next(&self) -> bool {
        self.current_index()
            .is_some_and(|idx| idx + 1 < self.steps.len())
    }

    /// Whether the current step is the final step in the sequence.
    #[must_use]
    pub fn is_last_step(&self) -> bool {
        self.current_index()
            .is_some_and(|idx| !self.steps.is_empty() && idx == self.steps.len() - 1)
    }

    /// The identifier of the previous step, if available.
    #[must_use]
    pub fn previous_step_id(&self) -> Option<StepId>
    where
        StepId: Clone,
    {
        if self.can_previous() {
            let idx = self.current_index()?;
            Some(self.steps[idx - 1].id.clone())
        } else {
            None
        }
    }

    /// The identifier of the next step, if available.
    #[must_use]
    pub fn next_step_id(&self) -> Option<StepId>
    where
        StepId: Clone,
    {
        if self.can_next() {
            let idx = self.current_index()?;
            Some(self.steps[idx + 1].id.clone())
        } else {
            None
        }
    }
}

/// Callbacks and configuration for the non-shifting wizard footer (`UIF-COMP-011`, `UIF-PRIN-004`).
pub struct WizardFooterConfig<'a, Message, StepId> {
    /// Handler invoked when the user navigates to the previous step.
    pub on_previous: Option<Box<dyn Fn(Commit<StepId>) -> Message + 'a>>,
    /// Handler invoked when the user advances to the next step.
    pub on_next: Option<Box<dyn Fn(Commit<StepId>) -> Message + 'a>>,
    /// Handler invoked when the user finishes the workflow on the final step.
    pub on_finish: Option<Box<dyn Fn(Commit<()>) -> Message + 'a>>,
    /// Handler invoked when the user cancels the wizard workflow.
    pub on_cancel: Option<Box<dyn Fn(Cancel<()>) -> Message + 'a>>,
}

impl<'a, Message, StepId> Default for WizardFooterConfig<'a, Message, StepId> {
    fn default() -> Self {
        Self {
            on_previous: None,
            on_next: None,
            on_finish: None,
            on_cancel: None,
        }
    }
}

impl<'a, Message: 'a, StepId: 'a> WizardFooterConfig<'a, Message, StepId> {
    /// Construct empty configuration.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach handler for previous step activation.
    #[must_use]
    pub fn on_previous(mut self, handler: impl Fn(Commit<StepId>) -> Message + 'a) -> Self {
        self.on_previous = Some(Box::new(handler));
        self
    }

    /// Attach handler for next step activation.
    #[must_use]
    pub fn on_next(mut self, handler: impl Fn(Commit<StepId>) -> Message + 'a) -> Self {
        self.on_next = Some(Box::new(handler));
        self
    }

    /// Attach handler for finish workflow activation.
    #[must_use]
    pub fn on_finish(mut self, handler: impl Fn(Commit<()>) -> Message + 'a) -> Self {
        self.on_finish = Some(Box::new(handler));
        self
    }

    /// Attach handler for cancel workflow activation.
    #[must_use]
    pub fn on_cancel(mut self, handler: impl Fn(Cancel<()>) -> Message + 'a) -> Self {
        self.on_cancel = Some(Box::new(handler));
        self
    }
}

/// Helper to construct a non-shifting wizard footer element (`UIF-COMP-011`, `UIF-PRIN-004`).
///
/// On step 0, `[← Previous]` renders as `Disabled` rather than disappearing, ensuring
/// that button coordinates never shift across steps (0px shift per `UIF-PRIN-004`).
#[must_use]
pub fn wizard_footer<'a, Message: 'a + Clone, StepId: 'a + Clone + PartialEq>(
    model: &StepperModel<StepId>,
    config: WizardFooterConfig<'a, Message, StepId>,
) -> Element<'a, Message> {
    let palette = Palette::dark();

    // Cancel button (left aligned)
    let cancel_btn: Element<'a, Message> = if let Some(on_cancel) = config.on_cancel {
        let cancel_text = iced::widget::text(LABEL_CANCEL)
            .size(typography::CAPTION.size)
            .line_height(typography::CAPTION.iced_line_height())
            .color(palette.text_muted);

        iced::widget::button(cancel_text)
            .height(FOOTER_BUTTON_HEIGHT)
            .padding(iced::Padding {
                top: space::XXS,
                bottom: space::XXS,
                left: space::MD,
                right: space::MD,
            })
            .style(
                move |_theme: &iced::Theme, _status| iced::widget::button::Style {
                    background: None,
                    text_color: palette.text_muted,
                    border: iced::Border {
                        color: palette.border_subtle,
                        width: shape::BORDER_HAIRLINE,
                        radius: shape::RADIUS_WIDGET.into(),
                    },
                    shadow: iced::Shadow::default(),
                    snap: true,
                },
            )
            .on_press((on_cancel)(Cancel::new(())))
            .into()
    } else {
        iced::widget::Space::new().width(0.0).into()
    };

    // Previous button [← Previous] (disabled on step 0 per UIF-PRIN-004)
    let can_prev = model.can_previous();
    let prev_text = iced::widget::text(LABEL_PREVIOUS)
        .size(typography::CAPTION.size)
        .line_height(typography::CAPTION.iced_line_height())
        .color(if can_prev {
            palette.text_primary
        } else {
            palette.text_disabled
        });

    let mut prev_btn = iced::widget::button(prev_text)
        .height(FOOTER_BUTTON_HEIGHT)
        .padding(iced::Padding {
            top: space::XXS,
            bottom: space::XXS,
            left: space::MD,
            right: space::MD,
        })
        .style(
            move |_theme: &iced::Theme, _status| iced::widget::button::Style {
                background: if can_prev {
                    Some(iced::Background::Color(palette.surface_raised))
                } else {
                    None
                },
                text_color: if can_prev {
                    palette.text_primary
                } else {
                    palette.text_disabled
                },
                border: iced::Border {
                    color: palette.border_subtle,
                    width: shape::BORDER_HAIRLINE,
                    radius: shape::RADIUS_WIDGET.into(),
                },
                shadow: iced::Shadow::default(),
                snap: true,
            },
        );

    if let (Some(on_previous), Some(prev_id)) = (config.on_previous, model.previous_step_id()) {
        prev_btn = prev_btn.on_press((on_previous)(Commit::new(prev_id)));
    }

    // Next / Finish button
    let is_last = model.is_last_step();
    let next_or_finish_btn: Element<'a, Message> = if is_last {
        let can_finish = config.on_finish.is_some();
        let finish_text = iced::widget::text(LABEL_FINISH)
            .size(typography::CAPTION.size)
            .line_height(typography::CAPTION.iced_line_height())
            .color(if can_finish {
                palette.text_strong
            } else {
                palette.text_disabled
            });

        let mut btn = iced::widget::button(finish_text)
            .height(FOOTER_BUTTON_HEIGHT)
            .padding(iced::Padding {
                top: space::XXS,
                bottom: space::XXS,
                left: space::MD,
                right: space::MD,
            })
            .style(
                move |_theme: &iced::Theme, _status| iced::widget::button::Style {
                    background: if can_finish {
                        Some(iced::Background::Color(palette.accent_fill))
                    } else {
                        Some(iced::Background::Color(palette.surface_raised))
                    },
                    text_color: if can_finish {
                        palette.text_strong
                    } else {
                        palette.text_disabled
                    },
                    border: iced::Border {
                        color: if can_finish {
                            palette.accent_border
                        } else {
                            palette.border_subtle
                        },
                        width: shape::BORDER_HAIRLINE,
                        radius: shape::RADIUS_WIDGET.into(),
                    },
                    shadow: iced::Shadow::default(),
                    snap: true,
                },
            );

        if let Some(on_finish) = config.on_finish {
            btn = btn.on_press((on_finish)(Commit::new(())));
        }
        btn.into()
    } else {
        let can_next = model.can_next() && config.on_next.is_some();
        let next_text = iced::widget::text(LABEL_NEXT)
            .size(typography::CAPTION.size)
            .line_height(typography::CAPTION.iced_line_height())
            .color(if can_next {
                palette.text_strong
            } else {
                palette.text_disabled
            });

        let mut btn = iced::widget::button(next_text)
            .height(FOOTER_BUTTON_HEIGHT)
            .padding(iced::Padding {
                top: space::XXS,
                bottom: space::XXS,
                left: space::MD,
                right: space::MD,
            })
            .style(
                move |_theme: &iced::Theme, _status| iced::widget::button::Style {
                    background: if can_next {
                        Some(iced::Background::Color(palette.accent_fill))
                    } else {
                        Some(iced::Background::Color(palette.surface_raised))
                    },
                    text_color: if can_next {
                        palette.text_strong
                    } else {
                        palette.text_disabled
                    },
                    border: iced::Border {
                        color: if can_next {
                            palette.accent_border
                        } else {
                            palette.border_subtle
                        },
                        width: shape::BORDER_HAIRLINE,
                        radius: shape::RADIUS_WIDGET.into(),
                    },
                    shadow: iced::Shadow::default(),
                    snap: true,
                },
            );

        if let (Some(on_next), Some(next_id)) = (config.on_next, model.next_step_id()) {
            btn = btn.on_press((on_next)(Commit::new(next_id)));
        }
        btn.into()
    };

    let actions = iced::widget::row![prev_btn, next_or_finish_btn].spacing(STEP_GAP);

    iced::widget::row![
        cancel_btn,
        iced::widget::Space::new().width(Length::Fill),
        actions
    ]
    .width(Length::Fill)
    .align_y(iced::alignment::Vertical::Center)
    .into()
}

fn build_stepper_content<'a, Message: 'a + Clone, StepId: 'a + Clone + PartialEq>(
    model: &StepperModel<StepId>,
    orientation: StepperOrientation,
    on_step_select: Option<&(dyn Fn(Commit<StepId>) -> Message + 'a)>,
) -> Element<'a, Message> {
    let palette = Palette::dark();
    let num_steps = model.steps.len();

    match orientation {
        StepperOrientation::Horizontal => {
            let mut row = iced::widget::row![]
                .width(Length::Fill)
                .height(BAR_HEIGHT)
                .align_y(iced::alignment::Vertical::Center);

            for (i, step) in model.steps.iter().enumerate() {
                let is_selectable = model.can_select(&step.id) && on_step_select.is_some();
                let node_element = render_step_node(i, step, &palette);
                let label_element = render_step_label(step, &palette);

                let item_content = iced::widget::row![node_element, label_element]
                    .spacing(LABEL_GAP)
                    .align_y(iced::alignment::Vertical::Center);

                let step_element: Element<'a, Message> = if is_selectable {
                    let select_fn = on_step_select.unwrap();
                    let step_id = step.id.clone();
                    iced::widget::button(item_content)
                        .padding(iced::Padding::new(0.0))
                        .style(
                            |_theme: &iced::Theme, _status| iced::widget::button::Style {
                                background: None,
                                text_color: Color::TRANSPARENT,
                                border: iced::Border {
                                    color: Color::TRANSPARENT,
                                    width: 0.0,
                                    radius: 0.0.into(),
                                },
                                shadow: iced::Shadow::default(),
                                snap: true,
                            },
                        )
                        .on_press((select_fn)(Commit::new(step_id)))
                        .into()
                } else {
                    iced::widget::container(item_content).into()
                };

                row = row.push(step_element);

                // Connector line to next step
                if i + 1 < num_steps {
                    let connector_color = if step.status == StepStatus::Completed {
                        palette.accent_border
                    } else {
                        palette.border_subtle
                    };

                    let line =
                        iced::widget::container(iced::widget::Space::new().width(Length::Fill))
                            .width(Length::Fill)
                            .height(CONNECTOR_STROKE)
                            .style(move |_theme: &iced::Theme| iced::widget::container::Style {
                                text_color: None,
                                background: Some(iced::Background::Color(connector_color)),
                                border: iced::Border::default(),
                                shadow: iced::Shadow::default(),
                                snap: true,
                            });

                    let connector = iced::widget::container(line)
                        .width(Length::Fill)
                        .padding(iced::Padding {
                            top: 0.0,
                            bottom: 0.0,
                            left: STEP_GAP,
                            right: STEP_GAP,
                        })
                        .align_y(iced::alignment::Vertical::Center);

                    row = row.push(connector);
                }
            }

            row.into()
        }
        StepperOrientation::Vertical => {
            let mut col = iced::widget::column![].spacing(0);

            for (i, step) in model.steps.iter().enumerate() {
                let is_selectable = model.can_select(&step.id) && on_step_select.is_some();
                let node_element = render_step_node(i, step, &palette);
                let label_element = render_step_label(step, &palette);

                let item_content = iced::widget::row![node_element, label_element]
                    .spacing(LABEL_GAP)
                    .align_y(iced::alignment::Vertical::Center);

                let step_element: Element<'a, Message> = if is_selectable {
                    let select_fn = on_step_select.unwrap();
                    let step_id = step.id.clone();
                    iced::widget::button(item_content)
                        .padding(iced::Padding::new(0.0))
                        .style(
                            |_theme: &iced::Theme, _status| iced::widget::button::Style {
                                background: None,
                                text_color: Color::TRANSPARENT,
                                border: iced::Border {
                                    color: Color::TRANSPARENT,
                                    width: 0.0,
                                    radius: 0.0.into(),
                                },
                                shadow: iced::Shadow::default(),
                                snap: true,
                            },
                        )
                        .on_press((select_fn)(Commit::new(step_id)))
                        .into()
                } else {
                    iced::widget::container(item_content).into()
                };

                col = col.push(step_element);

                // Vertical connector line
                if i + 1 < num_steps {
                    let connector_color = if step.status == StepStatus::Completed {
                        palette.accent_border
                    } else {
                        palette.border_subtle
                    };

                    let line = iced::widget::container(
                        iced::widget::Space::new()
                            .width(CONNECTOR_STROKE)
                            .height(space::LG),
                    )
                    .style(move |_theme: &iced::Theme| {
                        iced::widget::container::Style {
                            text_color: None,
                            background: Some(iced::Background::Color(connector_color)),
                            border: iced::Border::default(),
                            shadow: iced::Shadow::default(),
                            snap: true,
                        }
                    });

                    // Offset so vertical line aligns with the center of the 24px node
                    let connector_box = iced::widget::container(line).padding(iced::Padding {
                        top: 0.0,
                        bottom: 0.0,
                        left: (NODE_SIZE - CONNECTOR_STROKE) / 2.0,
                        right: 0.0,
                    });

                    col = col.push(connector_box);
                }
            }

            col.into()
        }
    }
}

fn render_step_node<'a, Message: 'a, StepId>(
    index: usize,
    step: &StepItem<StepId>,
    palette: &Palette,
) -> Element<'a, Message> {
    let (glyph_str, bg_color, border_color, border_width, fg_color, font_weight) = match step.status
    {
        StepStatus::Completed => (
            GLYPH_COMPLETED.to_string(),
            palette.accent_fill,
            palette.accent_fill,
            shape::BORDER_HAIRLINE,
            palette.text_strong,
            typography::Weight::SemiBold,
        ),
        StepStatus::Current => (
            (index + 1).to_string(),
            palette.surface_active,
            palette.accent_border,
            CURRENT_BORDER_WIDTH,
            palette.text_strong,
            typography::Weight::SemiBold,
        ),
        StepStatus::Pending => (
            (index + 1).to_string(),
            palette.surface_raised,
            palette.border_strong,
            shape::BORDER_HAIRLINE,
            palette.text_muted,
            typography::Weight::Regular,
        ),
        StepStatus::Error => (
            GLYPH_ERROR.to_string(),
            palette.status_danger_fill,
            palette.status_danger_fill,
            shape::BORDER_HAIRLINE,
            palette.text_strong,
            typography::Weight::SemiBold,
        ),
    };

    let text_widget = iced::widget::text(glyph_str)
        .size(typography::CAPTION.size)
        .line_height(typography::CAPTION.iced_line_height())
        .font(iced::Font {
            weight: font_weight.iced(),
            ..iced::Font::with_name("Fira Sans")
        })
        .color(fg_color)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center);

    iced::widget::container(text_widget)
        .width(NODE_SIZE)
        .height(NODE_SIZE)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_theme: &iced::Theme| iced::widget::container::Style {
            text_color: None,
            background: Some(iced::Background::Color(bg_color)),
            border: iced::Border {
                color: border_color,
                width: border_width,
                radius: NODE_RADIUS.into(),
            },
            shadow: iced::Shadow::default(),
            snap: true,
        })
        .into()
}

fn render_step_label<'a, Message: 'a, StepId>(
    step: &StepItem<StepId>,
    palette: &Palette,
) -> Element<'a, Message> {
    let (text_color, font_weight) = if step.disabled {
        (palette.text_disabled, typography::Weight::Regular)
    } else {
        match step.status {
            StepStatus::Current => (palette.text_strong, typography::Weight::SemiBold),
            StepStatus::Completed => (palette.text_primary, typography::Weight::Medium),
            StepStatus::Error => (palette.status_danger_text, typography::Weight::Medium),
            StepStatus::Pending => (palette.text_muted, typography::Weight::Regular),
        }
    };

    iced::widget::text(step.label.clone())
        .size(typography::CAPTION.size)
        .line_height(typography::CAPTION.iced_line_height())
        .font(iced::Font {
            weight: font_weight.iced(),
            ..iced::Font::with_name("Fira Sans")
        })
        .color(text_color)
        .align_y(iced::alignment::Vertical::Center)
        .into()
}

/// The live stepper widget for multi-step workflows (`UIF-COMP-011`, `UIF-PRIN-005`).
pub struct Stepper<'a, Message, StepId, Theme = iced::Theme, Renderer = iced::Renderer> {
    model: StepperModel<StepId>,
    orientation: StepperOrientation,
    on_step_select: Option<Box<dyn Fn(Commit<StepId>) -> Message + 'a>>,
    on_next: Option<Box<dyn Fn(Commit<StepId>) -> Message + 'a>>,
    on_previous: Option<Box<dyn Fn(Commit<StepId>) -> Message + 'a>>,
    on_finish: Option<Box<dyn Fn(Commit<()>) -> Message + 'a>>,
    on_cancel: Option<Box<dyn Fn(Cancel<()>) -> Message + 'a>>,
    content: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message: 'a + Clone, StepId: 'a + Clone + PartialEq>
    Stepper<'a, Message, StepId, iced::Theme, iced::Renderer>
{
    /// Construct a new stepper widget with default horizontal orientation.
    #[must_use]
    pub fn new(model: StepperModel<StepId>) -> Self {
        let content = build_stepper_content(&model, StepperOrientation::Horizontal, None);
        Self {
            model,
            orientation: StepperOrientation::Horizontal,
            on_step_select: None,
            on_next: None,
            on_previous: None,
            on_finish: None,
            on_cancel: None,
            content,
        }
    }

    fn rebuild(mut self) -> Self {
        self.content = build_stepper_content(
            &self.model,
            self.orientation,
            self.on_step_select.as_deref(),
        );
        self
    }

    /// Set the orientation of the stepper (Horizontal or Vertical).
    #[must_use]
    pub fn orientation(mut self, orientation: StepperOrientation) -> Self {
        self.orientation = orientation;
        self.rebuild()
    }

    /// Set the callback for step selection.
    #[must_use]
    pub fn on_step_select(mut self, f: impl Fn(Commit<StepId>) -> Message + 'a) -> Self {
        self.on_step_select = Some(Box::new(f));
        self.rebuild()
    }

    /// Set the callback for advancing to the next step.
    #[must_use]
    pub fn on_next(mut self, f: impl Fn(Commit<StepId>) -> Message + 'a) -> Self {
        self.on_next = Some(Box::new(f));
        self
    }

    /// Set the callback for returning to the previous step.
    #[must_use]
    pub fn on_previous(mut self, f: impl Fn(Commit<StepId>) -> Message + 'a) -> Self {
        self.on_previous = Some(Box::new(f));
        self
    }

    /// Set the callback for finishing the workflow.
    #[must_use]
    pub fn on_finish(mut self, f: impl Fn(Commit<()>) -> Message + 'a) -> Self {
        self.on_finish = Some(Box::new(f));
        self
    }

    /// Set the callback for cancelling the workflow.
    #[must_use]
    pub fn on_cancel(mut self, f: impl Fn(Cancel<()>) -> Message + 'a) -> Self {
        self.on_cancel = Some(Box::new(f));
        self
    }

    /// Access the model driving this stepper.
    #[must_use]
    pub fn model(&self) -> &StepperModel<StepId> {
        &self.model
    }

    /// Access the orientation of this stepper.
    #[must_use]
    pub fn orientation_value(&self) -> StepperOrientation {
        self.orientation
    }
}

impl<Message, Theme, Renderer, StepId> Widget<Message, Theme, Renderer>
    for Stepper<'_, Message, StepId, Theme, Renderer>
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

impl<'a, Message: 'a + Clone, StepId: 'a, Theme: 'a, Renderer: 'a + renderer::Renderer>
    From<Stepper<'a, Message, StepId, Theme, Renderer>> for Element<'a, Message, Theme, Renderer>
{
    fn from(widget: Stepper<'a, Message, StepId, Theme, Renderer>) -> Self {
        Element::new(widget)
    }
}

/// Helper function to construct a stepper widget.
#[must_use]
pub fn stepper<'a, Message: 'a + Clone, StepId: 'a + Clone + PartialEq>(
    model: StepperModel<StepId>,
) -> Stepper<'a, Message, StepId> {
    Stepper::new(model)
}

#[cfg(test)]
#[path = "stepper_tests.rs"]
mod stepper_tests;
