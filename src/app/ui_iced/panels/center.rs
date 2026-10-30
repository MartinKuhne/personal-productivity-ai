//! Center panel — the document tab strip and the viewer area
//! (UI-026..UI-034, UI-048, UI-049).
//!
//! Like [`top`](super::top) and [`left`](super::left), the panel is a
//! **pure view**: it renders a [`CenterPanelViewModel`] snapshot into an
//! [`iced::Element`] of [`Message`] and *emits* [`Message::Command`] for
//! user intent. It never touches the `UserCommand` bus, never reads a file
//! and never mutates `FastMdApp` — the reducer does all of that, so every
//! intent is self-contained and replayable.
//!
//! # Shape
//!
//! ```text
//! [ ●note.md × ][ other.md × ][ notes.md × ]   <- tab strip (UI-049)
//! ┌──────────────────────────────────────────┐
//! │ document viewer area                     │
//! └──────────────────────────────────────────┘
//! ```
//!
//! The strip is **always allocated**, including when no tab is open (it is
//! then an empty band) and including when the agent session owns the pane —
//! the band's shape never changes across those transitions.
//!
//! # Tab interactions → messages
//!
//! | Interaction | Emitted |
//! | --- | --- |
//! | left-click tab label | `Command(SelectFile { path, multi: false })` |
//! | middle-click tab | `Command(CloseTab(index))` |
//! | `×` button | `Command(CloseTab(index))` |
//! | right-click tab | `Tab(ContextMenuOpened { index })` |
//! | pointer motion over the app | `Layout(CursorMoved { point })` (root-space menu anchor) |
//!
//! **Tab selection is a command, not view state.** The panel keeps no active
//! tab index of its own — the active tab *is* `selection().selected_file()`,
//! so a click emits the `SelectFile` intent and the active flag is derived
//! from the snapshot. There is deliberately no
//! `TabMessage::TabSelected` variant ([`message`](crate::ui_iced::message)
//! routing rule 1).
//!
//! The `×` button sits **inside** the tab's `MouseArea`. That is safe:
//! `MouseArea::update` updates its child first and returns without touching
//! its own handlers when the child captured the event
//! (`iced_widget-0.14.2/src/mouse_area.rs:230-245`), and `Button::update`
//! calls `shell.capture_event()` on press (`button.rs:312`). So clicking
//! `×` closes the tab without also selecting it.
//!
//! # Tab context menu (UI-026..UI-034)
//!
//! [`tab_context_menu_items`] is the single definition of the tab menu. Since
//! INT-3b it emits [`ContextRow`]s in §4.9.1's **group order** rather than
//! the old flat order: **Tab Lifecycle** (Close Tab, Close Others, Close All
//! Tabs), then **Edit & Automation** (Edit, Open in Editor, then the
//! VFS-121 note skills), then **Location** (Copy path, Show in File
//! Explorer).
//!
//! **UI-033 "Format Markdown" is not a `UserCommand`.** It is a seeded
//! VFS-121 *note skill* (`Skills/Note/FormatMarkdown.md`) whose click reads
//! the skill file and publishes
//! `UserCommand::RunSkillPrompt { content, target_dir: None, target_file }`.
//! A pure view may not read files, so the row emits the **existing**
//! [`TreeMessage::SkillRunRequested`](crate::ui_iced::message::TreeMessage::SkillRunRequested)
//! deferred-intent variant — the same one
//! the file-tree's note-skill rows emit. Reusing it rather than adding a
//! `TabMessage` twin keeps UI-034's "consistent behaviour across both menu
//! locations" and keeps `TabMessage: Copy` intact. The reducer routes both
//! variants through one handler.
//!
//! Menu **separators** used to be unrepresentable: the overlay card was a
//! uniform column of buttons whose `MenuItem` rows had no separator kind, so
//! the grouping was carried by row order alone. That gap is closed — each row
//! declares its [`ContextGroup`] and the framework menu engine emits a real
//! `1px border.subtle` divider at every group boundary (§4.9.1).
//!
//! # UI-048 — the thinking delimiter
//!
//! The `🤔…🤔` extraction is NOT a center-panel concern: it lives in
//! `AgentTranscript::apply_event` / `split_thinking_and_content`
//! (`src/app/ui/agent/transcript.rs`) and in the agent's response formatter
//! (AGENT-022). What the center panel owns is the *gate* — show the
//! "Thinking Process" block only when the buffer is non-empty — expressed
//! here as the pure [`thinking_section_visible`]. The collapsible disclosure
//! and the response markdown belong to the agent-session render path; this
//! module renders the band's header, status and thinking text and stops
//! there.
//!
//! # Deferred elsewhere
//!
//! * **Document rendering.** The viewer area renders the active tab's title
//!   only. The YAML front-matter table, the search-aware markdown renderer,
//!   the FTWA table strategy and the task-checkbox toggles all belong to
//!   that render path.
//! * **Agent session body.** Header, close intent and the UI-048 gate are
//!   here; the streaming response view is not.
//! * **Unsaved-changes marker and dirty-close guard (UI-027/UI-029, §4.4.1 slot
//!   2, `UIF-TAB-003`).** Not implementable under this app's editing model, and
//!   removed rather than left as a placeholder. Nothing in the crate can mark a
//!   tab dirty: the only thing that edits a document is [`editor`](crate::ui_iced::editor),
//!   which is a scrimmed, focus-trapped **modal** surface
//!   (`SURFACE_ID_EDITOR`) that [`guard_leave`](crate::ui_iced::editor::guard_leave)
//!   refuses to close while dirty — so while a buffer is dirty the tab strip is
//!   behind the scrim, and when the modal closes the buffer is saved or
//!   explicitly discarded. There is no reachable state in which a dot could be
//!   observed, and `UIF-TAB-003`'s dirty-close guard is satisfied by substitution
//!   through `UIF-MOD-002`: a tab cannot be reached to close while its editor is
//!   up and dirty. Flagged as spec drift per [RUST-041]; reasoning and the
//!   contingency for a future docked editor are in
//!   `doc/planning/tab-strip-dead-space.md`.
//!
//! # Framework-free `UserCommand` constructors
//!
//! This module owns four framework-free `UserCommand` constructors
//! ([`clear_agent_session_command`], [`apply_tab_close_click`],
//! [`apply_tab_close_others_click`], [`apply_tab_close_all_click`]). Every
//! one is a pure `usize`/`() → UserCommand` constructor with **no framework
//! type** in its signature or body, so the view and the reducer call exactly
//! the same code.
//!
//! There is no task-toggle constructor here: the render path does not yet
//! emit task-checkbox intents (see the `message` module's UI-048 note).
//!
//! # Token binding (INT-7c)
//!
//! Every size and gap in this module is bound to a `hoarfrost` token; the doc
//! comment on each constant records the movement from the literal it replaced.
//! Three of them were **off the type scale entirely** and had no legal source:
//! `DOCUMENT_TITLE_SIZE` and `AGENT_HEADER_SIZE` (both `18`, now
//! `type.title_sm` 16) and `EMPTY_STATE_SIZE` (`15`, now `type.body` 14).
//!
//! The **tab-strip height is now explicit**. It used to be emergent from
//! `TAB_TEXT_SIZE + TAB_PADDING_Y + STRIP_PADDING`, which landed near 25 pt;
//! §4.4.1 pins the box at `Height: 36px` and `size.control.lg`'s own token
//! doc names itself as the Tab Strip height, so [`STRIP_HEIGHT`] is now a
//! fixed `CONTROL_LG`. The band closes arithmetically the way INT-7a's navbar
//! does:
//! `STRIP_HEIGHT (36) = STRIP_PADDING (4) + HIT_TARGET_MIN (24) + 2 × TAB_PADDING_Y (2) + STRIP_PADDING (4)`,
//! with [`TAB_HEIGHT`] = `28` = `size.control.sm` as the tab's own rung.
//!
//! The band's content is a horizontal `iced::widget::scrollable`
//! ([`TAB_STRIP_SCROLL_ID`])
//! carrying one **shrink-wrapped** tab box per open document. The scrollable
//! is what keeps a crowded strip inside the band: iced's `Row` neither wraps
//! nor clips, so measured without it the strip absorbed the overflow by
//! crushing its last tab to a third of the width that same tab claims on its
//! own. Each tab box is `Length::Shrink` rather than `Fill` so the box and the
//! painted selected-tab fill are the same rectangle by construction instead
//! of by the accident of the parent `Row`'s length.
//!
//! Two §4.4.1 details sit outside the band, and an earlier note here said
//! both were blocked because they "need a layer the current widget tree
//! cannot paint". **That premise was wrong** — INT-10's Part C audit found a
//! canvas painter was never required. One of the two is now built; the other
//! is blocked by something that does exist:
//!
//! * **The 2 px top accent indicator is built** (INT-11). A `Stack` per tab
//!   with a [`shape::BORDER_FOCUS`]-high styled `container` laid over it is
//!   the whole mechanism; the bar consumes no layout, so the pinned band
//!   closure below is untouched by it. What is *still* missing is the
//!   indicator's third state: §4.4.2 asks for `border.strong` on a hovered
//!   inactive tab, which needs the strip to report hover into the reducer
//!   the way the file tree does since INT-10. The active-only bar is what
//!   ships, and an inactive tab paints nothing rather than a leftover bar.
//! * **The strip's 1 px `border.subtle` bottom rule** is not painted, and
//!   this one is a real budget conflict rather than a missing layer: iced's
//!   `Border` is a uniform perimeter, so a bottom-only hairline needs a
//!   `rule::horizontal` sibling inside the band, which pushes the band's
//!   content box past the 36 px [`STRIP_HEIGHT`] pins. Left as a stated
//!   follow-on rather than folded in here.
//!
//! [`shape::BORDER_FOCUS`]: hoarfrost::token::shape::BORDER_FOCUS
//!
//! One further §4.4.2 deviation is recorded rather than changed: the spec
//! gives the **active tab** a `surface.overlay` fill, while the app paints
//! `palette.accent` under `text_strong`. That is a behaviour decision, not
//! a token gap — the current pairing already satisfies the `UIF-TOK-006`
//! fill/foreground lock (`accent_fill` vs `text_strong` = `4.80:1`), so
//! nothing is unsafe; moving to `surface.overlay` would remove the accent
//! highlight entirely and is the user's call.
//!
//! Unit tests live in the sibling `center_tests.rs` sidecar.

#[cfg(test)]
#[path = "center_tests.rs"]
mod tests;

use std::path::{Path, PathBuf};

use iced::widget::{Column, Row, Space, Stack, button, container, scrollable, text};
use iced::{Alignment, Background, Element, Length, Padding};

use hoarfrost::token::typography::{self, TypeRung};
use hoarfrost::token::{shape, size, space};
use hoarfrost::widget::button::ButtonVariant;
use hoarfrost::widget::button_widget::Button;

use crate::bus::events::user_command::UserCommand;
use crate::config::SkillFile;
use crate::ui::app::FastMdApp;
use crate::ui::strings;
use crate::ui::theme::Palette;
use crate::ui_iced::context_menu::{
    ContextGroup, ContextRow, KEY_CLOSE_ALL_TABS, KEY_CLOSE_OTHERS, KEY_CLOSE_TAB, KEY_COPY_PATH,
    KEY_EDIT, KEY_OPEN_IN_EDITOR, KEY_SHOW_IN_EXPLORER,
};
use crate::ui_iced::message::{Message, TabMessage};

use super::left::note_skill_files;
use super::top::phosphor_font;

// Framework-free `UserCommand` constructors owned by this module. Each is a
// pure `usize`/`()`→[`UserCommand`] constructor with no framework type in
// its signature or body, so the view and the reducer call exactly the same
// code.

/// Purpose: Creates a `UserCommand` to clear all agent-related session state.
/// Inputs: None
/// Outputs: `UserCommand::ClearAgentSession`
/// Purity: Pure (returns command, no state mutation).
/// Preconditions: None.
/// Postconditions: Returns command that, when executed, hides agent results, clears history and text buffers, and cancels any running agent.
#[must_use]
pub fn clear_agent_session_command() -> UserCommand {
    UserCommand::ClearAgentSession
}

/// Purpose: Applies the side effect of clicking the tab close `×`
/// button in the center panel.
/// Inputs: i (the index of the tab to close, as it appeared in the tab
/// strip on the frame the button was clicked)
/// Outputs: the `UserCommand::CloseTab` intent for that index.
/// Purity: Pure (returns command, no state mutation).
/// Preconditions: None — `i` is bounds-checked inside the executor; an
/// out-of-range index is a silent no-op.
/// Postconditions: Returns the command that, when executed, removes the tab
/// at that index; if the closed tab was the selected file the selection
/// falls back to the new last tab (or `None` if no tabs remain).
#[must_use]
pub fn apply_tab_close_click(i: usize) -> UserCommand {
    UserCommand::CloseTab(i)
}

/// The intent for "Close Other Tabs" on the tab at index `i`.
#[must_use]
pub fn apply_tab_close_others_click(i: usize) -> UserCommand {
    UserCommand::CloseOtherTabs(i)
}

/// The intent for "Close All Tabs".
#[must_use]
pub fn apply_tab_close_all_click() -> UserCommand {
    UserCommand::CloseAllTabs
}

/// The type rung a tab label sits on — `type.ui_compact` (`UIF-TOK-004`).
///
/// §4.4.1 prices the tab's title label at `13px`, and §2.4 names
/// `type.ui_compact` as the dense-list rung that carries tab labels. It is
/// held as a whole [`TypeRung`] rather than a bare size because the tab needs
/// three things out of it: the point size, the authoritative 18 px line box,
/// and the weight the *active* label promotes to (§4.4.2 "13px SemiBold"
/// active / "13px Regular" inactive) — and `UI_COMPACT` permits all four
/// weights, so the promotion never leaves the scale.
pub const TAB_RUNG: TypeRung = typography::UI_COMPACT;

/// Logical point size of a tab label — `type.ui_compact` (`UIF-TOK-004`).
///
/// **Value unchanged at 13.** The old constant was a hand-written `13.0`
/// described as "the nearest whole point to the 12.5 px default body size";
/// that number happens to *be* the `ui_compact` rung, so the movement here is
/// the binding, not the pixels.
pub const TAB_TEXT_SIZE: f32 = TAB_RUNG.size;

/// Horizontal padding inside one tab — `space.xs` (`UIF-TOK-001`).
///
/// **Movement: 12 → 4 (−8 per side, −16 pt per tab), by owner ruling.** This is
/// a recorded **deviation from §2.1**, not a reading of it: §2.1 binds
/// `space.md` to "standard horizontal padding inside buttons, inputs, **tabs**
/// and menu rows", and the constant was `space::MD` for exactly that reason. The
/// owner ruled the tab strip down to `space.xs` to recover the horizontal space
/// the strip was spending on chrome; the ruling and its reasoning live in
/// `doc/planning/tab-strip-dead-space.md`, and the drift is flagged here per
/// [RUST-041].
///
/// `space.xs` is still a rung on the base-4 grid, so `UIF-TOK-001`'s
/// "no off-rung spacing in layout code" rule is honoured even though the role
/// binding is not. The earlier history is preserved for context: the value before
/// `space.md` was a hand-written `10`, which was off the grid entirely.
pub const TAB_PADDING_X: f32 = space::XS;

/// Vertical padding inside one tab — **derived**, not picked.
///
/// `(TAB_HEIGHT − size.hit_target.min) / 2 = (28 − 24) / 2 = 2`, which lands
/// on `space.2xs` (`UIF-TOK-001`).
///
/// **Movement: 4 → 2 (−2 per side).** The close button's
/// [`TAB_CLOSE_HIT_TARGET`] (24) is now the tallest thing inside a tab, so
/// the vertical inset is whatever the band leaves over after it rather than a
/// number chosen independently of it. That is what makes the whole strip close
/// arithmetically instead of by coincidence:
/// `STRIP_HEIGHT (36) = STRIP_PADDING (4) + HIT_TARGET_MIN (24) + 2 × TAB_PADDING_Y (2) + STRIP_PADDING (4)`.
/// The old `4` could not coexist with a spec-mandated 24 × 24 close target —
/// `24 + 2 × 4 = 32` would have overflowed the 28 pt tab.
///
/// The `type.ui_compact` line box (18) sits inside the 24 pt target with 3 pt
/// to spare per side, so the label never touches the tab's edges.
///
/// Written as the rung rather than as the division that produced it: the
/// scanner reads `/ 2.0` as a bare literal on a visually-named constant,
/// and it is right to — a divisor is still a number nobody bound. The
/// derivation itself is not lost, it is asserted:
/// `the_tab_strip_band_closes_on_tokens` pins
/// `space::XXS == (TAB_HEIGHT - HIT_TARGET_MIN) / 2.0`, so the equality
/// breaks the build rather than the layout if either side moves.
pub const TAB_PADDING_Y: f32 = space::XXS;

/// Gap between adjacent tabs in the strip — `space.2xs` (`UIF-TOK-001`),
/// the dense inline micro-gap rung. Value unchanged at `2`.
pub const TAB_SPACING: f32 = space::XXS;

/// Gap between a tab's label and its `×` close button — `space.xs`
/// (`UIF-TOK-001`).
///
/// Value unchanged at `4`, which is also §4.4.1's stated separation between
/// the close button and the tab activation zone (`>= 2px`,
/// `UIF-PRIN-008`) with room to spare.
pub const TAB_CLOSE_SPACING: f32 = space::XS;

// The unsaved-changes marker (§4.4.1 slot 2) is removed. Its slot, dot, radius
// and colour helpers used to live here.
//
// `TabEntry::unsaved` was written `false` in exactly one place and read in one
// place, and no code path in the crate could set it — the only thing that edits a
// document is `ui_iced/editor.rs`, which is a scrimmed, focus-trapped **modal**
// surface (`SURFACE_ID_EDITOR`, `app.rs:1552`) that `guard_leave` (`UIF-MOD-002`)
// refuses to close while dirty. A tab therefore has no reachable state in which a
// dirty dot could be observed: while the editor is dirty the strip is behind the
// scrim, and when the editor closes the buffer is either saved or explicitly
// discarded. §4.4.1 slot 2 is consequently not implementable under this editing
// model, and `UIF-TAB-003`'s dirty-close guard is satisfied by substitution
// through `UIF-MOD-002` — flagged per [RUST-041].
//
// Reasoning and the contingency for a future *docked* editor:
// `doc/planning/tab-strip-dead-space.md`.

/// Height of a tab's top accent indicator — `shape.border.focus`
/// (`UIF-TOK-003`), the weight §2's border table names against the "active
/// tab accent bar" and §4.4.1 slot 1 prices at `2px`.
///
/// **This height is not subtracted from [`TAB_HEIGHT`].** The bar is laid
/// *over* the tab in a [`Stack`], so the band closure
/// `STRIP_HEIGHT (36) = STRIP_PADDING (4) + TAB_HEIGHT (28) + STRIP_PADDING (4)`
/// is arithmetically unchanged whether or not a bar is painted, and the
/// `36 > 30` content-floor pin cannot be disturbed by it.
pub const TAB_INDICATOR_HEIGHT: f32 = shape::BORDER_FOCUS;

/// Stroke width of a tab's perimeter border — `shape.border.hairline`
/// (`UIF-TOK-003`), i.e. §4.4.2's `1px` perimeter taken literally.
///
/// **Zero-cost to the pinned band closure, for the same reason the top bar is.**
/// iced 0.14's `container` layout function never receives the border at all:
/// `Container::layout` hands the layout helper only `width`, `height`,
/// `max_width`, `max_height`, `padding` and the two alignments
/// (`iced_widget-0.14.2/src/container.rs:259-276`); the border is read
/// solely in the paint path (`container.rs:443-449`). A tab therefore does not
/// grow, shrink or inset its content by this width in any posture, so
/// `STRIP_HEIGHT (36) = STRIP_PADDING (4) + TAB_HEIGHT (28) + STRIP_PADDING (4)`
/// and the `36 > 30` content floor are untouched by wiring the perimeter
/// column.
pub const TAB_BORDER_WIDTH: f32 = shape::BORDER_HAIRLINE;

/// Point size of a tab's `×` close glyph — `size.icon.sm` (`UIF-TOK-002`).
///
/// **Movement: 13 → 14 (+1).** §4.4.1 slot 4 names `size.icon.sm` for the
/// close glyph; the old value borrowed the label's rung instead of the icon
/// scale, which is how a glyph ends up at a size no icon token produces.
pub const TAB_CLOSE_GLYPH_SIZE: f32 = size::ICON_SM;

/// Square hit area of a tab's `×` close button — `size.hit_target.min`
/// (`UIF-TOK-002`).
///
/// **Movement: glyph-sized (~13 × ~18) → 24 × 24.** §4.4.1 requires a
/// "`24px × 24px` disjoint hit target"; before this the button was only as
/// large as its glyph plus iced's default padding, so the close affordance
/// sat *below* the 24 px floor every interactive control in the interface is
/// measured against. This is a real ergonomics fix, not a rename.
pub const TAB_CLOSE_HIT_TARGET: f32 = size::HIT_TARGET_MIN;

/// Padding inside the tab-strip band — `space.xs` (`UIF-TOK-001`). Value
/// unchanged at `4`.
pub const STRIP_PADDING: f32 = space::XS;

/// Height of the **tab-strip band** — `size.control.lg` (`UIF-TOK-002`).
///
/// **Movement: emergent (~25) → pinned 36.** The strip had no height
/// constant at all: its height was whatever `TAB_TEXT_SIZE` plus
/// [`TAB_PADDING_Y`] plus [`STRIP_PADDING`] happened to add up to, which put
/// it near 25 pt — 11 pt short of what the spec pins. §4.4.1 draws the box
/// as `Tab Strip (Height: 36px, …)`, and [`size::CONTROL_LG`]'s own doc
/// names itself as "the Top Navbar height and **the Tab Strip height**", so
/// 36 is not a guess — it is the same rung INT-7a put the navbar on.
///
/// The band must be explicit rather than emergent for the same reason the
/// navbar did: an emergent height means a type retune silently resizes a
/// band that the layout contract treats as fixed.
pub const STRIP_HEIGHT: f32 = size::CONTROL_LG;

/// Height of one tab inside the band, **derived** from the band rather than
/// picked: `STRIP_HEIGHT − 2 × STRIP_PADDING`.
///
/// `36 − 2 × 4 = 28`, which lands exactly on [`size::CONTROL_SM`] — the
/// same closure INT-7a established for the navbar
/// (`36 = XS + CONTROL_SM + XS`). The tab is therefore a compact control
/// centred in a large structural band, and retuning the band or the inset
/// moves the tab with it instead of leaving a height that no longer fits.
///
/// The subtraction is written out twice rather than as `2.0 × STRIP_PADDING`
/// so no literal sits on a visually-named constant; the equality with
/// `CONTROL_SM` and the band closure are both asserted in
/// `the_tab_strip_band_closes_on_tokens`.
pub const TAB_HEIGHT: f32 = STRIP_HEIGHT - STRIP_PADDING - STRIP_PADDING;

/// The rung the viewer's prose sits on — `type.body` (`UIF-TOK-004`).
///
/// Held as a rung for the same reason [`TAB_RUNG`] is: the viewer needs the
/// size *and* the authoritative 20 px line box, and binding both to one
/// constant is what stops a size retune leaving a line height behind that no
/// longer matches it.
pub const BODY_RUNG: TypeRung = typography::BODY;

/// The rung the center panel's header lines sit on — `type.title_sm`
/// (`UIF-TOK-004`).
///
/// Both the document-title placeholder and the agent-session band's header
/// are panel headers, so they share one rung and one line box.
pub const HEADER_RUNG: TypeRung = typography::TITLE_SM;

/// Logical point size of the document title in the viewer area —
/// `type.title_sm` (`UIF-TOK-004`).
///
/// **Movement: 18 → 16 (−2).** `18` is not a rung: `TITLE_SM` is 16 and
/// `TITLE_MD` is 20, so nothing on the scale produced that number, and the
/// whole point of the rung system is that off-scale sizes are illegal.
/// `TITLE_MD` was rejected because §2.4 reserves it for *document H2
/// headings* and the About header — this is the pane's title line, not a
/// heading inside the document. `TITLE_SM` is the rung §2.4 gives to panel
/// section titles and "side sheet / empty-state headlines", which is what
/// this placeholder is.
pub const DOCUMENT_TITLE_SIZE: f32 = HEADER_RUNG.size;

/// Logical point size of the agent-session header — `type.title_sm`
/// (`UIF-TOK-004`).
///
/// **Movement: 18 → 16 (−2).** The same off-scale `18` as the document
/// title, reached from a different direction and fixed the same way. §2.4
/// assigns `type.title_sm` to "movable overlay headers" and panel section
/// titles; the agent band is a panel header, so it takes that rung. `TITLE_MD`
/// would put a *document-H2* size on a chrome band whose controls are 28 pt
/// tall.
pub const AGENT_HEADER_SIZE: f32 = HEADER_RUNG.size;

/// Logical point size of the agent status and thinking text — `type.body`
/// (`UIF-TOK-004`). Value unchanged at `14`.
pub const AGENT_BODY_SIZE: f32 = BODY_RUNG.size;

/// Logical point size of the empty-state prompt — `type.body`
/// (`UIF-TOK-004`).
///
/// **Movement: 15 → 14 (−1).** `15` is not a rung. `CAPTION` (12) was
/// rejected because the empty-state prompt is the pane's only content and its
/// primary reading voice, not a caption subordinate to something else;
/// `TITLE_SM` (16) was rejected because §2.4's empty-state *headline* is a
/// heading, and this is a one-line prompt string. `BODY` is the document-prose
/// rung the viewer actually carries once the real render lands, so the
/// placeholder now matches the content it stands in for.
pub const EMPTY_STATE_SIZE: f32 = BODY_RUNG.size;

/// Padding inside the document-viewer area — `space.sm` (`UIF-TOK-001`),
/// the standard gap rung used as the pane's inner inset. Value unchanged
/// at `8`.
pub const VIEWER_PADDING: f32 = space::SM;

/// Gap between blocks inside the viewer area — `space.sm` (`UIF-TOK-001`).
///
/// **Movement: 6 → 8 (+2).** `6` is not on the base-4 grid and sits exactly
/// midway between `XS` (4) and `SM` (8). The tie is broken on *role*, the
/// same way INT-7a broke it for `BAND_PADDING`: `SM` is the standard
/// **sibling gap**, `XS` is the compact **inner padding**, and this is a gap
/// between sibling blocks. It also puts the viewer's block rhythm on the
/// same rung as the render path's own
/// [`BLOCK_SPACING`](crate::ui_iced::render::BLOCK_SPACING), whose module
/// doc says it mirrors this constant — so the two stop drifting apart.
pub const VIEWER_SPACING: f32 = space::SM;

/// Stable iced widget id of the document-viewer `scrollable`.
///
/// The reducer needs a stable id to aim a scroll operation at the viewer
/// (e.g. a jump to a heading), and a widget without an explicit id is
/// re-keyed on every rebuild and loses its scroll position. Named once so
/// the view and the reducer cannot drift (§5 id-stability rule, [RUST-021]).
pub const DOCUMENT_VIEWER_SCROLLABLE_ID: &str = "document_viewer_scroll";

/// Stable focus id of the agent-session close button in the tab strip.
///
/// Named once in the module that owns the widget so the view and anything
/// addressing the button later cannot drift (§5 id-stability rule,
/// [RUST-021]).
pub const AGENT_SESSION_CLOSE_BUTTON_ID: &str = "agent_session_close";

/// Stable iced widget id of the tab-strip **band** container.
///
/// The band is the box whose laid-out height every geometry test measures
/// against [`STRIP_HEIGHT`], and whose laid-out right edge is the line the
/// strip must never paint past (defect #16). Named once so the view and
/// anything addressing it later cannot drift (§5 id-stability rule,
/// [RUST-021]).
pub const TAB_STRIP_BAND_ID: &str = "center.tab_strip.band";

/// Stable iced widget id of the tab-strip's horizontal `scrollable`
/// (defect #16).
///
/// Overflowing tabs must scroll instead of overpainting the neighbouring
/// pane, and a `scrollable` only keeps its scroll position across the
/// per-frame rebuild when it carries an explicit id — never
/// `Id::unique()` (§5 id-stability rule).
pub const TAB_STRIP_SCROLL_ID: &str = "center.tab_strip.scroll";

/// Stable id of the tab at `index`'s **outer layout box** — the rectangle
/// the strip's `Row` allocates to that tab.
///
/// iced 0.14's `Stack` exposes no `id` builder, so the tab's outer box is a
/// shrink-wrapped `container` around it. The container reports the same
/// rectangle the `Stack` occupies, which is what a headless layout test
/// needs in order to *measure* "how wide is this tab" rather than assert
/// the intent (defect #15).
pub fn tab_slot_id(index: usize) -> iced::widget::Id {
    iced::widget::Id::from(format!("center.tab.slot.{index}"))
}

/// Stable id of the tab at `index`'s **painted surface** — the container
/// the selected-tab background fill is actually painted on.
///
/// Defect #15 is precisely the gap between this rectangle and
/// [`tab_slot_id`]'s; both are addressable so the gap is measurable.
pub fn tab_surface_id(index: usize) -> iced::widget::Id {
    iced::widget::Id::from(format!("center.tab.surface.{index}"))
}

/// Stable id of the tab at `index`'s top accent indicator.
///
/// The indicator rides in the tab's `Stack` at `Length::Fill`, so its
/// laid-out width is the measurement of "does the accent bar still cover
/// the whole tab" once the tab stops stretching (defect #15).
pub fn tab_indicator_id(index: usize) -> iced::widget::Id {
    iced::widget::Id::from(format!("center.tab.indicator.{index}"))
}

/// The title a tab renders: the file name, or the empty string when the path
/// has none (e.g. a bare root).
///
/// This is the same mapping `Tabs::tab_strip` caches; the view cannot call
/// `tab_strip()` because that cache needs `&mut Tabs` while a view model is
/// built from `&FastMdApp`.
///
/// Purity: pure function of `path`.
#[must_use]
pub fn tab_title(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// Index of the tab holding `selected`, `None` when the selected file is not
/// open (or nothing is selected).
///
/// The `selected_file() == Some(tab_path)` test is stated once here so the
/// strip and the view model cannot disagree.
///
/// Purity: pure.
#[must_use]
pub fn active_tab_index(paths: &[PathBuf], selected: Option<&Path>) -> Option<usize> {
    match selected {
        Some(path) => paths.iter().position(|tab| tab == path),
        None => None,
    }
}

/// Builds the tab-strip entries for `paths`, marking the one holding
/// `selected` active.
///
/// Purity: pure.
#[must_use]
pub fn tab_entries(paths: &[PathBuf], selected: Option<&Path>) -> Vec<TabEntry> {
    let active = active_tab_index(paths, selected);
    paths
        .iter()
        .enumerate()
        .map(|(index, path)| TabEntry {
            path: path.clone(),
            title: tab_title(path),
            active: active == Some(index),
        })
        .collect()
}

/// Every tab index except `keep` — the set `Close Others` (UI-028) closes.
///
/// The command itself carries only the kept index and the reducer does the
/// work; this helper exists so the rule is stated once and a future
/// confirmation dialog (UI-029) can enumerate what would close without
/// re-deriving it. A `keep` outside `0..tabs.len()` closes everything,
/// matching the reducer's "nothing was kept" reading.
///
/// Purity: pure.
#[must_use]
pub fn other_tab_indices(tabs: &[TabEntry], keep: usize) -> Vec<usize> {
    (0..tabs.len()).filter(|index| *index != keep).collect()
}

/// The intent a left-click on a tab produces.
///
/// Selecting a tab publishes `UserCommand::SelectFile { path, multi: false }`
/// — never a multi-select, because the tab strip has no modifier semantics.
///
/// Purity: pure.
#[must_use]
pub fn tab_select_command(path: &Path) -> UserCommand {
    UserCommand::SelectFile {
        path: path.to_path_buf(),
        multi: false,
    }
}

/// The message a middle-click or `×` click on tab `index` produces
/// (UI-049, UI-027).
///
/// Purity: pure.
#[must_use]
pub fn tab_close_message(index: usize) -> Message {
    Message::Command(apply_tab_close_click(index))
}

/// The message that opens tab `index`'s context menu (UI-026).
///
/// Purity: pure.
#[must_use]
pub fn tab_context_menu_open_message(index: usize) -> Message {
    Message::Tab(TabMessage::ContextMenuOpened { index })
}

/// The message that dismisses the tab context menu.
///
/// Purity: pure.
#[must_use]
pub fn tab_context_menu_close_message() -> Message {
    Message::Tab(TabMessage::ContextMenuClosed)
}

/// The message the agent-session band's Close button produces: the
/// `ClearAgentSession` intent that hides the session and clears its state.
///
/// Purity: pure.
#[must_use]
pub fn agent_session_close_message() -> Message {
    Message::Command(clear_agent_session_command())
}

/// UI-048 gate: whether the "Thinking Process" block has anything to show.
///
/// The `🤔…🤔` delimiter itself is already resolved upstream by
/// `AgentTranscript::apply_event` / `split_thinking_and_content`
/// (`src/app/ui/agent/transcript.rs`) and the agent's response formatter
/// (AGENT-022); the panel only decides whether the extracted buffer is
/// non-empty.
///
/// Purity: pure function of `thinking`.
#[must_use]
pub fn thinking_section_visible(thinking: &str) -> bool {
    !thinking.is_empty()
}

/// Colour of a tab's `2px` top accent indicator, or `None` when it paints
/// nothing.
///
/// §4.4.1 slot 1 gives this bar three states: `accent.border` when active,
/// `transparent` when inactive at rest, and `border.strong` on hover.
/// **All three are wired as of INT-12.** The hovered state needed the strip
/// to report the pointer into the reducer the way the file tree has since
/// INT-10, and that crossing is now in place (`TabMessage::Entered` /
/// `Left` → `OverlayState::hovered_tab` → `CenterPanelViewModel::hovered_tab`).
///
/// **Active outranks hover.** A hovered *active* tab keeps
/// `accent.border`, because §4.4.1's hover rung is stated for the tab that
/// is not already marked active — a bar that changed from `accent.border`
/// to `border.strong` under the cursor would read as the tab losing
/// activation.
///
/// An inactive tab at rest resolves to `None` rather than to a transparent
/// fill, so the render path emits no bar at all and a leftover or partial
/// indicator on an inactive tab is not reachable from here.
///
/// `accent.border` and `border.strong` are read from the framework palette;
/// no numeric colour is introduced in this module (§4a). Against the
/// hovered tab's `surface.hover` fill, `border.strong` measures
/// `3.677181:1`, which is the `3.68:1` §4.4.2 prints for this bar and
/// clears the `3.0:1` chrome floor.
///
/// Purity: pure function of `(active, hovered)`.
#[must_use]
pub fn tab_top_indicator(active: bool, hovered: bool) -> Option<iced::Color> {
    let pro = Palette::pro();
    if active {
        Some(pro.accent_border)
    } else if hovered {
        Some(pro.border_strong)
    } else {
        None
    }
}

/// Whether `tab` is the tab the pointer is currently over.
///
/// Compared by **path**, mirroring `left::row_is_hovered`. The strip
/// index is deliberately not consulted: `Close Others`, `Close All`, a
/// middle-click and a watcher-driven file deletion all change what index
/// a given file sits at, and a highlight keyed by index would slide onto
/// whatever tab moved into the remembered slot. Keyed by path, the
/// highlight can only ever sit on the tab the pointer actually entered.
///
/// Purity: pure.
#[must_use]
pub fn tab_is_hovered(tab: &TabEntry, hovered: Option<&std::path::Path>) -> bool {
    hovered == Some(tab.path.as_path())
}

/// Label colour for a tab, resolved from §4.4.2's label column.
///
/// | State | Spec label | Resolves to | Measured on the fill this paint puts under it |
/// | --- | --- | --- | --- |
/// | Active | `text.strong` | `palette.text_strong` | `4.801987:1` on `accent.fill` |
/// | Inactive (Hovered) | `text.primary` | `palette.text` | `10.738035:1` on `surface.hover` |
/// | Inactive (Rest) | `text.muted` | `palette.text_muted` | `7.608225:1` on the unpainted strip |
///
/// Only palette tokens are used; no numeric colour is added (§4a).
///
/// **The active row is the weakest of the three, and the reason is the fill,
/// not the label.** §4.4.2 prints `17.72:1` for the active label, and that
/// figure is arithmetically correct — for `text.strong` on `surface.overlay`,
/// the fill the spec names. The app does not paint `surface.overlay` on the
/// active tab (see [`tab_fill`]), so the shipped active label measures
/// `4.801987:1`, not `17.72:1`. It still clears the `4.5:1` text floor, but
/// by `0.3` rather than by `13.2`.
///
/// Purity: pure function of `(active, hovered)` for a given palette.
#[must_use]
pub fn tab_text_color(palette: &Palette, active: bool, hovered: bool) -> iced::Color {
    if active {
        palette.text_strong
    } else if hovered {
        palette.text
    } else {
        palette.text_muted
    }
}

/// Label font for a tab: the active tab takes [`TAB_RUNG`]'s **emphasis**
/// weight (SemiBold), the inactive tabs its **resting** weight (Regular),
/// both on `font.family.sans`.
///
/// This is §4.4.2's tab typography read literally — "13px SemiBold,
/// `text.strong`" for Active against "13px Regular, `text.muted`" for
/// Inactive. Before this the active tab changed *colour* only and the weight
/// never moved, so the active tab was quieter than the spec asks. Both
/// weights sit inside `UI_COMPACT`'s `allowed_weights`, so the emphasis is a
/// rung-legal move rather than a weight invented for the occasion.
///
/// Purity: pure function of `active`.
#[must_use]
pub fn tab_label_font(active: bool) -> iced::Font {
    let weight = if active {
        TAB_RUNG.emphasis_weight
    } else {
        TAB_RUNG.weight
    };
    iced::Font {
        weight: weight.iced(),
        ..iced::Font::with_name("Fira Sans")
    }
}

/// The fill a tab paints behind its contents, resolved from §4.4.2's fill
/// column.
///
/// | State | Spec fill | What this returns |
/// | --- | --- | --- |
/// | Active | `surface.overlay` | `palette.accent` — **recorded deviation** |
/// | Inactive (Hovered) | `surface.hover` | `pro.surface_hover` (`32,32,36`) |
/// | Inactive (Rest) | `surface.raised` | `None` — **left unwired, see below** |
///
/// Extracted so the dot-colour rule below can ask *what is behind the dot*
/// rather than assume - which is exactly what
/// [`hoarfrost::token::contrast::fill_lock`] says a
/// foreground audit has to do, because the style itself does not know.
///
/// # The active rung is a deviation, not an oversight
///
/// §4.4.2 asks for `surface.overlay` (`24,24,27`); the app paints
/// `palette.accent`, which *is* `color.accent.fill` — one of the four solid
/// fills `UIF-TOK-006` rule 4 role-locks. That single substitution is what
/// makes three of this matrix's printed figures stop describing the shipped
/// UI, because every one of them was computed against `surface.overlay`:
///
/// * active label `text.strong` — spec `17.72:1`, actual **`4.801987:1`**
/// * active top bar `accent.border` — spec `5.94:1`, actual **`1.609837:1`**
/// * active perimeter `border.strong` — spec `4.01:1`, actual **`1.087631:1`**
///
/// The last two are effectively invisible against `accent.fill`. This is
/// reported rather than changed: the accent fill is a recorded INT-11 decision
/// with a role lock that used to hang off the unsaved dot (since removed), so
/// moving it is a spec decision that cascades, not a local edit.
///
/// # The rest rung is deliberately not painted
///
/// §4.4.2 names `surface.raised` for a resting inactive tab. Wiring it would
/// recolour **every** inactive tab in the strip, which is a wider visual change
/// than this increment was scoped to (its headline was the hovered column), so
/// the cell is left as the `None` it has always returned and reported as the
/// one matrix cell still unwired. It is a one-line change —
/// `Some(palette.surface_variant)` — if that repaint is wanted.
///
/// Note the rest fill is also the cell the spec's own track assumption is
/// missing behind: §4.4.2's `:1298` puts every tab inside a recessed
/// `surface.sunken` track, and no sunken track is painted anywhere in this
/// strip, so a `raised` fill would read against `surface.base`
/// (`1.063306:1`) rather than against a recess.
///
/// Purity: pure function of `(active, hovered)` for a given palette.
#[must_use]
pub fn tab_fill(palette: &Palette, active: bool, hovered: bool) -> Option<iced::Color> {
    if active {
        Some(palette.accent)
    } else if hovered {
        Some(Palette::pro().surface_hover)
    } else {
        None
    }
}

/// The perimeter stroke a tab paints, resolved from §4.4.2's perimeter
/// column: `border.strong` on the active tab and on a hovered inactive tab,
/// `border.subtle` on a resting inactive tab, always at
/// [`TAB_BORDER_WIDTH`] (`1px`).
///
/// Before this increment **no perimeter at all** was drawn on any tab — the
/// tab's `container::Style` left `border` at its default zero width — so the
/// whole column was unwired rather than partly wrong.
///
/// # Measured, against the surface each one actually sits on
///
/// * hovered inactive: `border.strong` / `surface.hover` = **`3.677181:1`**,
///   which is the `3.68:1` §4.4.2 prints and clears the `3.0:1` chrome floor.
/// * resting inactive: `border.subtle` / the unpainted strip
///   (`surface.base`) = **`1.349455:1`**, and `1.269112:1` against the
///   `surface.raised` the spec names. §4.4.2 prints *no* figure for this cell,
///   so it makes no legibility claim — but if the `3.0:1` chrome floor were
///   read as applying to separators, this cell fails it **as specified**, not
///   as implemented. Reported as a spec gap rather than quietly upgraded to a
///   token that would pass.
/// * active: `border.strong` / `accent.fill` = **`1.087631:1`** — see
///   [`tab_fill`]. The colour is spec-correct; the fill underneath it is not
///   what the spec assumed, so this stroke is painted and is invisible.
///
/// # The active tab's bottom mask does not exist, and cannot
///
/// §4.4.2 asks that the active tab's bottom border "mask the `1px` track
/// border and open into the viewer". There is no `1px` track border to mask:
/// the strip paints no `surface.sunken` track and no track border at all
/// (`tab_strip` applies a plain padded container). The masking behaviour is
/// therefore **not implemented and is not reachable** until the track itself
/// is built. A border drawn here today would cover nothing.
///
/// Purity: pure function of `(active, hovered)`.
#[must_use]
pub fn tab_perimeter(active: bool, hovered: bool) -> iced::Color {
    let pro = Palette::pro();
    if active || hovered {
        pro.border_strong
    } else {
        pro.border_subtle
    }
}

/// Colour of a tab's `×` close glyph, resolved from §4.4.2's close column.
///
/// Both the resting glyph and its own hover sub-state are covered, and the
/// glyph's hover is the **close button's** hover, not the tab's — the two are
/// independent, which is what §4.4.1 slot 4's "disjoint hit target" and
/// "independent hover highlight" require.
///
/// | Tab state | Glyph at rest | Glyph on close-hover |
/// | --- | --- | --- |
/// | Active | `text.muted` | `status.danger.text` |
/// | Inactive (Hovered) | `text.primary` | `status.danger.text` |
/// | Inactive (Rest) | `text.disabled` | `text.strong` |
///
/// Before this increment the glyph carried **no colour at all** — it inherited
/// iced's default button text colour — so the whole column was unwired.
///
/// # Measured
///
/// * `status.danger.text` / `surface.hover` = **`5.363298:1`**, the `5.36:1`
///   §4.4.1 slot 4 prints. ✓
/// * `text.disabled` / the unpainted strip = **`5.916514:1`**, and
///   `5.564263:1` on the spec's `surface.raised`. Both sit inside
///   [`hoarfrost::token::contrast::DISABLED_TEXT_MIN_CONTRAST`] ..
///   [`hoarfrost::token::contrast::DISABLED_TEXT_MAX_CONTRAST`] (`4.0 .. 6.05`), the band a
///   disabled-looking control is required to fall in — so the resting glyph is
///   legible without reading as enabled.
/// * `text.strong` / `surface.hover` = **`16.235083:1`** for the
///   inactive-rest hover sub-state.
/// * `text.muted` on the **active** tab = **`1.836338:1`** against
///   `accent.fill`. §4.4.2 asks for `text.muted` there and that is what is
///   painted, but against the fill the app actually uses it is close to
///   invisible — the same `surface.overlay` substitution that flattens the
///   active label and the active perimeter.
///
/// Purity: pure function of `(active, tab_hovered, close_hovered)`.
#[must_use]
pub fn tab_close_glyph_color(
    palette: &Palette,
    active: bool,
    tab_hovered: bool,
    close_hovered: bool,
) -> iced::Color {
    if close_hovered {
        if active || tab_hovered {
            palette.error
        } else {
            palette.text_strong
        }
    } else if active {
        palette.text_muted
    } else if tab_hovered {
        palette.text
    } else {
        palette.text_faint
    }
}

/// The hover highlight behind a tab's `×` close button — `surface.hover`, per
/// §4.4.1 slot 4's "independent hover highlight".
///
/// Read from the framework palette because the app palette has no
/// `surface.hover` role; no numeric colour is introduced (§4a).
#[must_use]
pub fn tab_close_hover_fill() -> iced::Color {
    Palette::pro().surface_hover
}

/// Full iced style for a tab's `×` close button, resolved from the tab's own
/// visual state plus iced's button status.
///
/// **Why the status is folded in here rather than tracked as app state.** The
/// close button's hover is a third hover axis (tab hover, close hover, press)
/// and none of it needs to reach the reducer: iced already computes it and
/// hands it to the style closure as `button::Status`, the same way
/// [`top::tag_trigger_style`](super::top) routes `pick_list::Status`. Keeping
/// it in the style means no new `OverlayState` field, no new message, and no
/// chance of a hover report racing a tab reorder.
///
/// `Pressed` is treated as hovered so the glyph does not flicker back to its
/// resting colour during the click that is closing the tab.
///
/// Purity: pure function of `(active, tab_hovered, status)`.
#[must_use]
pub fn tab_close_style(active: bool, tab_hovered: bool, status: button::Status) -> button::Style {
    let close_hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
    button::Style {
        background: close_hovered.then_some(Background::Color(tab_close_hover_fill())),
        text_color: tab_close_glyph_color(
            &Palette::fastmd_dark(),
            active,
            tab_hovered,
            close_hovered,
        ),
        border: iced::Border::default(),
        shadow: iced::Shadow::default(),
        snap: false,
    }
}

/// Context-menu rows for the tab at `index`, in menu order (UI-026..UI-033),
/// followed by the VFS-121 note skills.
///
/// `note_skills` is passed in rather than looked up here so the builder stays
/// pure: `list_note_skills()` creates its directories as a side effect,
/// which a per-frame view must not do.
///
/// Purity: pure.
#[must_use]
pub fn tab_context_menu_items(
    index: usize,
    path: &Path,
    note_skills: &[SkillFile],
) -> Vec<ContextRow> {
    let path = path.to_path_buf();
    let mut items = vec![
        ContextRow::new(
            KEY_CLOSE_TAB,
            strings::CLOSE_TAB_MENU,
            ContextGroup::Primary,
            Message::Command(apply_tab_close_click(index)),
        ),
        ContextRow::new(
            KEY_CLOSE_OTHERS,
            strings::CLOSE_OTHERS_MENU,
            ContextGroup::Primary,
            Message::Command(apply_tab_close_others_click(index)),
        ),
        ContextRow::new(
            KEY_CLOSE_ALL_TABS,
            strings::CLOSE_ALL_TABS_MENU,
            ContextGroup::Primary,
            Message::Command(apply_tab_close_all_click()),
        ),
        ContextRow::new(
            KEY_EDIT,
            strings::EDIT_BUTTON,
            ContextGroup::Automation,
            Message::Command(UserCommand::OpenInEditor(path.clone())),
        ),
        ContextRow::new(
            KEY_OPEN_IN_EDITOR,
            strings::OPEN_IN_EDITOR_ACTION,
            ContextGroup::Automation,
            Message::Command(UserCommand::OpenInEditor(path.clone())),
        ),
    ];

    items.extend(
        note_skills
            .iter()
            .map(|skill| ContextRow::skill(skill, None, &path)),
    );

    items.extend([
        ContextRow::new(
            KEY_COPY_PATH,
            strings::COPY_PATH_ACTION,
            ContextGroup::Management,
            Message::Command(UserCommand::CopyPath(path.clone())),
        ),
        ContextRow::new(
            KEY_SHOW_IN_EXPLORER,
            strings::SHOW_IN_EXPLORER_ACTION,
            ContextGroup::Management,
            Message::Command(UserCommand::ShowInExplorer(path.clone())),
        ),
    ]);

    items
}

/// One open document in the tab strip.
///
/// There is no unsaved-changes field: §4.4.1 slot 2 is not implementable under
/// this app's modal editing model, so the field would be permanently `false`
/// state nothing could observe. See the note at the module header and
/// `doc/planning/tab-strip-dead-space.md`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabEntry {
    /// The document's full path — what every tab command carries.
    pub path: PathBuf,
    /// The rendered label (the file name).
    pub title: String,
    /// Whether this is the active document.
    pub active: bool,
}

impl TabEntry {
    /// A tab for `path`, titled from its file name.
    #[must_use]
    pub fn new(path: PathBuf, active: bool) -> Self {
        Self {
            title: tab_title(&path),
            path,
            active,
        }
    }
}

/// Everything the center panel needs to render, snapshotted off `FastMdApp`.
///
/// Same contract as [`super::top::TopBarViewModel`] and
/// [`super::left::LeftPanelViewModel`]: the widget tree reads only this
/// struct, which keeps the view unit-testable without a live iced runtime.
#[derive(Clone, Debug, PartialEq)]
pub struct CenterPanelViewModel {
    /// Open tabs, in strip order (UI-049).
    pub tabs: Vec<TabEntry>,
    /// Index of the active tab, `None` when no open document is selected.
    /// Kept alongside the per-tab `active` flag because the reducer wants
    /// the index; both come from the same [`active_tab_index`] call.
    pub active_tab: Option<usize>,
    /// Whether the agent session owns the center pane
    /// (`agent_panel_state.show_results`).
    pub show_agent_session: bool,
    /// The agent's status line text (`agent().state().status`).
    pub agent_status: String,
    /// Whether the agent is currently running. iced 0.14 has no spinner
    /// widget, so the running state is carried through the status text
    /// (see [`super::top`]).
    pub agent_running: bool,
    /// The extracted agent thinking buffer (UI-048), gated by
    /// [`thinking_section_visible`].
    pub thinking: String,
    /// Whether the active document is PDF-backed — the viewer paints the
    /// `pdf_surface` token behind it.
    pub active_pdf_backed: bool,
    /// Note skills offered on the tab context menu (VFS-121), which is
    /// where "Format Markdown" (UI-033) comes from.
    pub note_skills: Vec<SkillFile>,
    /// The tab the pointer is currently over, by **path** (INT-12).
    ///
    /// Drives §4.4.1 slot 1's third state: a hovered **inactive** tab
    /// takes `color.border.strong` on its top indicator. Reported `None`
    /// by [`from_app`](Self::from_app), which has no pointer input, and
    /// overwritten by `ViewModels::from_app` from
    /// [`OverlayState::hovered_tab`](crate::ui_iced::app::OverlayState::hovered_tab).
    ///
    /// Keyed by path rather than by strip index so a close or a
    /// watcher-driven re-order cannot move the highlight onto a tab the
    /// pointer never entered.
    pub hovered_tab: Option<PathBuf>,
}

impl CenterPanelViewModel {
    /// Snapshots the center panel's inputs off `app`.
    ///
    /// Reads the note-skill folder itself; prefer
    /// [`Self::from_app_with_note_skills`] when the caller already has the
    /// listing.
    pub fn from_app(app: &FastMdApp) -> Self {
        let note_skills = note_skill_files(app);
        Self::from_app_with_note_skills(app, note_skills)
    }

    /// [`Self::from_app`] with the note-skill listing supplied by the caller.
    ///
    /// The same folder backs the left panel's file menu and the tab strip's
    /// context menu, so [`ViewModels::from_app`](crate::ui_iced::app::ViewModels)
    /// reads it once per rebuild and hands the result to both instead of each
    /// panel hitting the directory itself.
    pub fn from_app_with_note_skills(app: &FastMdApp, note_skills: Vec<SkillFile>) -> Self {
        let selected = app.selection().selected_file().cloned();
        let paths = &app.tabs().tabs;
        let active_tab = active_tab_index(paths, selected.as_deref());
        let tabs = tab_entries(paths, selected.as_deref());

        Self {
            tabs,
            active_tab,
            show_agent_session: app.orchestrator.agent_panel_state.show_results,
            agent_status: app.agent().state().status.clone(),
            agent_running: app.agent().state().running,
            thinking: app.orchestrator.agent_transcript.thinking.clone(),
            active_pdf_backed: selected
                .as_deref()
                .is_some_and(|path| app.pdf_backing_tracker().is_pdf_backed(path)),
            note_skills,
            hovered_tab: None,
        }
    }

    /// The active tab, `None` when nothing is selected or no tab matches.
    #[must_use]
    pub fn active_tab_entry(&self) -> Option<&TabEntry> {
        self.active_tab.and_then(|index| self.tabs.get(index))
    }

    /// Whether the tab strip has any tab open.
    #[must_use]
    pub fn has_tabs(&self) -> bool {
        !self.tabs.is_empty()
    }

    /// The context-menu rows for the tab at `index`; empty for an index that
    /// is not open, so a stale menu request renders nothing.
    #[must_use]
    pub fn menu_items_for(&self, index: usize) -> Vec<ContextRow> {
        match self.tabs.get(index) {
            Some(tab) => tab_context_menu_items(index, &tab.path, &self.note_skills),
            None => Vec::new(),
        }
    }

    /// Whether the UI-048 thinking block renders for this snapshot.
    #[must_use]
    pub fn show_thinking(&self) -> bool {
        thinking_section_visible(&self.thinking)
    }
}

/// Renders the center panel from a view-model snapshot.
///
/// Emitted messages:
/// * tab left-click → [`Message::Command`] `SelectFile { multi: false }`
/// * tab middle-click / `×` → [`Message::Command`] `CloseTab(index)`
/// * tab right-click → [`Message::Tab`] `ContextMenuOpened { index }`
/// * pointer motion → [`Message::Layout`] `CursorMoved` (the menu anchor)
/// * agent-session Close → [`Message::Command`] `ClearAgentSession`
///
/// The tab strip is always allocated; the viewer area below it is always
/// allocated too, showing the empty-state prompt when no document is active.
pub fn center_panel(view_model: &CenterPanelViewModel) -> Element<'_, Message> {
    center_panel_rendered(view_model, None)
}

/// The center panel with a pre-rendered document body in the viewer area.
///
/// Same chrome as [`center_panel`]; the rendered element replaces the
/// placeholder title inside the always-allocated scrollable. The body is
/// passed in rather than held on the view-model because
/// [`crate::ui_iced::render::render_document`] produces an
/// `Element<'static, Message>`, and `Element` is neither `Clone` nor
/// `Debug` — a snapshot struct could not own one.
///
/// `None` keeps the placeholder behaviour, which is what the empty state
/// (no open tab) renders.
pub fn center_panel_rendered<'a>(
    view_model: &'a CenterPanelViewModel,
    document: Option<iced::Element<'static, Message>>,
) -> Element<'a, Message> {
    let body: Element<'a, Message> = if view_model.show_agent_session {
        agent_session_area(view_model)
    } else {
        Column::with_children(vec![
            tab_strip(view_model),
            viewer_area(view_model, document),
        ])
        .height(Length::Fill)
        .into()
    };

    container(body)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// The tab-strip band: one [`tab_widget`] per open document, laid out left
/// to right. An empty snapshot renders the same band with no children, so
/// the strip's shape never changes (UI-049).
fn tab_strip(view_model: &CenterPanelViewModel) -> Element<'_, Message> {
    let tabs = view_model.tabs.iter().enumerate().fold(
        Row::<Message>::new()
            .spacing(TAB_SPACING)
            .align_y(Alignment::Center),
        |tabs, (index, tab)| {
            tabs.push(tab_widget(
                tab,
                index,
                tab_is_hovered(tab, view_model.hovered_tab.as_deref()),
            ))
        },
    );

    // The tabs ride a horizontal `scrollable` (defect #16). iced's `Row`
    // never wraps and layout never clips, so once the tabs' combined minimum
    // exceeded the band the surplus tabs were laid out past its right edge
    // and painted over the neighbouring pane.
    //
    // `Scrollable::horizontal` sets `Direction::Horizontal`
    // (`iced_widget-0.14.2/src/scrollable.rs:133-135`), which hands the
    // content an unbounded maximum width
    // (`scrollable.rs:449-464`). The inner `Row` is already
    // content-sized (`Row::from_vec` defaults to `Length::Shrink`), so it
    // shrink-wraps to the tabs' real total width and that becomes the
    // scroll extent.
    //
    // The height is pinned to `TAB_HEIGHT` rather than left to `Fill` so the
    // band's own closure — `STRIP_HEIGHT (36) = STRIP_PADDING (4) +
    // TAB_HEIGHT (28) + STRIP_PADDING (4)` — cannot be moved by a scrollbar
    // rail. With the default `Scrollbar` (`spacing: None`,
    // `scrollable.rs:330-340`) the rail floats over the content and
    // consumes no layout anyway; the explicit height keeps that guarantee
    // from depending on a theme default.
    let scroller = scrollable(tabs)
        .id(TAB_STRIP_SCROLL_ID)
        .horizontal()
        .width(Length::Fill)
        .height(Length::Fixed(TAB_HEIGHT));

    container(scroller)
        .id(TAB_STRIP_BAND_ID)
        .width(Length::Fill)
        .height(Length::Fixed(STRIP_HEIGHT))
        .padding(STRIP_PADDING)
        .align_y(Alignment::Center)
        .into()
}

/// One tab: the title, the `×` close button, and the full select /
/// middle-close / right-menu / cursor-motion interaction.
///
/// `hovered` is the INT-12 pointer state, resolved by
/// [`tab_is_hovered`] against the tab's **path**. INT-12 routed it to
/// §4.4.1 slot 1's third indicator state only; INT-16 routes it to the
/// whole §4.4.2 matrix — fill, label, perimeter and the close glyph's own
/// hover sub-state. Active-tab styling is still hover-inert, so it cannot
/// regress, and no hover transition moves geometry: the indicator rides in a
/// `Stack` and takes no layout, and iced 0.14 never reads a container's
/// `border` in its layout path at all (see [`TAB_BORDER_WIDTH`]).
fn tab_widget<'a>(tab: &'a TabEntry, index: usize, hovered: bool) -> Element<'a, Message> {
    let palette = Palette::fastmd_dark();

    let label = text(&tab.title)
        .size(TAB_TEXT_SIZE)
        .line_height(TAB_RUNG.iced_line_height())
        .font(tab_label_font(tab.active))
        .color(tab_text_color(&palette, tab.active, hovered));

    // iced's `Button` lays its content at the top-left of its padding box, so
    // the glyph is centred by the container that fills the hit target.
    let close = Button::icon_only(
        ButtonVariant::Ghost,
        text(strings::ICON_X)
            .font(phosphor_font())
            .size(TAB_CLOSE_GLYPH_SIZE),
        tab_close_message(index),
        format!("tab_close_{}", index),
        strings::CLOSE_TAB_TOOLTIP,
    )
    .compact()
    .palette(Palette::pro());

    let surface = container(
        Row::with_children(vec![label.into(), close.into()])
            .spacing(TAB_CLOSE_SPACING)
            .align_y(Alignment::Center),
    )
    .id(tab_surface_id(index))
    .height(Length::Fixed(TAB_HEIGHT))
    .padding(Padding {
        top: TAB_PADDING_Y,
        right: TAB_PADDING_X,
        bottom: TAB_PADDING_Y,
        left: TAB_PADDING_X,
    })
    .style(move |_theme| container::Style {
        background: tab_fill(&palette, tab.active, hovered).map(Background::Color),
        border: iced::Border {
            width: TAB_BORDER_WIDTH,
            color: tab_perimeter(tab.active, hovered),
            ..Default::default()
        },
        ..container::Style::default()
    });

    let tab_body = crate::ui_iced::overlay::context_menu_trigger(
        surface,
        tab_context_menu_open_message(index),
    )
    .on_press(Message::Command(tab_select_command(&tab.path)))
    .on_middle_press(tab_close_message(index))
    // INT-12: the same edge-triggered hover crossing INT-10 put on the
    // tree rows. `mouse_area` publishes `on_enter` only on
    // `is_hovered && !was_hovered` and `on_exit` only on the inverse, and
    // skips the check entirely when neither the cursor nor its own bounds
    // moved (`iced_widget-0.14.2/src/mouse_area.rs:349-366`), so a
    // per-frame strip rebuild cannot flood the reducer with hover reports.
    .on_enter(Message::Tab(TabMessage::Entered {
        path: tab.path.clone(),
    }))
    .on_exit(Message::Tab(TabMessage::Left {
        path: tab.path.clone(),
    }));

    // The `2px` top accent indicator rides *over* the tab in a `Stack`
    // rather than inside its padding box, and that placement is the whole
    // reason it costs nothing: the bar takes no layout, so [`STRIP_HEIGHT`]
    // and [`TAB_HEIGHT`] keep the exact numbers the band closure pins
    // (`36 = 4 + 28 + 4`). It occupies `y ∈ [0, 2]` and the tab's content
    // box does not start until `y = TAB_PADDING_Y = 2`, so the bar cannot
    // cover the title in any posture.
    //
    // The bar is a *sibling* of the tab's mouse area, not a child of it,
    // and iced dispatches events without occlusion culling: the indicator
    // carries no handler of its own, so select, middle-close and the
    // right-click menu still land on the area underneath.
    let indicator = container(Space::new())
        .id(tab_indicator_id(index))
        .width(Length::Fill)
        .height(Length::Fixed(TAB_INDICATOR_HEIGHT))
        .style(move |_theme| container::Style {
            background: tab_top_indicator(tab.active, hovered).map(Background::Color),
            ..container::Style::default()
        });

    // The tab's own box is **shrink-wrapped**, not `Fill` (defect #15). The
    // `surface` inside it is content-sized, so a `Fill` box would let the
    // tab claim width the painted fill cannot follow. Today the strip's
    // `Row` is itself `Length::Shrink`, and iced propagates that as a
    // *compression* flag which already forces a `Fill` child down to its
    // intrinsic size (`iced_core-0.14.0/src/layout/limits.rs:160-170`), so
    // the two measure identical here — but that conformance was an accident
    // of the parent's length, not of this widget. Naming `Shrink` makes the
    // tab's box and its fill the same rectangle by construction, whatever
    // the strip above it decides to do.
    container(
        Stack::with_children(vec![tab_body.into(), indicator.into()])
            .width(Length::Shrink)
            .height(Length::Fixed(TAB_HEIGHT)),
    )
    .id(tab_slot_id(index))
    .width(Length::Shrink)
    .height(Length::Fixed(TAB_HEIGHT))
    .into()
}

/// The document-viewer area below the strip.
///
/// The `pdf_surface` frame tints the area for PDF-backed documents; the
/// empty state shows [`strings::NO_FILE_SELECTED_PROMPT`].
fn viewer_area<'a>(
    view_model: &'a CenterPanelViewModel,
    document: Option<iced::Element<'static, Message>>,
) -> Element<'a, Message> {
    let palette = Palette::fastmd_dark();
    let pdf_backed = view_model.active_pdf_backed;

    let body: Element<'a, Message> = match (view_model.active_tab_entry(), document) {
        (Some(_), Some(rendered)) => rendered,
        (Some(tab), None) => document_preview(tab),
        (None, _) => text(strings::NO_FILE_SELECTED_PROMPT)
            .size(EMPTY_STATE_SIZE)
            .line_height(BODY_RUNG.iced_line_height())
            .color(palette.text_muted)
            .into(),
    };

    container(
        scrollable(container(body).width(Length::Fill).padding(VIEWER_PADDING))
            .id(DOCUMENT_VIEWER_SCROLLABLE_ID)
            .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .style(move |_theme| container::Style {
        background: pdf_backed.then_some(Background::Color(palette.pdf_surface)),
        ..container::Style::default()
    })
    .into()
}

/// The active document's render block — the placeholder.
///
/// The real render (YAML front-matter table, search-aware markdown, FTWA
/// table strategy, task-checkbox toggles) is supplied to
/// [`center_panel_rendered`] by the root view; this is what shows when no
/// rendered body was handed in, so the pane is never blank.
fn document_preview<'a>(tab: &'a TabEntry) -> Element<'a, Message> {
    let palette = Palette::fastmd_dark();

    Column::with_children(vec![
        text(&tab.title)
            .size(DOCUMENT_TITLE_SIZE)
            .line_height(HEADER_RUNG.iced_line_height())
            .font(iced::Font {
                weight: HEADER_RUNG.weight.iced(),
                ..iced::Font::with_name("Fira Sans")
            })
            .color(palette.text_strong)
            .into(),
    ])
    .spacing(VIEWER_SPACING)
    .into()
}

/// The agent-session band that replaces the document view while
/// `show_results` is on.
///
/// Carries the header, the close intent and the UI-048 thinking block. The
/// streaming response markdown and the collapsible disclosure belong to the
/// agent-session render path.
fn agent_session_area(view_model: &CenterPanelViewModel) -> Element<'_, Message> {
    let palette = Palette::fastmd_dark();

    let header = Row::with_children(vec![
        text(strings::AGENT_SESSION_HEADER)
            .size(AGENT_HEADER_SIZE)
            .line_height(HEADER_RUNG.iced_line_height())
            .font(iced::Font {
                weight: HEADER_RUNG.weight.iced(),
                ..iced::Font::with_name("Fira Sans")
            })
            .color(palette.accent_text)
            .into(),
        container(
            Button::new(
                ButtonVariant::Ghost,
                text(strings::AGENT_SESSION_CLOSE_BUTTON)
                    .size(TAB_TEXT_SIZE)
                    .line_height(TAB_RUNG.iced_line_height()),
                agent_session_close_message(),
                AGENT_SESSION_CLOSE_BUTTON_ID,
            )
            .palette(Palette::pro()),
        )
        .width(Length::Fill)
        .align_right(Length::Shrink)
        .into(),
    ])
    .align_y(Alignment::Center);

    let status = text(format!(
        "{}{}",
        strings::AGENT_STATUS_PREFIX,
        view_model.agent_status
    ))
    .size(AGENT_BODY_SIZE)
    .line_height(BODY_RUNG.iced_line_height())
    .color(palette.text_strong);

    let thinking: Element<'_, Message> = if view_model.show_thinking() {
        Column::with_children(vec![
            text(strings::AGENT_THINKING_PROCESS)
                .size(AGENT_BODY_SIZE)
                .line_height(BODY_RUNG.iced_line_height())
                .color(palette.text_strong)
                .into(),
            text(&view_model.thinking)
                .size(AGENT_BODY_SIZE)
                .line_height(BODY_RUNG.iced_line_height())
                .font(italic_font())
                .color(palette.text_muted)
                .into(),
        ])
        .spacing(VIEWER_SPACING)
        .into()
    } else {
        Column::<Message>::new().into()
    };

    Column::with_children(vec![header.into(), status.into(), thinking])
        .spacing(VIEWER_SPACING)
        .padding(VIEWER_PADDING)
        .into()
}

/// Italic variant of the default UI font — iced's `Text` has no `.italic()`
/// helper, so the style rides on the [`Font`] struct (the same trick as
/// [`super::top::indexing_status_font`]).
fn italic_font() -> iced::Font {
    iced::Font {
        style: iced::font::Style::Italic,
        ..iced::Font::DEFAULT
    }
}
