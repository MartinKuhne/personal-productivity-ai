//! Headless layout probe: turns a rendered view into a deterministic text
//! manifest of every named widget's geometry.
//!
//! Bounds are the only layout claim that can be gated automatically. A
//! raster golden freezes the renderer, the font files and the hinting
//! settings of whichever machine drew it, so it can only ever be reviewed
//! by a human. This module instead walks the widget tree with a custom
//! [`iced_selector::Selector`] that records **every** candidate it is
//! handed and returns `None`, so traversal never stops early, and prints
//! the result as a sorted, fixed-precision text manifest.
//!
//! The manifest keys on **named** widget ids only. `iced_core::widget::Id`
//! exposes no text accessor — only `Debug` — and `Id::unique()` mints a
//! fresh counter value on every run, so anything addressed by a test must
//! carry an `Id::new("…")`-style stable name (see each pane module's
//! `*_ID` constants). Anonymous candidates are counted but never named,
//! which keeps the manifest byte-identical across runs.
//!
//! The harness mirrors `hoarfrost::test_helpers::Harness`: build a real
//! `UserInterface` over a real renderer, drive the operation through its
//! whole `Outcome::Chain`, then take the cache back out. `iced_test`'s
//! own `Simulator` keeps its interface private and only exposes the `One`
//! selection strategy, which cannot enumerate the tree.
//!
//! Unit tests live in the sibling `layout_probe_tests.rs` sidecar.

use std::sync::{Arc, Mutex};

use iced::advanced::renderer;
use iced::advanced::widget::operation::{Outcome, black_box};
use iced::advanced::widget::{Id, Operation};
use iced::{Element, Font, Pixels, Rectangle, Size};

use iced_test::futures::futures::executor::block_on;
use iced_test::renderer::Renderer as HeadlessRenderer;
use iced_test::runtime::user_interface::{self, UserInterface};
use iced_test::selector::{Candidate, Selector};

use crate::app::Message;

/// Number of decimal places a manifest coordinate is rounded to.
///
/// Two is enough to catch a real layout regression (which is usually off
/// by whole pixels) while discarding the sub-pixel jitter that float
/// accumulation introduces between otherwise identical builds.
pub const COORD_PRECISION: usize = 2;

/// The font the headless renderer is built with.
///
/// Matches the `fira-sans` iced feature the demo enables, so metrics here
/// match metrics on screen.
pub const PROBE_FONT: &str = "Fira Sans";

/// One recorded widget.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// The widget's stable name, from its `Id`.
    pub id: String,
    /// Which `Candidate` variant produced this record.
    pub kind: &'static str,
    /// The widget's layout bounds.
    pub bounds: Rectangle,
    /// The part of the widget inside the viewport, if the recorder saw one.
    pub visible: Option<Rectangle>,
}

impl Entry {
    /// One manifest line: `id [kind] | x y w h | visible x y w h`.
    ///
    /// The `kind` column is part of the key, not decoration: a single
    /// widget can register more than one candidate under the same `Id`
    /// (a focusable node plus the text node inside it), and without the
    /// kind the two lines look like a duplicated widget.
    #[must_use]
    pub fn line(&self) -> String {
        match self.visible {
            Some(visible) => format!(
                "{} [{}] | {} | {}",
                self.id,
                self.kind,
                Self::rect(&self.bounds),
                Self::rect(&visible)
            ),
            None => format!(
                "{} [{}] | {} | -",
                self.id,
                self.kind,
                Self::rect(&self.bounds)
            ),
        }
    }

    fn rect(rect: &Rectangle) -> String {
        format!(
            "{:.4$} {:.4$} {:.4$} {:.4$}",
            rect.x, rect.y, rect.width, rect.height, COORD_PRECISION
        )
    }
}

/// A captured layout snapshot.
#[derive(Debug, Clone)]
pub struct Probe {
    viewport: Size,
    entries: Vec<Entry>,
    all: Vec<Entry>,
    anonymous: usize,
}

impl Probe {
    /// Renders `root` at `viewport` and records every named widget.
    #[must_use]
    pub fn capture(viewport: Size, root: Element<'_, Message>) -> Self {
        let mut renderer = headless_renderer();
        Self::capture_with(&mut renderer, viewport, root)
    }

    /// Same as [`capture`](Self::capture), on a renderer the caller owns.
    ///
    /// Building the headless renderer costs roughly half a second — more
    /// than the layout pass and the raster put together — so anything that
    /// takes several snapshots should build one and hand it to this
    /// method rather than paying for a fresh renderer per snapshot.
    /// [`crate::render_capture::Studio`] exists to do exactly that.
    #[must_use]
    pub fn capture_with(
        renderer: &mut HeadlessRenderer,
        viewport: Size,
        root: Element<'_, Message>,
    ) -> Self {
        let mut ui: UserInterface<'_, Message, iced::Theme, HeadlessRenderer> =
            UserInterface::build(root, viewport, user_interface::Cache::default(), renderer);

        let store: Shared = Arc::new(Mutex::new(Vec::new()));
        run_operation(&mut ui, renderer, Recorder::new(Arc::clone(&store)));

        let entries = Arc::try_unwrap(store)
            .expect("the recorder is the only other owner of the entry store")
            .into_inner()
            .expect("the entry store is not poisoned");

        Self::assemble(viewport, entries)
    }

    /// Splits recorded candidates into the named and the anonymous view.
    fn assemble(viewport: Size, entries: Vec<Entry>) -> Self {
        let mut all = entries;
        for (index, entry) in all.iter_mut().enumerate() {
            if entry.id.is_empty() {
                entry.id = format!("#{} ({})", index, entry.kind);
            }
        }
        let anonymous = all.iter().filter(|e| e.id.starts_with('#')).count();
        let named: Vec<Entry> = all
            .iter()
            .filter(|e| !e.id.starts_with('#'))
            .cloned()
            .collect();

        Self {
            viewport,
            entries: named,
            all,
            anonymous,
        }
    }

    /// Every recorded candidate, named or not, in traversal order.
    ///
    /// Anonymous candidates get a synthetic `#<index> (kind)` id so a
    /// diagnostic line still says which node it came from.
    #[must_use]
    pub fn all_entries(&self) -> &[Entry] {
        &self.all
    }

    /// Every recorded candidate whose geometry is not finite.
    #[must_use]
    pub fn infinite_entries(&self) -> Vec<&Entry> {
        self.all
            .iter()
            .filter(|e| !is_finite_rect(&e.bounds))
            .collect()
    }

    /// Every named entry, in tree traversal order.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The viewport this probe was captured at.
    #[must_use]
    pub fn viewport(&self) -> Size {
        self.viewport
    }

    /// How many candidates carried no stable name.
    #[must_use]
    pub fn anonymous_count(&self) -> usize {
        self.anonymous
    }

    /// The entry with a given stable id, if present.
    #[must_use]
    pub fn find(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.id == id)
    }

    /// Every entry whose id starts with `prefix`.
    #[must_use]
    pub fn with_prefix(&self, prefix: &str) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| e.id.starts_with(prefix))
            .collect()
    }

    /// The byte-stable manifest: entries sorted by id, one per line.
    ///
    /// Sorting by id rather than traversal order means a section that moves
    /// within the pane shows up as a changed coordinate rather than a
    /// renumbered block, and the manifest does not depend on how the tree
    /// happened to be walked.
    #[must_use]
    pub fn manifest(&self) -> String {
        let mut lines: Vec<&Entry> = self.entries.iter().collect();
        lines.sort_by(|a, b| a.id.cmp(&b.id));

        let mut out = String::new();
        out.push_str(&format!(
            "# viewport {:.2} x {:.2}\n",
            self.viewport.width, self.viewport.height
        ));
        out.push_str(&format!(
            "# named {} / anonymous {}\n",
            self.entries.len(),
            self.anonymous
        ));
        for entry in lines {
            out.push_str(&entry.line());
            out.push('\n');
        }
        out
    }
}

type Shared = Arc<Mutex<Vec<Entry>>>;

/// A [`Selector`] that records every candidate and never matches.
///
/// Returning `None` from [`Selector::select`] is what keeps the traversal
/// going to the end of the tree: the `All` strategy only stops when its
/// selector says it is done, and this one never does.
struct Recorder {
    sink: Shared,
}

impl Recorder {
    fn new(sink: Shared) -> Self {
        Self { sink }
    }

    fn record(&self, candidate: &Candidate<'_>) {
        let id = candidate.id().and_then(stable_name).unwrap_or_default();
        let entry = Entry {
            id,
            kind: kind_of(candidate),
            bounds: candidate.bounds(),
            visible: candidate.visible_bounds(),
        };
        self.sink
            .lock()
            .expect("the entry store is not poisoned")
            .push(entry);
    }
}

impl Selector for Recorder {
    type Output = ();

    fn select(&mut self, candidate: Candidate<'_>) -> Option<()> {
        self.record(&candidate);
        None
    }

    fn description(&self) -> String {
        String::from("record-all-named-widgets")
    }
}

/// Whether every component of a rectangle is a finite number.
///
/// A `Length::Fill` resolved inside an unbounded main axis yields
/// `f32::INFINITY`, which silently poisons every following sibling's `y`
/// downstream of it. Surfacing it as a boolean is what lets the gate fail
/// loudly instead of printing `inf` in a golden nobody reads.
#[must_use]
pub fn is_finite_rect(rect: &Rectangle) -> bool {
    rect.x.is_finite() && rect.y.is_finite() && rect.width.is_finite() && rect.height.is_finite()
}

/// The `Candidate` variant's name, for the manifest's provenance column.
fn kind_of(candidate: &Candidate<'_>) -> &'static str {
    match candidate {
        Candidate::Container { .. } => "container",
        Candidate::Focusable { .. } => "focusable",
        Candidate::Scrollable { .. } => "scrollable",
        Candidate::TextInput { .. } => "text_input",
        Candidate::Text { .. } => "text",
        Candidate::Custom { .. } => "custom",
    }
}

/// The stable name inside an [`Id`], or `None` if the id is not stable.
///
/// `Id` has no public accessor for its name — only `Debug` — so the
/// variant has to be read off the debug rendering. `Id::new("x")` and
/// `Id::from(String)` both render as a `Custom` variant carrying the
/// quoted name; `Id::unique()` renders as `Unique(n)`, where `n` is a
/// process-global counter that changes between runs, so it is rejected.
#[must_use]
pub fn stable_name(id: &Id) -> Option<String> {
    let rendered = format!("{id:?}");
    if !rendered.contains("Custom") {
        return None;
    }
    // UFCS because `Selector::find` is in scope in this module and would
    // otherwise shadow `str::find`.
    let start = <str>::find(&rendered, '"')?;
    let end = <str>::rfind(&rendered, '"')?;
    if end <= start {
        return None;
    }
    Some(rendered[start + 1..end].to_string())
}

/// Builds iced's real software renderer, with no window and no GPU.
///
/// Registers the symbol fallback first. Every headless path in this crate
/// — the probe, the gate, the alignment audit, and any pane captured on
/// its own without going through [`crate::app::DemoApp::boot`] — comes
/// through here, so this is the one place that guarantees the tests shape
/// text against the same font database the user's window does.
///
/// The framework's own `test_helpers::headless_renderer()` pins
/// `Font::with_name("Fira Sans")` and is not touched; the fallback lives
/// in the process-wide font system, which that renderer shares.
#[must_use]
pub fn headless_renderer() -> HeadlessRenderer {
    crate::font_fallback::register();

    block_on(<HeadlessRenderer as renderer::Headless>::new(
        Font::with_name(PROBE_FONT),
        Pixels(16.0),
        None,
    ))
    .expect("a headless renderer should be available for demo layout probes")
}

/// Runs `selector` over `ui`'s whole widget tree, driving the operation
/// chain the way `iced_winit`'s `Action::Widget` handler does.
///
/// A single `operate` call runs only part of a chained operation, so
/// without this loop a traversal would stop early and the manifest would
/// silently be missing widgets.
fn run_operation<S>(
    ui: &mut UserInterface<'_, Message, iced::Theme, HeadlessRenderer>,
    renderer: &HeadlessRenderer,
    selector: S,
) where
    S: Selector<Output = ()> + Send + 'static,
{
    let mut operation: Box<dyn Operation<Vec<()>>> = Box::new(selector.find_all());
    loop {
        ui.operate(renderer, &mut black_box(operation.as_mut()));
        match operation.finish() {
            Outcome::None | Outcome::Some(_) => break,
            Outcome::Chain(next) => operation = next,
        }
    }
}

/// Collects the visible text of a rendered element, for content assertions.
///
/// `Element` exposes no text accessor, so this walks the same candidate
/// stream the probe uses and keeps every `Candidate::Text` payload.
#[must_use]
pub fn text_of<'a>(root: Element<'a, Message>) -> String {
    let store: Shared = Arc::new(Mutex::new(Vec::new()));
    let mut renderer = headless_renderer();
    let mut ui: UserInterface<'_, Message, iced::Theme, HeadlessRenderer> = UserInterface::build(
        root,
        Size::new(1400.0, 900.0),
        user_interface::Cache::default(),
        &mut renderer,
    );

    struct TextCollector(Shared);
    impl Selector for TextCollector {
        type Output = ();
        fn select(&mut self, candidate: Candidate<'_>) -> Option<()> {
            if let Candidate::Text { content, .. } = candidate {
                self.0
                    .lock()
                    .expect("the text store is not poisoned")
                    .push(Entry {
                        id: (*content).to_string(),
                        kind: "text",
                        bounds: candidate.bounds(),
                        visible: candidate.visible_bounds(),
                    });
            }
            None
        }
        fn description(&self) -> String {
            String::from("collect-text")
        }
    }

    run_operation(&mut ui, &renderer, TextCollector(Arc::clone(&store)));

    Arc::try_unwrap(store)
        .expect("the collector is the only other owner")
        .into_inner()
        .expect("the text store is not poisoned")
        .into_iter()
        .map(|e| e.id)
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
#[path = "layout_probe_tests.rs"]
mod tests;
