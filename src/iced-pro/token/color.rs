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
//! # The polarity split
//!
//! Values are transcribed from `doc/planning/ui-framework.md` §2.5 as
//! revised on 2026-10-22, and that transcription is authoritative. The
//! revision re-baselines the vocabulary around a **polarity split**: a
//! chromatic signal is no longer one token every context reuses, but a
//! set of role-scoped tokens — the colour you fill *with*, the colour
//! you press *to*, and the colour you stroke *with*.
//!
//! `color.accent.*` carries three of them:
//! [`accent_fill`](Self::accent_fill) (the solid CTA / selected-row
//! fill), [`accent_fill_active`](Self::accent_fill_active) (the same
//! fill under a press) and [`accent_border`](Self::accent_border) (the
//! focus ring, the active tab indicator, the accent stroke).
//! `color.status.danger.*` carries the mirror-image three:
//! [`status_danger_text`](Self::status_danger_text) (the red you write
//! errors *in*), [`status_danger_fill`](Self::status_danger_fill) (the
//! red a destructive label sits *on*) and
//! [`status_danger_fill_active`](Self::status_danger_fill_active) (that
//! background under a press).
//!
//! That shape is not new to this crate — it is what two ledger decisions
//! argued for, and the revised spec has now adopted both outright:
//!
//! * **D-5** (gap **G-8**) split `status.danger` into a text red and a
//!   background red because the two roles demand opposite luminance and
//!   no single value can serve them. §2.5 now prints the split as
//!   structure rather than leaving it a crate-local addition, and goes
//!   further: the pressed step this crate had to synthesise as
//!   "`status.danger_fill` at 85% opacity" is its own
//!   `status.danger.fill_active`.
//! * **D-6** (gap **G-9**) made the focus ring adaptive because a fixed
//!   accent ring vanished on a saturated fill. §3.0.2 now states that
//!   rule directly — resolve to `accent.border`, fall back to
//!   `text.strong` — and, crucially, gives it a token to resolve *from*.
//!   Before the split there was no `accent.border` to try; the ring had
//!   to fall back from the fill token itself.
//!
//! The five pairings that decision **A** of the **G-7** ledger retuned
//! are no longer deviations either. The revised §2.5 prints the values
//! that clear the rules, so **every token in this module is the spec's
//! number verbatim** — there is no crate-local value left to reconcile.
//!
//! # One printed luminance is wrong, and it is recorded here
//!
//! §2.5 gives `color.status.danger.fill_active` a relative luminance of
//! `0.1309`. Recomputed from that row's own printed `RGB(202, 24, 16)`
//! through the WCAG 2.1 transfer function the true figure is `0.1325`.
//! The colour is right and the printed `L` is not; the gap is recorded
//! against **G-5**, whose standing finding is that §2.5's numeric
//! columns were hand-collected. Two of that row's contrast columns
//! inherit the error and are likewise off: `vs surface.base` prints
//! `3.43` where the colour measures `3.4575`, and `vs surface.active`
//! prints `2.46` against a measured `2.4842`. Per **G-5** the values
//! `token/contrast.rs` computes are authoritative and nothing in this
//! crate transcribes the printed figures.
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
    ///
    /// It is also the darkest place a disabled placeholder can appear,
    /// which matters to `UIF-TOK-006` rule 2: `text.disabled` measures
    /// `6.0205:1` here and breaches that rule's `6.00:1` ceiling. See
    /// the rule-2 known-conflict test in `token/contrast_tests.rs`.
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
    /// menu item, active submenu parent. `RGB(32, 32, 36)`.
    pub surface_hover: Color,

    /// `color.surface.active` — pressed quiet/secondary control fill and
    /// the movable overlay title bar header. `RGB(42, 42, 46)`.
    ///
    /// The lightest surface in the set, so the gap between it and the
    /// surface it sits on is the widest the elevation model ever gets.
    /// The revised §2.5 pulled hover up and this down relative to the
    /// pre-revision ladder, narrowing the `hover → active` luminance step
    /// to `0.0088` — still strictly increasing, and still enough that a
    /// pressed control reads as a step rather than a repaint.
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
    /// alignment alone, quiet enough that it disappears once read. Under
    /// `UIF-TOK-006` rule 3's four-class chrome model it is a
    /// `RestingDivider`, which is why the `3.00:1` active-chrome floor
    /// is not asserted against it.
    pub border_subtle: Color,

    /// `color.border.strong` — floating card perimeters (modals, menus,
    /// movable overlays) and the markdown table perimeter.
    /// `RGB(120, 120, 120)`.
    ///
    /// The stroke that says "this edge is a floating thing", which is
    /// why it is the border a detached card gets on every side.
    pub border_strong: Color,

    /// `color.accent.fill` — solid primary CTA button fill and selected
    /// tree-row background, always under [`text_strong`](Self::text_strong).
    /// `RGB(93, 96, 238)`.
    ///
    /// The fill half of the accent's polarity split. It is *only* ever a
    /// background: the `UIF-TOK-006` rule 4 foreground lock pins
    /// [`text_strong`](Self::text_strong) on it (`4.8020:1`) and forbids
    /// every foreground-only accent/status token from rendering on it.
    /// When this colour is needed as a *stroke*, reach for
    /// [`accent_border`](Self::accent_border) rather than reusing the
    /// fill — that is the whole reason the split exists.
    pub accent_fill: Color,

    /// `color.accent.fill_active` — the pressed primary CTA button fill,
    /// under [`text_strong`](Self::text_strong). `RGB(82, 85, 222)`.
    ///
    /// The step the pre-split palette had to fake with opacity. Because a
    /// primary CTA is the most-pressed control in the interface and its
    /// fill cannot change hue, the pressed depth had to become its own
    /// token rather than a modifier on the resting one.
    ///
    /// It is darker than [`accent_fill`](Self::accent_fill) rather than
    /// lighter, which is why the label it carries gains contrast instead
    /// of losing it: `text.strong` reads `5.6559:1` here against
    /// `4.8020:1` on the resting fill.
    pub accent_fill_active: Color,

    /// `color.accent.border` — keyboard focus rings, the active tab's
    /// top indicator, and pressed/focused accent strokes.
    /// `RGB(129, 140, 248)`.
    ///
    /// The stroke half of the accent's polarity split, and the token
    /// `UIF-ATOM-000` §2's dynamic ring rule resolves *from*: the ring
    /// is this colour wherever it clears `MIN_CHROME_CONTRAST`
    /// (`3.00:1`), and falls back to [`text_strong`](Self::text_strong)
    /// where it cannot.
    ///
    /// It is much lighter than [`accent_fill`](Self::accent_fill) —
    /// `L = 0.3020` against `0.1687` — which is what lets one token
    /// clear `3.00:1` against every surface in the ladder (worst case
    /// `4.7922:1` on `surface.active`) while still reading as the same
    /// indigo. The cost is that it cannot clear `3.00:1` against a
    /// *saturated* fill, which is exactly the case the adaptive fallback
    /// in `widget::focus` exists to handle.
    pub accent_border: Color,

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
    /// Also the **mandatory** foreground on all four saturated fills —
    /// [`accent_fill`](Self::accent_fill),
    /// [`accent_fill_active`](Self::accent_fill_active),
    /// [`status_danger_fill`](Self::status_danger_fill) and
    /// [`status_danger_fill_active`](Self::status_danger_fill_active) —
    /// under the `UIF-TOK-006` rule 4 foreground lock, which pins a
    /// selected row or primary button so it can never be re-skinned into
    /// an unreadable pairing. Across those four fills the pairing spans
    /// `4.8020:1` to `5.7543:1`.
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
    ///
    /// `UIF-TOK-006` rule 2 exempts it from the `4.50:1` active-text
    /// floor and holds it inside a `[4.00:1, 6.00:1]` muted-legibility
    /// band instead, so a disabled control stays readable without
    /// competing with the active ones. It clears that band on six of the
    /// seven surfaces; `surface.sunken` measures `6.0205:1` and breaches
    /// the ceiling by `0.0205`. That is a defect in the band, not in
    /// this token, and is pinned as a known conflict rather than tuned
    /// away — see `token/contrast_tests.rs`.
    pub text_disabled: Color,

    /// `color.status.success` — completion states and positive status
    /// chips (`✓ parallel`). `RGB(100, 255, 100)`.
    pub status_success: Color,

    /// `color.status.warning` — the unsaved indicator dot (`●`), warning
    /// icons (`⚠`) and the non-selected special-item tint.
    /// `RGB(220, 130, 0)`.
    pub status_warning: Color,

    /// `color.status.danger.text` — validation error **text and icons**,
    /// invalid input borders, and the resting `Danger` button
    /// label/border. `RGB(255, 92, 92)`.
    ///
    /// **This is the text/icon red. Reach for it when the danger is
    /// *written* — an error message, a validation glyph, a resting
    /// Danger label. It is not a background.** When the danger is the
    /// surface a label sits *on*, use
    /// [`status_danger_fill`](Self::status_danger_fill) instead.
    ///
    /// It is the foreground half of the danger polarity split **D-5**
    /// argued for, and the reason the split cannot collapse back into one
    /// token is arithmetic rather than taste: a red bright enough to read
    /// as body text on `surface.overlay` (`5.8528:1`) is far too bright
    /// to carry `text.strong` on top of it — the pairing measures
    /// `3.0271:1`, well under the `4.50:1` rule. The two roles pull
    /// luminance in opposite directions, which is why there are three
    /// danger tokens and not one.
    ///
    /// It is also one of the three *active* border tokens rule 3a judges
    /// against the `3.00:1` chrome floor, and it clears it across the
    /// whole ladder — worst case `4.7223:1` on `surface.active`.
    pub status_danger_text: Color,

    /// `color.status.danger.fill` — the destructive red used **as a
    /// background behind [`text_strong`](Self::text_strong)**: the
    /// hovered `Danger` button fill and the hovered destructive menu row.
    /// `RGB(216, 28, 20)`.
    ///
    /// **This is a background red. Reach for it when a label is painted
    /// *on* the danger — a filled destructive button or row. It is not
    /// a text colour**; for written errors use
    /// [`status_danger_text`](Self::status_danger_text).
    ///
    /// It sits on the low side of the disjoint luminance bounds **G-8**
    /// records: `L = 0.1548`, under the `0.183333` ceiling the
    /// white-on-red rule imposes, and below the `0.216697` floor the
    /// red-on-overlay text rule imposes. That is the whole argument for
    /// the split — no value satisfies both, so the roles are separated
    /// instead.
    ///
    /// It is not a surface and must never be mistaken for one. Every
    /// `surface.*` token is a near-neutral grey spanning at most `4`
    /// units of channel spread; this one spans `196`, and it clears every
    /// surface by at least `2.7881:1`, so it never luminance-matches
    /// the layer it is meant to stand out from.
    pub status_danger_fill: Color,

    /// `color.status.danger.fill_active` — the **pressed** destructive
    /// red, behind [`text_strong`](Self::text_strong).
    /// `RGB(202, 24, 16)`.
    ///
    /// The pressed step of the danger fill, mirroring
    /// [`accent_fill_active`](Self::accent_fill_active). Before the
    /// split this crate synthesised the same step as
    /// "`status_danger_fill` at 85% opacity"; the revised spec makes it
    /// a real token, which is better because an opacity fill inherits
    /// whatever is behind it and a token does not.
    ///
    /// Two things about this token are worth pinning:
    ///
    /// * Its printed luminance in §2.5 (`0.1309`) is wrong. The true
    ///   value for `RGB(202, 24, 16)` is `0.1325`. Recorded against
    ///   **G-5**; see this module's doc comment.
    /// * It is the token that breaks the `CoincidentWithFill` rule. A
    ///   `1px` stroke set to this same token takes its boundary contrast
    ///   from the fill, which must therefore reach `3.08:1` against the
    ///   surrounding surface — and against `surface.overlay` it measures
    ///   `3.0789:1`, short by `0.0011`. A pressed destructive row on a
    ///   floating menu is a real, ordinary case. This is a defect in the
    ///   rule's threshold, not in the colour, and it is pinned as a
    ///   known conflict in `token/contrast_tests.rs` rather than tuned
    ///   away.
    pub status_danger_fill_active: Color,
}

impl Palette {
    /// The dark palette exactly as `UIF-TOK-005`'s revised §2.5
    /// (2026-10-22) specifies it, including the six-token polarity split
    /// that decision **D-5** and decision **D-6** anticipated.
    ///
    /// Every value here is the spec's own number. The three tokens the
    /// earlier **G-7** retune moved — `accent.primary`, `surface.active`
    /// and `status.danger` — have been renamed and revalued to the
    /// figures the revised table prints, so there is no longer a
    /// crate-local deviation to track.
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
            surface_hover: Color::from_rgb8(32, 32, 36),
            surface_active: Color::from_rgb8(42, 42, 46),
            surface_scrim: Color::from_rgba8(9, 9, 11, 0.55),
            border_subtle: Color::from_rgb8(40, 40, 40),
            border_strong: Color::from_rgb8(120, 120, 120),
            accent_fill: Color::from_rgb8(93, 96, 238),
            accent_fill_active: Color::from_rgb8(82, 85, 222),
            accent_border: Color::from_rgb8(129, 140, 248),
            accent_text: Color::from_rgb8(100, 200, 255),
            text_primary: Color::from_rgb8(210, 210, 210),
            text_strong: Color::from_rgb8(255, 255, 255),
            text_muted: Color::from_rgb8(160, 160, 160),
            text_disabled: Color::from_rgb8(140, 140, 140),
            status_success: Color::from_rgb8(100, 255, 100),
            status_warning: Color::from_rgb8(220, 130, 0),
            status_danger_text: Color::from_rgb8(255, 92, 92),
            status_danger_fill: Color::from_rgb8(216, 28, 20),
            status_danger_fill_active: Color::from_rgb8(202, 24, 16),
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
