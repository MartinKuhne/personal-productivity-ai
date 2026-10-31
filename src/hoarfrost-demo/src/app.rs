//! The demo application's state, message channel and root view.
//!
//! [`DemoApp`] is the whole iced `Application` triple: [`boot`](DemoApp::boot)
//! mints the initial state, [`update`](DemoApp::update) folds a
//! [`Message`] into it, and [`view`](DemoApp::view) renders the shell.
//!
//! Per the plan's constraint C4 there is exactly **one** real behaviour in
//! this program: [`Message::Close`] ends the run through [`iced::exit`].
//! Every other message mutates local demo state so the control under test
//! still feels alive — nothing is persisted, no file is read, no socket is
//! opened and no clock is read.
//!
//! The view is three bands: the chrome ([`crate::chrome`]), the two panes
//! ([`crate::pane_atoms`] and [`crate::pane_composites`]) separated by a
//! framework [`SplitDivider`](hoarfrost::composite::split_divider::SplitDivider), and
//! the status bar. This module owns only the composition and the state the
//! panes mutate.

use iced::Element;
use iced::widget::{Column, scrollable};

use hoarfrost::composite::navbar::NavBarModel;
use hoarfrost::composite::split_divider::{SplitAxis, SplitDividerModel};
use hoarfrost::surface::menu::model::Menu;

use crate::chrome;
use crate::data;
use crate::menu_surface;
use crate::pane_atoms;
use crate::pane_composites;

/// User-facing text owned by this module.
pub mod strings {
    /// The window title.
    pub const WINDOW_TITLE: &str = "hoarfrost demo";
    /// The event text reported when a navbar trigger is latched open.
    pub const NAVBAR_OPENED: &str = "navbar trigger latched";
    /// The event text reported when every navbar menu is closed.
    pub const NAVBAR_CLOSED: &str = "navbar menus closed";
    /// The event text reported when a mounted menu row is activated.
    pub const MENU_ROW_ACTIVATED: &str = "menu row activated";
    /// The event text reported while the splitter is being dragged.
    pub const SPLIT_DRAGGING: &str = "split divider dragging";
    /// The event text reported when a split drag lands.
    pub const SPLIT_COMMITTED: &str = "split divider committed";
    /// The event text reported when a split drag is abandoned.
    pub const SPLIT_CANCELLED: &str = "split divider cancelled";
}

/// The demo's message channel.
#[derive(Debug, Clone)]
pub enum Message {
    /// Quit the application — the one real behaviour in this program.
    Close,
    /// A navbar trigger was pressed; the payload is the trigger's id.
    NavbarToggled(String),
    /// A row of the mounted menu card was activated; the payload is the
    /// row's key.
    MenuRowActivated(String),
    /// The split ratio changed live during a drag.
    SplitLive(f32),
    /// The split drag landed and the ratio is committed.
    SplitCommitted(f32),
    /// The split drag was abandoned and the ratio rolled back.
    SplitCancelled,
    /// An atom in the left pane reported something.
    Atoms(pane_atoms::AtomsMessage),
    /// A composite in the right pane reported something.
    Composites(pane_composites::CompositesMessage),
}

/// Everything the demo application carries between frames.
#[derive(Debug)]
pub struct DemoApp {
    /// The navbar's declarative model, held so the latch survives a frame.
    navbar: NavBarModel,
    /// The menu tree the navbar's card renders while a trigger is latched.
    menu: Menu,
    /// The most recent demo event, shown in the status bar.
    last_event: String,
    /// Current split ratio between the two panes.
    split_ratio: f32,
    /// State owned by the atoms pane (controlled-widget models live here).
    atoms: pane_atoms::AtomsState,
    /// State owned by the composites pane (controlled-widget models live here).
    composites: pane_composites::CompositesState,
}

impl DemoApp {
    /// The initial state.
    ///
    /// Registers the symbol fallback font ([`crate::font_fallback`])
    /// before anything else. This is the one boot path the real window and
    /// every headless capture share, and it runs before iced lays out or
    /// paints a single widget — which is the only point at which a newly
    /// loaded face is guaranteed to be seen. cosmic-text caches its
    /// per-`Attrs` font-match list in `FontSystem::font_matches_cache`
    /// and `load_font` does not invalidate it, so a font registered
    /// *after* the first frame could be skipped by faces already resolved
    /// in that frame.
    pub fn boot() -> (Self, iced::Task<Message>) {
        crate::font_fallback::register();

        (
            Self {
                navbar: data::navbar_model(),
                menu: data::menu_model(),
                last_event: String::from(data::STATUS_READY),
                split_ratio: data::SPLIT_RATIO_DEFAULT,
                atoms: pane_atoms::AtomsState::new(),
                composites: pane_composites::CompositesState::new(),
            },
            iced::Task::none(),
        )
    }

    /// Folds one [`Message`] into the state.
    ///
    /// [`Message::Close`] is the only message with a real effect; every
    /// other variant records its description in [`Self::last_event`] (and,
    /// for the splitter, a new ratio).
    pub fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::Close => return iced::exit(),
            Message::NavbarToggled(id) => {
                self.navbar.toggle_menu(&id);
                self.last_event = if self.navbar.active_menu_id.is_some() {
                    format!("{}: {id}", strings::NAVBAR_OPENED)
                } else {
                    String::from(strings::NAVBAR_CLOSED)
                };
            }
            Message::MenuRowActivated(key) => {
                self.last_event = format!("{}: {key}", strings::MENU_ROW_ACTIVATED);
            }
            Message::SplitLive(ratio) => {
                self.split_ratio = ratio;
                self.last_event = String::from(strings::SPLIT_DRAGGING);
            }
            Message::SplitCommitted(ratio) => {
                self.split_ratio = ratio;
                self.last_event = format!("{}: {ratio:.2}", strings::SPLIT_COMMITTED);
            }
            Message::SplitCancelled => {
                self.split_ratio = data::SPLIT_RATIO_DEFAULT;
                self.last_event = String::from(strings::SPLIT_CANCELLED);
            }
            Message::Atoms(atoms) => {
                self.atoms.apply(&atoms);
                self.last_event = pane_atoms::AtomsState::describe(&atoms);
            }
            Message::Composites(composite) => {
                self.composites.apply(&composite);
                self.last_event = self.composites.describe(&composite);
            }
        }
        iced::Task::none()
    }

    /// The navbar trigger currently latched open, if any.
    #[must_use]
    pub fn active_menu_id(&self) -> Option<&str> {
        self.navbar.active_menu_id.as_deref()
    }

    /// The atoms pane's controlled-widget state.
    #[must_use]
    pub fn atoms(&self) -> &crate::pane_atoms::AtomsState {
        &self.atoms
    }

    /// The composites pane's controlled-widget state.
    #[must_use]
    pub fn composites(&self) -> &crate::pane_composites::CompositesState {
        &self.composites
    }

    /// Total number of demo widgets this frame lays down.
    ///
    /// A latched navbar trigger adds the rows of the card it mounts, so the
    /// count the status bar shows tracks what is actually on screen.
    #[must_use]
    pub fn widget_count(&self) -> usize {
        let atoms = pane_atoms::atoms_pane(self.atoms()).widgets;
        let composites = self.composites.widget_count();
        let mounted = match self.active_menu_id() {
            Some(_) => menu_surface::mounted_row_count(&self.menu),
            None => 0,
        };
        chrome::WIDGET_COUNT + atoms + composites + mounted
    }

    /// Renders the whole window.
    pub fn view(&self) -> Element<'_, Message> {
        let left = pane_atoms::atoms_pane(self.atoms());
        let right = pane_composites::composites_pane(self.composites());

        let divider = SplitDividerModel::new(SplitAxis::Horizontal, self.split_ratio)
            .with_min_ratio(data::SPLIT_RATIO_MIN)
            .with_max_ratio(data::SPLIT_RATIO_MAX);

        let (left_portion, right_portion) = fill_portions(self.split_ratio);

        let panes = iced::widget::Row::new()
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .push(
                scrollable(left.sections)
                    .id(pane_atoms::PANE_SCROLL_ID)
                    .width(iced::Length::FillPortion(left_portion))
                    .height(iced::Length::Fill),
            )
            .push(
                hoarfrost::composite::split_divider::split_divider(divider)
                    .on_resize_change(|live| Message::SplitLive(*live.value()))
                    .on_resize_commit(|commit| Message::SplitCommitted(*commit.value()))
                    .on_resize_cancel(|_cancel| Message::SplitCancelled),
            )
            .push(
                scrollable(right.sections)
                    .id(crate::pane_composites::PANE_SCROLL_ID)
                    .width(iced::Length::FillPortion(right_portion))
                    .height(iced::Length::Fill),
            );

        let base: Element<'_, Message> = Column::new()
            .width(iced::Length::Fill)
            .height(iced::Length::Fill)
            .push(chrome::navbar_row(&self.navbar))
            .push(panes)
            .push(chrome::status_bar(&self.last_event, self.widget_count()))
            .into();

        // The card is mounted only while a trigger is latched open, so a
        // closed menu leaves the rendered tree exactly as it was — no
        // phantom surface, and no change to the closed-menu goldens.
        match self.active_menu_id() {
            Some(trigger) => menu_surface::mount(base, trigger, &self.menu),
            None => base,
        }
    }
}

/// How finely the split ratio is quantised into iced fill portions.
///
/// A thousand parts keeps every ratio the demo allows representable to
/// better than a tenth of a percent, and keeps both portions well inside
/// `u16`'s range.
const PORTION_TOTAL: u16 = 1000;

/// Turns the divider's ratio into the two panes' iced fill portions.
///
/// `SplitDivider` is a fixed-width grab handle: it *reports* a ratio
/// through its callbacks but never resizes its siblings. The caller owns
/// that, so the ratio has to be carried back into the panes' widths —
/// otherwise the handle drags, the status line updates, and nothing in
/// the window actually moves.
///
/// Fill portions are used rather than fixed pixel widths because the demo
/// has no live viewport to measure, and portions keep the split correct
/// at every window size.
#[must_use]
pub fn fill_portions(ratio: f32) -> (u16, u16) {
    // `f32::clamp` passes `NaN` straight through (every comparison with
    // `NaN` is false), and `NaN as u16` saturates to `0` — which would
    // hand one pane no space at all. A non-finite ratio therefore falls
    // back to the boot default rather than reaching layout.
    let sane = if ratio.is_finite() {
        ratio
    } else {
        data::SPLIT_RATIO_DEFAULT
    };
    let clamped = sane.clamp(data::SPLIT_RATIO_MIN, data::SPLIT_RATIO_MAX);
    let scaled = (clamped * f32::from(PORTION_TOTAL)).round();

    // Neither pane may collapse to zero portions, or iced would give it no
    // space at all and the divider would sit against the window edge.
    let left = scaled.clamp(1.0, f32::from(PORTION_TOTAL - 1)) as u16;

    (left, PORTION_TOTAL - left)
}

#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
