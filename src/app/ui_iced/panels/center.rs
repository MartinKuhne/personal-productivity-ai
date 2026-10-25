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
//! * **Unsaved-changes confirmation (UI-027/UI-029).** The app has **no**
//!   per-tab dirty tracking anywhere (`Tabs` has no dirty flag and no
//!   editor-modified signal reaches the tab strip), so [`TabEntry::unsaved`]
//!   is reported `false` by `from_app`. The field and its marker exist so
//!   the view is ready for it; the reducer populates it when dirty tracking
//!   lands. Flagged as spec drift per [RUST-041].
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
//! Every size and gap in this module is bound to a `iced_pro` token; the doc
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
//! Two §4.4.1 details are **not** implemented, both for the same structural
//! reason — they need a layer the current widget tree cannot paint:
//!
//! * **The 2 px top accent indicator** (`accent.border` active,
//!   `border.strong` hovered) needs a `Stack` per tab or a row widget that
//!   reports hover into its own style. Same blocker as the file tree's 2 px
//!   selection bar (INT-7b follow-on): iced 0.14's `mouse_area` never
//!   reports hover into a `container` style.
//! * **The strip's 1 px `border.subtle` bottom rule** is not painted either.
//!   iced's `Border` is a uniform perimeter, so a bottom-only hairline needs
//!   a `rule::horizontal` sibling inside the band — which would push the
//!   band's content box past 36 unless the height is re-derived. Left as a
//!   stated follow-on rather than folded in here.
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

use iced::widget::{Column, Row, button, container, scrollable, text};
use iced::{Alignment, Background, Element, Length, Padding};

use iced_pro::token::contrast;
use iced_pro::token::typography::{self, TypeRung};
use iced_pro::token::{size, space};

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

/// Horizontal padding inside one tab — `space.md` (`UIF-TOK-001`).
///
/// **Movement: 10 → 12 (+2 per side, +4 pt per tab).** `10` is not on the
/// base-4 grid, and `UIF-TOK-001` states that a spacing value which is not
/// one of the rungs must not appear in layout code. The tie between `SM` (8)
/// and `MD` (12) is not a coin flip here: `space.md`'s own doc names itself
/// as the "standard horizontal padding inside buttons, inputs, **tabs** and
/// menu rows", so the tab is the token's stated role and the old `10` was an
/// unowned number sitting 2 pt off the rung that was drawn for it.
pub const TAB_PADDING_X: f32 = space::MD;

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

/// Gap between a tab's unsaved marker and its title — `space.2xs`
/// (`UIF-TOK-001`). Value unchanged at `2`.
pub const TAB_MARKER_SPACING: f32 = space::XXS;

/// Gap between a tab's label and its `×` close button — `space.xs`
/// (`UIF-TOK-001`).
///
/// Value unchanged at `4`, which is also §4.4.1's stated separation between
/// the close button and the tab activation zone (`>= 2px`,
/// `UIF-PRIN-008`) with room to spare.
pub const TAB_CLOSE_SPACING: f32 = space::XS;

/// Fixed width of the unsaved-changes marker slot — `space.md`
/// (`UIF-TOK-001`), which is exactly §4.4.1's "Leading Status Slot
/// (`12px`)".
///
/// Value unchanged at `12`. Kept as a fixed slot rather than shrink-to-fit so
/// a tab keeps the same shape whether or not its document is dirty (the
/// conditional-rendering rule: allocate, then toggle the content).
pub const UNSAVED_MARKER_WIDTH: f32 = space::MD;

/// Rendered size of the unsaved dot inside that slot — the type floor.
///
/// **Movement: 13 → 12 (−1).** The dot used to be painted at the tab label's
/// own point size, which made it as loud as the title.
///
/// **A spec conflict, recorded rather than papered over.** §4.4.1 asks for a
/// `6px` marker (`size.indicator.dot`), and [`size::INDICATOR_DOT`] is that
/// token. It cannot be honoured here: this marker is painted with a **font
/// glyph** (`strings::ICON_UNSAVED_DOT`), and §2.4's rule is absolute —
/// *"No text anywhere in the interface may render smaller than 12px"*. A
/// `text(..).size(6.0)` call is precisely the shape the INT-7c floor guard
/// exists to catch, so taking the token literally would mean either breaking
/// the floor or exempting this one call site and gutting the guard.
///
/// The marker is therefore pinned at [`typography::MIN_SIZE`] — the smallest
/// size that is still legal text, and as much quieter than the label rung as
/// a glyph can get. Rendering a true 6 px **shape** needs a quad/canvas
/// painter under the tab, which is the same missing layer as the 2 px accent
/// indicator recorded in the module header; when that lands this constant
/// should move to [`size::INDICATOR_DOT`] and the guard should keep its teeth
/// by scoping to legible text only.
///
/// [`size::INDICATOR_DOT`]: iced_pro::token::size::INDICATOR_DOT
/// [`typography::MIN_SIZE`]: iced_pro::token::typography::MIN_SIZE
pub const UNSAVED_DOT_SIZE: f32 = typography::MIN_SIZE;

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
/// Every tab starts **not** unsaved: the shipped build tracks no dirty state
/// (see the module header), so this is the honest default rather than a
/// placeholder.
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
            unsaved: false,
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

/// The marker rendered in a tab's fixed-width unsaved slot: the dot glyph
/// when the document is dirty, a blank of the same width otherwise, so the
/// tab's shape never changes ([RUST-021]: the glyph itself is a
/// [`strings`] constant).
///
/// Purity: pure function of `unsaved`.
#[must_use]
pub fn unsaved_marker(unsaved: bool) -> &'static str {
    if unsaved {
        strings::ICON_UNSAVED_DOT
    } else {
        ""
    }
}

/// Label colour for a tab: `text_strong` on the active tab, `text_muted` on
/// the others. Only palette tokens are used; no numeric colour is added.
///
/// Purity: pure function of `active` for a given palette.
#[must_use]
pub fn tab_text_color(palette: &Palette, active: bool) -> iced::Color {
    if active {
        palette.text_strong
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
    TAB_RUNG.font(typography::Family::Sans, weight)
}

/// The fill a tab paints behind its contents: `palette.accent` on the active
/// tab, nothing on the others.
///
/// Extracted so the dot-colour rule below can ask *what is behind the dot*
/// rather than assume — which is exactly what [`contrast::fill_lock`] says a
/// foreground audit has to do, because the style itself does not know.
///
/// Purity: pure function of `active` for a given palette.
#[must_use]
pub fn tab_fill(palette: &Palette, active: bool) -> Option<iced::Color> {
    active.then_some(palette.accent)
}

/// Colour of a tab's unsaved dot, given whether that tab is active.
///
/// **A real contrast fix, not a rename.** On an inactive tab the dot is
/// `status.warning` over the pane surface — `6.86:1` on `surface.base`,
/// `6.45:1` on `surface.raised`, comfortably legal. On the **active** tab
/// the app paints `palette.accent` behind it, and `palette.accent` *is*
/// `color.accent.fill` — one of the four solid fills `UIF-TOK-006` rule 4
/// locks. `status.warning` on `accent.fill` measures **`1.65:1`**, so the
/// dirty marker was effectively invisible on precisely the tab the user is
/// looking at.
///
/// Rule 4's answer is role-based rather than threshold-based: every
/// foreground painted inside a locked fill promotes to `color.text.strong`
/// (`4.80:1` against `accent.fill`). That is what this does — the same
/// `locked_over` shape INT-7b put on the file tree's row label, so the lock
/// cannot release on an arithmetic accident.
///
/// Purity: pure function of `active` for a given palette.
#[must_use]
pub fn tab_unsaved_dot_color(palette: &Palette, active: bool) -> iced::Color {
    let pro = Palette::pro();
    match tab_fill(palette, active) {
        Some(fill) if contrast::fill_lock_role(fill, &pro).is_locked_fill() => palette.text_strong,
        _ => palette.warning,
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabEntry {
    /// The document's full path — what every tab command carries.
    pub path: PathBuf,
    /// The rendered label (the file name).
    pub title: String,
    /// Whether the document has unsaved changes (UI-027). Reported `false`
    /// until dirty tracking lands — see the module header.
    pub unsaved: bool,
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
            unsaved: false,
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
        |tabs, (index, tab)| tabs.push(tab_widget(tab, index)),
    );

    container(tabs)
        .width(Length::Fill)
        .height(Length::Fixed(STRIP_HEIGHT))
        .padding(STRIP_PADDING)
        .align_y(Alignment::Center)
        .into()
}

/// One tab: the unsaved marker slot, the title, the `×` close button, and
/// the full select / middle-close / right-menu / cursor-motion interaction.
fn tab_widget<'a>(tab: &'a TabEntry, index: usize) -> Element<'a, Message> {
    let palette = Palette::fastmd_dark();

    let marker = container(
        text(unsaved_marker(tab.unsaved))
            .size(UNSAVED_DOT_SIZE)
            .line_height(TAB_RUNG.iced_line_height())
            .color(tab_unsaved_dot_color(&palette, tab.active)),
    )
    .width(Length::Fixed(UNSAVED_MARKER_WIDTH))
    .align_x(Alignment::Center);

    let label = text(&tab.title)
        .size(TAB_TEXT_SIZE)
        .line_height(TAB_RUNG.iced_line_height())
        .font(tab_label_font(tab.active))
        .color(tab_text_color(&palette, tab.active));

    let label_row = Row::with_children(vec![marker.into(), label.into()])
        .spacing(TAB_MARKER_SPACING)
        .align_y(Alignment::Center);

    // iced's `Button` lays its content at the top-left of its padding box, so
    // the glyph is centred by the container that fills the hit target.
    let close = button(
        container(
            text(strings::ICON_X)
                .font(phosphor_font())
                .size(TAB_CLOSE_GLYPH_SIZE),
        )
        .width(Length::Fixed(TAB_CLOSE_HIT_TARGET))
        .height(Length::Fixed(TAB_CLOSE_HIT_TARGET))
        .align_x(Alignment::Center)
        .align_y(Alignment::Center),
    )
    .padding(0.0)
    .on_press(tab_close_message(index));

    let surface = container(
        Row::with_children(vec![label_row.into(), close.into()])
            .spacing(TAB_CLOSE_SPACING)
            .align_y(Alignment::Center),
    )
    .height(Length::Fixed(TAB_HEIGHT))
    .padding(Padding {
        top: TAB_PADDING_Y,
        right: TAB_PADDING_X,
        bottom: TAB_PADDING_Y,
        left: TAB_PADDING_X,
    })
    .style(move |_theme| container::Style {
        background: tab_fill(&palette, tab.active).map(Background::Color),
        ..container::Style::default()
    });

    crate::ui_iced::overlay::context_menu_trigger(surface, tab_context_menu_open_message(index))
        .on_press(Message::Command(tab_select_command(&tab.path)))
        .on_middle_press(tab_close_message(index))
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
            .font(HEADER_RUNG.font(typography::Family::Sans, HEADER_RUNG.weight))
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
            .font(HEADER_RUNG.font(typography::Family::Sans, HEADER_RUNG.weight))
            .color(palette.accent_text)
            .into(),
        container(
            button(
                text(strings::AGENT_SESSION_CLOSE_BUTTON)
                    .size(TAB_TEXT_SIZE)
                    .line_height(TAB_RUNG.iced_line_height()),
            )
            .on_press(agent_session_close_message()),
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
