//! Semantic colour tokens: surfaces, borders, accents, text roles and
//! status colours (`UIF-TOK-005`).
//!
//! The palette is a dark-first elevation model. Every layer above the
//! root canvas is a progressively lighter neutral grey, and each one is
//! held apart from its neighbour by an explicit perimeter border rather
//! than by a shadow — elevation reads as lightness plus outline, which
//! is what keeps a floating menu legible wherever it lands.
//!
//! The token type is `iced::Color`, the same choice the FastMD app's own
//! palette makes (`GUD-003`), so a token here drops straight into an
//! iced style with no conversion step.
//!
//! Two structs, because the interface carries two irreconcilable
//! surfaces. [`Palette`] is the dark application chrome.
//! [`EditorPalette`] is the inverted white-on-black surface the inline
//! document editor borrows for the length of an edit, and stays a
//! separate, self-contained value rather than a flag on the dark palette:
//! it is a different colour world, not a variant of this one.
//!
//! Values are transcribed from `doc/planning/ui-framework.md` §2.5, and
//! that transcription is authoritative. Where the app's existing
//! `src/app/ui/theme.rs` disagrees with the spec — the editor error red
//! is the standing example — the spec's number wins (decision **D-4**
//! in `doc/planning/iced-pro-tasks.md`).
//!
//! What this module deliberately does *not* do is arithmetic on
//! contrast. The luminance and WCAG-ratio figures the spec's table
//! carries belong to `token/contrast.rs` (`UIF-TOK-006` /
//! `UIF-VER-001`); the tokens here are the values those rules are
//! measured against, and nothing more.
//!
//! Unit tests live in the sibling `color_tests.rs` sidecar.

use iced::Color;

/// The dark application palette (`UIF-TOK-005`).
///
/// One field per semantic colour token in `doc/planning/ui-framework.md`
/// §2.5. A field name is its token's last two segments in snake_case —
/// `color.surface.base` is [`Palette::surface_base`],
/// `color.border.strong` is [`Palette::border_strong`] — so the spec
/// token is recoverable from the field without a lookup table.
///
/// Read the fields by role, never by value. The point of the set is that
/// a widget asks for "hover fill" and gets whatever hover fill currently
/// means; a widget that compares against a channel value has stopped
/// reading the token and started re-deriving it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    /// `color.surface.base` — root window canvas and primary
    /// document/tree pane background. `RGB(9, 9, 11)`.
    ///
    /// The zero of the elevation scale: every other surface is stated as
    /// lightness *relative to base*, so moving this token moves the whole
    /// ladder with it.
    pub surface_base: Color,

    /// `color.surface.sunken` — deep input wells and scrollable log/code
    /// viewport backgrounds. `RGB(6, 6, 8)`.
    ///
    /// The one surface *below* base. It reads as a hole cut into whatever
    /// contains it, which is why it is reserved for content the user
    /// looks *into* rather than chrome the user looks *at*.
    pub surface_sunken: Color,

    /// `color.surface.raised` — navbar, bottom bar, inactive tabs and
    /// subtle list-row hover. `RGB(18, 18, 20)`.
    pub surface_raised: Color,

    /// `color.surface.code` — fenced code blocks and the resting
    /// background of a text input. `RGB(20, 20, 22)`.
    pub surface_code: Color,

    /// `color.surface.overlay` — floating surfaces: menus, input
    /// dialogs, modal dialogs, movable overlays and the active tab.
    /// `RGB(24, 24, 27)`.
    ///
    /// The highest surface a whole *card* rests on. Above it the two
    /// remaining surfaces are fills for individual controls and rows,
    /// not for containers.
    pub surface_overlay: Color,

    /// `color.surface.hover` — interactive control hover fill, hovered
    /// menu item, active submenu parent. `RGB(38, 38, 44)`.
    pub surface_hover: Color,

    /// `color.surface.active` — pressed control fill and the movable
    /// overlay title bar header. `RGB(50, 50, 58)`.
    ///
    /// The lightest surface in the set, so the gap between it and the
    /// surface it sits on is the widest the elevation model ever gets.
    pub surface_active: Color,

    /// `color.surface.scrim` — the dimmed backdrop behind blocking
    /// modal dialogs. `RGBA(9, 9, 11, 0.55)`.
    ///
    /// This is [`surface_base`](Self::surface_base) at 55% alpha, on
    /// purpose: the scrim is not a surface you paint *instead of* the
    /// canvas, it is the canvas pulled forward and dimmed so the modal
    /// card above it reads as the only thing in front. It **composites**
    /// over whatever is underneath rather than replacing it, so the
    /// workspace stays visible, and the composited result depends on
    /// what was painted below — the alpha is the rule, the resulting
    /// colour is not a token.
    pub surface_scrim: Color,

    /// `color.border.subtle` — pane split lines, menu separators,
    /// resting input borders and inactive tab borders. `RGB(40, 40, 40)`.
    ///
    /// The resting stroke: present so structure is never implied by
    /// alignment alone, quiet enough that it disappears once read.
    pub border_subtle: Color,

    /// `color.border.strong` — floating card perimeters (modals, menus,
    /// movable overlays) and the markdown table perimeter.
    /// `RGB(120, 120, 120)`.
    ///
    /// The stroke that says "this edge is a floating thing", which is
    /// why it is the border a detached card gets on every side.
    pub border_strong: Color,

    /// `color.accent.primary` — primary CTA fill, selected row
    /// background, keyboard focus ring and the active tab's top
    /// indicator. `RGB(99, 102, 241)`.
    ///
    /// The only colour the interface uses to say "act here". Because it
    /// is a fill, the `UIF-TOK-006` foreground lock applies: text on
    /// it is always [`text_strong`](Self::text_strong), never a status
    /// colour.
    pub accent_primary: Color,

    /// `color.accent.text` — headings, active links, menu checkmarks
    /// (`✓`) and submenu indicators (`>`). `RGB(100, 200, 255)`.
    ///
    /// A text-role accent rather than a fill-role one: it is bright
    /// enough to read as type, which the primary accent is not.
    pub accent_text: Color,

    /// `color.text.primary` — default body text and resting control
    /// labels. `RGB(210, 210, 210)`.
    pub text_primary: Color,

    /// `color.text.strong` — emphasised text, hovered and focused control
    /// labels. `RGB(255, 255, 255)`.
    ///
    /// Also the **mandatory** foreground on
    /// [`accent_primary`](Self::accent_primary): the selection
    /// foreground lock (`UIF-TOK-006`) pins this token there so a
    /// selected row or primary button can never be re-skinned into an
    /// unreadable pairing.
    pub text_strong: Color,

    /// `color.text.muted` — secondary descriptions, inactive tab
    /// labels, keyboard shortcut hints and section headings.
    /// `RGB(160, 160, 160)`.
    pub text_muted: Color,

    /// `color.text.disabled` — disabled button, menu and input text, and
    /// empty placeholders. `RGB(140, 140, 140)`.
    ///
    /// The dimmest text role, and still the brightest thing on the
    /// darkest surface: disabled means *present but inert*
    /// (`UIF-PRIN-004`), not gone.
    pub text_disabled: Color,

    /// `color.status.success` — completion states and positive status
    /// chips (`✓ parallel`). `RGB(100, 255, 100)`.
    pub status_success: Color,

    /// `color.status.warning` — the unsaved indicator dot (`●`), warning
    /// icons (`⚠`) and the non-selected special-item tint.
    /// `RGB(220, 130, 0)`.
    pub status_warning: Color,

    /// `color.status.danger` — validation error text and borders, and
    /// the destructive-action hover/CTA. `RGB(244, 15, 2)`.
    pub status_danger: Color,
}

impl Palette {
    /// The dark palette exactly as `UIF-TOK-005` specifies it.
    ///
    /// A `const fn` rather than a `static`: the palette is a value, and
    /// call sites are expected to take it by copy and read fields off it
    /// rather than share one global instance.
    #[must_use]
    pub const fn dark() -> Self {
        Self {
            surface_base: Color::from_rgb8(9, 9, 11),
            surface_sunken: Color::from_rgb8(6, 6, 8),
            surface_raised: Color::from_rgb8(18, 18, 20),
            surface_code: Color::from_rgb8(20, 20, 22),
            surface_overlay: Color::from_rgb8(24, 24, 27),
            surface_hover: Color::from_rgb8(38, 38, 44),
            surface_active: Color::from_rgb8(50, 50, 58),
            surface_scrim: Color::from_rgba8(9, 9, 11, 0.55),
            border_subtle: Color::from_rgb8(40, 40, 40),
            border_strong: Color::from_rgb8(120, 120, 120),
            accent_primary: Color::from_rgb8(99, 102, 241),
            accent_text: Color::from_rgb8(100, 200, 255),
            text_primary: Color::from_rgb8(210, 210, 210),
            text_strong: Color::from_rgb8(255, 255, 255),
            text_muted: Color::from_rgb8(160, 160, 160),
            text_disabled: Color::from_rgb8(140, 140, 140),
            status_success: Color::from_rgb8(100, 255, 100),
            status_warning: Color::from_rgb8(220, 130, 0),
            status_danger: Color::from_rgb8(244, 15, 2),
        }
    }
}

/// The inverted high-contrast surface of the inline document editor
/// (`color.editor.inverted.*`, `UIF-TOK-005`).
///
/// The editor has to look unmistakably different from the dark reading
/// surface it replaces: raw editing on white, reading in the dark. That
/// is a separate colour world rather than a mode flag on
/// [`Palette`], so it is carried as its own self-contained token set —
/// the same shape the app's own `EditorColors` uses.
///
/// [`Default`] is the inverted set, because inversion is the *only*
/// variant this surface has: there is no dark editor to fall back to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditorPalette {
    /// `color.editor.inverted.bg` — the editor's canvas.
    /// `RGB(255, 255, 255)`.
    pub background: Color,

    /// `color.editor.inverted.fg` — the buffer text. `RGB(0, 0, 0)`.
    pub text: Color,

    /// `color.editor.inverted.err` — validation and save-error text.
    /// `RGB(180, 0, 0)`.
    ///
    /// **Note the divergence from the app:** the spec's editor error is
    /// `RGB(180, 0, 0)`, not the `RGB(255, 0, 0)` the current
    /// `src/app/ui/theme.rs` editor palette uses. Per **D-4** the spec
    /// value wins here, and the darker red is the better choice on
    /// white regardless: it clears the 4.5:1 normal-text rule at about
    /// `7.14:1`, where pure red lands near `4.00:1` and fails it.
    pub error: Color,
}

impl EditorPalette {
    /// The inverted (black text on white) editor palette.
    #[must_use]
    pub const fn inverted() -> Self {
        Self {
            background: Color::from_rgb8(255, 255, 255),
            text: Color::from_rgb8(0, 0, 0),
            error: Color::from_rgb8(180, 0, 0),
        }
    }
}

impl Default for EditorPalette {
    fn default() -> Self {
        Self::inverted()
    }
}

#[cfg(test)]
#[path = "color_tests.rs"]
mod color_tests;
