//! The keyboard shortcut recorder and key chord interceptor composite widget (`UIF-COMP-015`, `UIF-PRIN-005`, `UIF-PRIN-006`).
//!
//! §3B.15 specifies an interactive shortcut capture input featuring:
//! - Visual dimensions: 32px height ([`HEIGHT`], [`size::CONTROL_MD`]), `surface.code` fill,
//!   `1px border.strong` ([`BORDER_REST_WIDTH`]) at rest or `2px accent.border` ([`BORDER_FOCUS_WIDTH`])
//!   when listening.
//! - Keycap pills: 24px high pills ([`KEYCAP_HEIGHT`], [`size::HIT_TARGET_MIN`]) in [`typography::FontFamily::Mono`]
//!   (e.g. `[Ctrl] + [Shift] + [P]`), `surface.raised` fill, `1px border.subtle`.
//! - Trailing 24×24px `[× Clear]` action button ([`CLEAR_BUTTON_SIZE`], [`GLYPH_CLEAR`]).
//! - Interaction lifecycle (§3.0.4.2 tiers):
//!   - When listening: raw key presses are captured into a [`KeyChord`], emitting
//!     `on_chord_preview(Live<KeyChord>)` on each keystroke and `on_chord_commit(Commit<Option<KeyChord>>)`
//!     when a non-modifier key is pressed.
//!   - `Escape`: cancels listening without clearing the existing chord, emitting `on_cancel(Cancel<()>)`.
//!   - `Backspace` / `Delete`: clears chord to `None`, emitting `on_chord_commit(Commit<Option<KeyChord>>)`.
//!   - Conflict surfacing: any conflict reported in [`KeybindRecorderModel::conflict_with`] is displayed
//!     in `status.danger.text`.
//!
//! Unit tests live in the sibling `keybind_recorder_tests.rs` sidecar.

use crate::binding::{Cancel, Commit, Live};
use crate::token::color::Palette;
use crate::token::{shape, size, space, typography};
use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::Operation;
use iced::advanced::widget::tree::Tree;
use iced::advanced::{Clipboard, Shell, Widget};
use iced::alignment;
use iced::keyboard::Key;
use iced::keyboard::key::Named;
use iced::mouse;
use iced::{Color, Element, Length, Rectangle, Size, Vector};

// ---------------------------------------------------------------------------
// Surface Geometry Constants & Tokens (§3B.15, `UIF-COMP-015`)
// ---------------------------------------------------------------------------

/// Overall height of the recorder input — `size::CONTROL_MD` (32px, §3B.15, `UIF-COMP-015`).
pub const HEIGHT: f32 = size::CONTROL_MD;

/// Height of each keycap pill — `size::HIT_TARGET_MIN` (24px, §3B.15, `UIF-COMP-015`).
pub const KEYCAP_HEIGHT: f32 = size::HIT_TARGET_MIN;

/// Size of the trailing clear button — `size::HIT_TARGET_MIN` (24px, §3B.15, `UIF-COMP-015`).
pub const CLEAR_BUTTON_SIZE: f32 = size::HIT_TARGET_MIN;

/// Outer container corner radius — `shape::RADIUS_WIDGET` (4px, §3B.15, `UIF-PRIN-006`).
pub const RADIUS: f32 = shape::RADIUS_WIDGET;

/// Resting border hairline width — `shape::BORDER_HAIRLINE` (1px, §3B.15, `UIF-PRIN-006`).
pub const BORDER_REST_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Listening/focus border width — `shape::BORDER_FOCUS` (2px, §3B.15, `UIF-PRIN-006`).
pub const BORDER_FOCUS_WIDTH: f32 = shape::BORDER_FOCUS;

/// Keycap pill corner radius — `shape::RADIUS_WIDGET` (4px, §3B.15).
pub const KEYCAP_RADIUS: f32 = shape::RADIUS_WIDGET;

/// Horizontal padding inside each keycap pill — `space::XS` (6px, §3B.15).
pub const KEYCAP_PADDING_X: f32 = space::XS;

/// Horizontal gap between keycaps — `space::XXS` (4px, §3B.15).
pub const KEYCAP_GAP: f32 = space::XXS;

/// Horizontal internal container padding — `space::SM` (8px, §3B.15).
pub const PADDING_X: f32 = space::SM;

/// Glyph displayed on the clear button — `"×"` (§3B.15).
pub const GLYPH_CLEAR: &str = "×";

/// Prompt displayed while listening for shortcut keys — `"Press shortcut keys..."` (§3B.15).
pub const LABEL_RECORDING: &str = "Press shortcut keys...";

/// Prompt displayed when no chord is recorded and not listening — `"Record shortcut"` (§3B.15).
pub const LABEL_EMPTY: &str = "Record shortcut";

/// Plus separator between keycaps — `"+"` (§3B.15).
pub const PLUS_SEPARATOR: &str = "+";

// ---------------------------------------------------------------------------
// Model (§3B.15, `UIF-COMP-015`)
// ---------------------------------------------------------------------------

/// A structured keyboard shortcut combination (§3B.15, `UIF-COMP-015`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct KeyChord {
    /// Whether Control modifier is pressed.
    pub ctrl: bool,
    /// Whether Alt modifier is pressed.
    pub alt: bool,
    /// Whether Shift modifier is pressed.
    pub shift: bool,
    /// Whether Super / Meta / Logo modifier is pressed.
    pub super_key: bool,
    /// Base key name (e.g. `"P"`, `"F1"`, `"Enter"`).
    pub key: String,
}

impl KeyChord {
    /// Construct a new key chord (§3B.15, `UIF-COMP-015`).
    pub fn new(
        ctrl: bool,
        alt: bool,
        shift: bool,
        super_key: bool,
        key: impl Into<String>,
    ) -> Self {
        Self {
            ctrl,
            alt,
            shift,
            super_key,
            key: key.into(),
        }
    }

    /// Return the individual string parts of this chord in canonical modifier order.
    #[must_use]
    pub fn parts(&self) -> Vec<String> {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl".to_string());
        }
        if self.alt {
            parts.push("Alt".to_string());
        }
        if self.shift {
            parts.push("Shift".to_string());
        }
        if self.super_key {
            parts.push("Super".to_string());
        }
        if !self.key.is_empty() {
            parts.push(self.key.clone());
        }
        parts
    }

    /// Format the chord as a human-readable string (e.g. `"Ctrl + Shift + P"`).
    #[must_use]
    pub fn format(&self) -> String {
        self.parts().join(" + ")
    }
}

/// Model driving the [`KeybindRecorder`] composite widget (§3B.15, `UIF-COMP-015`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindRecorderModel {
    /// Currently assigned key chord, if any.
    pub chord: Option<KeyChord>,
    /// Whether the recorder is actively listening for raw key presses.
    pub listening: bool,
    /// Optional conflicting command or shortcut name.
    pub conflict_with: Option<String>,
    /// Whether the widget is disabled from user interaction.
    pub disabled: bool,
}

impl Default for KeybindRecorderModel {
    fn default() -> Self {
        Self::new()
    }
}

impl KeybindRecorderModel {
    /// Create a new keybind recorder model in resting state (§3B.15, `UIF-COMP-015`).
    #[must_use]
    pub fn new() -> Self {
        Self {
            chord: None,
            listening: false,
            conflict_with: None,
            disabled: false,
        }
    }

    /// Set the recorded chord.
    #[must_use]
    pub fn with_chord(mut self, chord: Option<KeyChord>) -> Self {
        self.chord = chord;
        self
    }

    /// Set whether the recorder is actively listening.
    #[must_use]
    pub fn with_listening(mut self, listening: bool) -> Self {
        self.listening = listening;
        self
    }

    /// Set an optional conflicting command name.
    #[must_use]
    pub fn with_conflict(mut self, conflict: Option<String>) -> Self {
        self.conflict_with = conflict;
        self
    }

    /// Set whether the widget is disabled.
    #[must_use]
    pub fn with_disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

// ---------------------------------------------------------------------------
// Content Builder (§3B.15, `UIF-COMP-015`, `UIF-PRIN-005`, `UIF-PRIN-006`)
// ---------------------------------------------------------------------------

fn build_keybind_recorder_content<'a, Message: 'a + Clone>(
    model: &KeybindRecorderModel,
    on_chord_commit: Option<&(dyn Fn(Commit<Option<KeyChord>>) -> Message + 'a)>,
) -> Element<'a, Message> {
    let palette = Palette::dark();

    // Keycaps or prompt on left
    let mut keycaps_row = iced::widget::row![]
        .spacing(KEYCAP_GAP)
        .align_y(alignment::Vertical::Center);

    if model.listening {
        let recording_label = iced::widget::text(LABEL_RECORDING)
            .size(typography::UI_COMPACT.size)
            .line_height(typography::UI_COMPACT.iced_line_height())
            .font(typography::FAMILY_MONO.regular())
            .color(palette.text_strong);
        keycaps_row = keycaps_row.push(recording_label);
    } else if let Some(chord) = &model.chord {
        let parts = chord.parts();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                let sep = iced::widget::text(PLUS_SEPARATOR)
                    .size(typography::CAPTION.size)
                    .line_height(typography::CAPTION.iced_line_height())
                    .color(palette.text_muted);
                keycaps_row = keycaps_row.push(sep);
            }

            let pill_label = iced::widget::text(part.clone())
                .size(typography::CAPTION.size)
                .line_height(typography::CAPTION.iced_line_height())
                .font(typography::FAMILY_MONO.regular())
                .color(if model.disabled {
                    palette.text_disabled
                } else {
                    palette.text_strong
                });

            let pill = iced::widget::container(pill_label)
                .height(KEYCAP_HEIGHT)
                .padding(iced::Padding {
                    top: 0.0,
                    bottom: 0.0,
                    left: KEYCAP_PADDING_X,
                    right: KEYCAP_PADDING_X,
                })
                .align_y(alignment::Vertical::Center)
                .style(move |_theme: &iced::Theme| iced::widget::container::Style {
                    background: Some(iced::Background::Color(palette.surface_raised)),
                    border: iced::Border {
                        color: palette.border_subtle,
                        width: shape::BORDER_HAIRLINE,
                        radius: KEYCAP_RADIUS.into(),
                    },
                    ..Default::default()
                });

            keycaps_row = keycaps_row.push(pill);
        }
    } else {
        let empty_label = iced::widget::text(LABEL_EMPTY)
            .size(typography::UI_COMPACT.size)
            .line_height(typography::UI_COMPACT.iced_line_height())
            .color(if model.disabled {
                palette.text_disabled
            } else {
                palette.text_muted
            });
        keycaps_row = keycaps_row.push(empty_label);
    }

    // Trailing clear button (24x24px)
    let clear_btn: Element<'a, Message> =
        if !model.disabled && model.chord.is_some() && !model.listening {
            let clear_label = iced::widget::text(GLYPH_CLEAR)
                .size(typography::UI_COMPACT.size)
                .line_height(typography::UI_COMPACT.iced_line_height())
                .color(palette.text_muted)
                .align_x(alignment::Horizontal::Center)
                .align_y(alignment::Vertical::Center);

            let mut btn = iced::widget::button(clear_label)
                .width(CLEAR_BUTTON_SIZE)
                .height(CLEAR_BUTTON_SIZE)
                .padding(0)
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
                            color: Color::TRANSPARENT,
                            width: 0.0,
                            radius: RADIUS.into(),
                        },
                        shadow: iced::Shadow::default(),
                        snap: true,
                    }
                });

            if let Some(on_commit) = on_chord_commit {
                btn = btn.on_press((on_commit)(Commit::new(None)));
            }
            btn.into()
        } else {
            iced::widget::Space::new().width(0).into()
        };

    let inner_row = iced::widget::row![
        keycaps_row,
        iced::widget::Space::new().width(Length::Fill),
        clear_btn
    ]
    .height(HEIGHT)
    .padding(iced::Padding {
        top: 0.0,
        bottom: 0.0,
        left: PADDING_X,
        right: space::XS,
    })
    .align_y(alignment::Vertical::Center);

    let border_color = if model.disabled {
        palette.border_subtle
    } else if model.listening {
        palette.accent_border
    } else {
        palette.border_strong
    };

    let border_width = if model.listening {
        BORDER_FOCUS_WIDTH
    } else {
        BORDER_REST_WIDTH
    };

    let recorder_box = iced::widget::container(inner_row)
        .width(Length::Fill)
        .height(HEIGHT)
        .padding(0)
        .style(move |_theme: &iced::Theme| iced::widget::container::Style {
            background: Some(iced::Background::Color(palette.surface_code)),
            border: iced::Border {
                color: border_color,
                width: border_width,
                radius: RADIUS.into(),
            },
            ..Default::default()
        });

    if let Some(conflict) = &model.conflict_with {
        let conflict_label = iced::widget::text(format!("Conflict: {conflict}"))
            .size(typography::CAPTION.size)
            .line_height(typography::CAPTION.iced_line_height())
            .color(palette.status_danger_text);

        iced::widget::column![recorder_box, conflict_label]
            .spacing(space::XXS)
            .width(Length::Fill)
            .into()
    } else {
        recorder_box.into()
    }
}

// ---------------------------------------------------------------------------
// Widget Implementation (§3B.15, `UIF-COMP-015`)
// ---------------------------------------------------------------------------

/// Callback type for chord commit emissions (§3B.15, `UIF-COMP-015`).
pub type ChordCommitCallback<'a, Message> = Box<dyn Fn(Commit<Option<KeyChord>>) -> Message + 'a>;

/// The keybind recorder composite widget (§3B.15, `UIF-COMP-015`).
pub struct KeybindRecorder<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    model: KeybindRecorderModel,
    on_chord_preview: Option<Box<dyn Fn(Live<KeyChord>) -> Message + 'a>>,
    on_chord_commit: Option<ChordCommitCallback<'a, Message>>,
    on_cancel: Option<Box<dyn Fn(Cancel<()>) -> Message + 'a>>,
    on_start_listen: Option<Box<dyn Fn() -> Message + 'a>>,
    content: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message: 'a + Clone> KeybindRecorder<'a, Message, iced::Theme, iced::Renderer> {
    /// Construct a new [`KeybindRecorder`] widget with the given model (§3B.15, `UIF-COMP-015`).
    #[must_use]
    pub fn new(model: KeybindRecorderModel) -> Self {
        let content = build_keybind_recorder_content(&model, None);
        Self {
            model,
            on_chord_preview: None,
            on_chord_commit: None,
            on_cancel: None,
            on_start_listen: None,
            content,
        }
    }

    fn rebuild(mut self) -> Self {
        self.content = build_keybind_recorder_content(&self.model, self.on_chord_commit.as_deref());
        self
    }

    /// Set callback emitted on live raw chord preview keystrokes.
    #[must_use]
    pub fn on_chord_preview(mut self, f: impl Fn(Live<KeyChord>) -> Message + 'a) -> Self {
        self.on_chord_preview = Some(Box::new(f));
        self
    }

    /// Set callback emitted when a chord is settled and committed.
    #[must_use]
    pub fn on_chord_commit(mut self, f: impl Fn(Commit<Option<KeyChord>>) -> Message + 'a) -> Self {
        self.on_chord_commit = Some(Box::new(f));
        self.rebuild()
    }

    /// Set callback emitted when recording is cancelled via Escape.
    #[must_use]
    pub fn on_cancel(mut self, f: impl Fn(Cancel<()>) -> Message + 'a) -> Self {
        self.on_cancel = Some(Box::new(f));
        self
    }

    /// Set callback emitted when clicking the widget to begin listening.
    #[must_use]
    pub fn on_start_listen(mut self, f: impl Fn() -> Message + 'a) -> Self {
        self.on_start_listen = Some(Box::new(f));
        self.rebuild()
    }

    /// Access the underlying model.
    #[must_use]
    pub fn model(&self) -> &KeybindRecorderModel {
        &self.model
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for KeybindRecorder<'_, Message, Theme, Renderer>
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
        match event {
            iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. })
                if !self.model.disabled && self.model.listening =>
            {
                shell.capture_event();
                match key {
                    Key::Named(Named::Escape) => {
                        if let Some(on_cancel) = &self.on_cancel {
                            shell.publish((on_cancel)(Cancel::new(())));
                        }
                    }
                    Key::Named(Named::Backspace) | Key::Named(Named::Delete) => {
                        if let Some(on_commit) = &self.on_chord_commit {
                            shell.publish((on_commit)(Commit::new(None)));
                        }
                    }
                    Key::Named(Named::Control)
                    | Key::Named(Named::Alt)
                    | Key::Named(Named::Shift)
                    | Key::Named(Named::Super) => {
                        let chord = KeyChord {
                            ctrl: modifiers.control(),
                            alt: modifiers.alt(),
                            shift: modifiers.shift(),
                            super_key: modifiers.logo(),
                            key: String::new(),
                        };
                        if let Some(on_preview) = &self.on_chord_preview {
                            shell.publish((on_preview)(Live::new(chord)));
                        }
                    }
                    _ => {
                        let key_name = match key {
                            Key::Character(s) => s.to_uppercase(),
                            Key::Named(n) => format!("{n:?}"),
                            _ => return,
                        };
                        let chord = KeyChord {
                            ctrl: modifiers.control(),
                            alt: modifiers.alt(),
                            shift: modifiers.shift(),
                            super_key: modifiers.logo(),
                            key: key_name,
                        };
                        if let Some(on_preview) = &self.on_chord_preview {
                            shell.publish((on_preview)(Live::new(chord.clone())));
                        }
                        if let Some(on_commit) = &self.on_chord_commit {
                            shell.publish((on_commit)(Commit::new(Some(chord))));
                        }
                    }
                }
                return;
            }
            iced::Event::Mouse(iced::mouse::Event::ButtonPressed(iced::mouse::Button::Left))
                if !self.model.disabled
                    && !self.model.listening
                    && cursor.is_over(layout.bounds()) =>
            {
                if let Some(on_start) = &self.on_start_listen {
                    shell.publish((on_start)());
                    shell.capture_event();
                    return;
                }
            }
            _ => {}
        }

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
        if !self.model.disabled && !self.model.listening && cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            self.content.as_widget().mouse_interaction(
                &tree.children[0],
                layout,
                cursor,
                viewport,
                renderer,
            )
        }
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

impl<'a, Message: 'a + Clone> From<KeybindRecorder<'a, Message>> for Element<'a, Message> {
    fn from(widget: KeybindRecorder<'a, Message>) -> Self {
        Element::new(widget)
    }
}

/// Helper to construct a [`KeybindRecorder`] composite widget (§3B.15, `UIF-COMP-015`).
#[must_use]
pub fn keybind_recorder<'a, Message: 'a + Clone>(
    model: KeybindRecorderModel,
) -> KeybindRecorder<'a, Message> {
    KeybindRecorder::new(model)
}

#[cfg(test)]
#[path = "keybind_recorder_tests.rs"]
mod keybind_recorder_tests;
