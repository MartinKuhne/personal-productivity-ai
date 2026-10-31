//! Unit tests for the diff viewer composite (`UIF-COMP-015`).

use super::*;
use crate::binding::Commit;
use crate::test_helpers::Candidate;
use crate::test_helpers::{
    Capture, Harness, KIND_CONTAINER, RecordCandidates, assert_repainted, centre, headless_renderer,
};
use crate::token::{shape, size};
use iced::Point;
use iced::advanced::widget::operation::black_box;
use iced::advanced::widget::tree::State as TreeState;

#[test]
fn test_diff_line_constructors_and_kinds() {
    let ctx = DiffLine::context(10, 10, "unchanged context line");
    assert_eq!(ctx.kind, DiffLineKind::Context);
    assert_eq!(ctx.old_line, Some(10));
    assert_eq!(ctx.new_line, Some(10));
    assert_eq!(ctx.text, "unchanged context line");

    let add = DiffLine::added(11, "newly inserted line");
    assert_eq!(add.kind, DiffLineKind::Added);
    assert_eq!(add.old_line, None);
    assert_eq!(add.new_line, Some(11));
    assert_eq!(add.text, "newly inserted line");

    let del = DiffLine::deleted(10, "old deleted line");
    assert_eq!(del.kind, DiffLineKind::Deleted);
    assert_eq!(del.old_line, Some(10));
    assert_eq!(del.new_line, None);
    assert_eq!(del.text, "old deleted line");
}

#[test]
fn test_diff_hunk_and_model_creation() {
    let hunk = DiffHunk::new(
        "hunk-1",
        "@@ -1,3 +1,3 @@",
        vec![
            DiffLine::context(1, 1, "fn main() {"),
            DiffLine::deleted(2, "    println!(\"old\");"),
            DiffLine::added(2, "    println!(\"new\");"),
            DiffLine::context(3, 3, "}"),
        ],
    );

    assert_eq!(hunk.id, "hunk-1");
    assert_eq!(hunk.header, "@@ -1,3 +1,3 @@");
    assert_eq!(hunk.lines.len(), 4);

    let model = DiffViewerModel::new(vec![hunk]);
    assert_eq!(model.mode, DiffDisplayMode::Unified);
    assert_eq!(model.hunks.len(), 1);

    let side_by_side = model.with_mode(DiffDisplayMode::SideBySide);
    assert_eq!(side_by_side.mode, DiffDisplayMode::SideBySide);
}

#[test]
fn test_geometry_constants_conform_to_spec() {
    assert_eq!(HEADER_HEIGHT, size::CONTROL_SM);
    assert_eq!(HEADER_HEIGHT, 28.0);

    assert_eq!(GUTTER_WIDTH, 40.0);
    assert_eq!(INDICATOR_BAR_WIDTH, shape::BORDER_FOCUS);
    assert_eq!(INDICATOR_BAR_WIDTH, 2.0);

    assert_eq!(RADIUS, shape::RADIUS_WIDGET);
    assert_eq!(RADIUS, 4.0);

    assert_eq!(BORDER_WIDTH, shape::BORDER_HAIRLINE);
    assert_eq!(BORDER_WIDTH, 1.0);

    assert_eq!(DIVIDER_WIDTH, shape::BORDER_HAIRLINE);
    assert_eq!(DIVIDER_WIDTH, 1.0);

    assert_eq!(BUTTON_HEIGHT, size::HIT_TARGET_MIN);
    assert_eq!(BUTTON_HEIGHT, 24.0);

    assert_eq!(GLYPH_ADDED, "+");
    assert_eq!(GLYPH_DELETED, "-");
    assert_eq!(GLYPH_CONTEXT, " ");
}

#[test]
fn test_contrast_and_color_pairings_for_redundant_cues() {
    // UIF-PRIN-005 & UIF-PRIN-006: Redundant cues (glyph + 2px bar + text role),
    // never relying on background tint alone.
    let palette = Palette::dark();

    // Added: '+' glyph, 2px status.success bar, text.strong foreground
    assert_eq!(GLYPH_ADDED, "+");
    assert_eq!(INDICATOR_BAR_WIDTH, 2.0);
    assert_eq!(palette.status_success, Color::from_rgb8(100, 255, 100));
    assert_eq!(palette.text_strong, Color::from_rgb8(255, 255, 255));

    // Deleted: '-' glyph, 2px status.danger.text bar, text.primary foreground
    assert_eq!(GLYPH_DELETED, "-");
    assert_eq!(palette.status_danger_text, Color::from_rgb8(255, 92, 92));
    assert_eq!(palette.text_primary, Color::from_rgb8(210, 210, 210));

    // Context: ' ' glyph, text.muted foreground
    assert_eq!(GLYPH_CONTEXT, " ");
    assert_eq!(palette.text_muted, Color::from_rgb8(160, 160, 160));
}

#[test]
fn test_build_content_unified_mode() {
    let hunk = DiffHunk::new(
        1,
        "@@ -10,2 +10,3 @@",
        vec![
            DiffLine::context(10, 10, "let mut x = 1;"),
            DiffLine::deleted(11, "x += 1;"),
            DiffLine::added(11, "x += 2;"),
            DiffLine::added(12, "println!(\"{x}\");"),
        ],
    );
    let model = DiffViewerModel::new(vec![hunk]).with_mode(DiffDisplayMode::Unified);
    let elem: Element<'_, ()> = build_diff_viewer_content(&model, None, None, None);
    assert_eq!(elem.as_widget().size().width, Length::Fill);
}

#[test]
fn test_build_content_side_by_side_mode() {
    let hunk = DiffHunk::new(
        1,
        "@@ -10,2 +10,3 @@",
        vec![
            DiffLine::context(10, 10, "let mut x = 1;"),
            DiffLine::deleted(11, "x += 1;"),
            DiffLine::added(11, "x += 2;"),
            DiffLine::added(12, "println!(\"{x}\");"),
        ],
    );
    let model = DiffViewerModel::new(vec![hunk]).with_mode(DiffDisplayMode::SideBySide);
    let elem: Element<'_, ()> = build_diff_viewer_content(&model, None, None, None);
    assert_eq!(elem.as_widget().size().width, Length::Fill);
}

#[test]
fn test_widget_callbacks_and_emissions() {
    #[derive(Debug, Clone, PartialEq, Eq)]
    enum DiffMsg {
        Accept(&'static str),
        Reject(&'static str),
        Mode(DiffDisplayMode),
    }

    let hunk = DiffHunk::new(
        "hunk-alpha",
        "@@ -1 +1 @@",
        vec![DiffLine::context(1, 1, "test")],
    );
    let model = DiffViewerModel::new(vec![hunk]);

    let mut accepted = None;
    let mut rejected = None;
    let mut switched_mode = None;

    let widget = diff_viewer(model)
        .on_accept_hunk(|commit: Commit<&'static str>| DiffMsg::Accept(commit.into_value()))
        .on_reject_hunk(|commit: Commit<&'static str>| DiffMsg::Reject(commit.into_value()))
        .on_mode_change(|commit: Commit<DiffDisplayMode>| DiffMsg::Mode(commit.into_value()));

    if let Some(on_accept) = &widget.on_accept_hunk {
        let msg = (on_accept)(Commit::new("hunk-alpha"));
        if let DiffMsg::Accept(id) = msg {
            accepted = Some(id);
        }
    }
    if let Some(on_reject) = &widget.on_reject_hunk {
        let msg = (on_reject)(Commit::new("hunk-alpha"));
        if let DiffMsg::Reject(id) = msg {
            rejected = Some(id);
        }
    }
    if let Some(on_mode) = &widget.on_mode_change {
        let msg = (on_mode)(Commit::new(DiffDisplayMode::SideBySide));
        if let DiffMsg::Mode(m) = msg {
            switched_mode = Some(m);
        }
    }

    assert_eq!(accepted, Some("hunk-alpha"));
    assert_eq!(rejected, Some("hunk-alpha"));
    assert_eq!(switched_mode, Some(DiffDisplayMode::SideBySide));
}

#[test]
fn test_from_diff_viewer_into_element() {
    let hunk = DiffHunk::new(1, "@@ -1 +1 @@", vec![DiffLine::context(1, 1, "alpha")]);
    let widget = diff_viewer(DiffViewerModel::new(vec![hunk]));
    let _element: Element<'_, ()> = widget.into();
}

// ---------------------------------------------------------------------------
// Helpers for the widget-level contract
// ---------------------------------------------------------------------------

/// Message the widget-level tests watch, one variant per commit channel.
#[derive(Debug, Clone, PartialEq)]
enum DiffMsg {
    Accept(&'static str),
    Reject(&'static str),
    Mode(DiffDisplayMode),
}

/// Marker stashed in a child tree to see whether `diff` kept it or rebuilt it.
const SLOT_MARKER: u32 = 0xD1FF;

/// The viewport every widget-level test lays its viewer out in.
const OFFERED: Size = Size::new(400.0, 300.0);

/// A viewer with every callback wired, so a test can tell which channel fired.
fn wired(model: DiffViewerModel<&'static str>) -> DiffViewer<'static, DiffMsg, &'static str> {
    diff_viewer(model)
        .on_accept_hunk(|commit: Commit<&'static str>| DiffMsg::Accept(commit.into_value()))
        .on_reject_hunk(|commit: Commit<&'static str>| DiffMsg::Reject(commit.into_value()))
        .on_mode_change(|commit: Commit<DiffDisplayMode>| DiffMsg::Mode(commit.into_value()))
}

/// A hunk holding one replacement: a deleted line and its added replacement,
/// bracketed by context.
fn sample_hunk(id: &'static str) -> DiffHunk<&'static str> {
    DiffHunk::new(
        id,
        "@@ -10,2 +10,3 @@",
        vec![
            DiffLine::context(10, 10, "let mut x = 1;"),
            DiffLine::deleted(11, "x += 1;"),
            DiffLine::added(11, "x += 2;"),
            DiffLine::context(12, 12, "println!(\"{x}\");"),
        ],
    )
}

/// The number of lines [`sample_hunk`] carries.
const SAMPLE_LINES: usize = 4;

/// The height a unified body of `lines` diff lines occupies.
fn body_height(lines: usize) -> f32 {
    lines as f32 * LINE_HEIGHT
}

/// The action buttons a laid-out viewer publishes, in traversal order: per hunk
/// the mode toggle, then accept, then reject.
///
/// Line rows are [`LINE_HEIGHT`] (18px) and headers [`HEADER_HEIGHT`] (28px), so
/// the 24px height is unique to the action band — but the band is published
/// twice over per hunk, because iced 0.14 has every `Row` report its own bounds
/// through `Operation::container` as well: the row *holding* the buttons arrives
/// alongside them. A button is therefore a 24px box that no *other* 24px box sits
/// inside, and a lone button shares its row's bounds exactly, so one box reported
/// twice counts once.
fn action_boxes(published: &[Candidate]) -> Vec<Rectangle> {
    let mut band: Vec<Rectangle> = published
        .iter()
        .filter(|candidate| candidate.kind == KIND_CONTAINER)
        .map(|candidate| candidate.bounds)
        .filter(|bounds| bounds.height == BUTTON_HEIGHT)
        .collect();
    // The row is published immediately before the buttons it holds.
    band.dedup();

    band.iter()
        .copied()
        .filter(|outer| {
            !band
                .iter()
                .any(|inner| inner != outer && inner.is_within(outer))
        })
        .collect()
}

/// The two panes a side-by-side viewer publishes, in traversal order: old left,
/// new right. Each is the container that starts under the header, holds
/// `lines` diff lines, and is one half of the width the [`DIVIDER_WIDTH`] seam
/// leaves over.
fn side_by_side_panes(published: &[Candidate], offered_width: f32, lines: usize) -> Vec<Rectangle> {
    let pane_width = (offered_width - DIVIDER_WIDTH) / 2.0;

    published
        .iter()
        .filter(|candidate| candidate.kind == KIND_CONTAINER)
        .map(|candidate| candidate.bounds)
        .filter(|bounds| {
            bounds.y == HEADER_HEIGHT
                && bounds.width == pane_width
                && bounds.height == body_height(lines)
        })
        .collect()
}

/// Every box a laid-out widget publishes through the operation stream.
fn published_boxes<Message, Theme, Renderer: iced::advanced::Renderer>(
    widget: &mut dyn Widget<Message, Theme, Renderer>,
    tree: &mut Tree,
    layout: Layout<'_>,
    renderer: &Renderer,
) -> Vec<Candidate> {
    let mut probe = RecordCandidates::default();
    {
        let probe: &mut dyn Operation<Vec<Candidate>> = &mut probe;
        let mut erased = black_box(probe);
        widget.operate(tree, layout, renderer, &mut erased);
    }
    probe.candidates()
}

/// Move, press and release at `at`, keeping every message the viewer answered.
fn click<Message: Send + 'static>(
    harness: &mut Harness<'_, Message, iced::Renderer>,
    at: Point,
) -> Vec<Message> {
    let mut emitted = harness.hover(at);
    emitted.extend(harness.press(at));
    emitted.extend(harness.release(at));
    emitted
}

// ---------------------------------------------------------------------------
// The widget's own contract with the host
// ---------------------------------------------------------------------------

#[test]
fn test_the_viewer_asks_to_fill_the_width_and_shrink_to_its_hunks() {
    let widget = wired(DiffViewerModel::new(vec![sample_hunk("hunk-1")]));
    let requested = widget.size();

    assert_eq!(
        requested.width,
        Length::Fill,
        "a diff pane spans the width it is given (UIF-PRIN-004)"
    );
    assert_eq!(
        requested.height,
        Length::Shrink,
        "the viewer is as tall as its hunks and must not stretch to fill"
    );
    assert_eq!(
        widget.children().len(),
        1,
        "the composite wraps a single content element, so it publishes one child tree"
    );
}

#[test]
fn test_diff_rebuilds_a_missing_child_slot_and_keeps_an_unchanged_one() {
    let widget = wired(DiffViewerModel::new(vec![sample_hunk("hunk-1")]));

    let mut tree = Tree::empty();
    assert!(
        tree.children.is_empty(),
        "an empty tree carries no child slot to diff"
    );

    widget.diff(&mut tree);
    assert_eq!(
        tree.children.len(),
        1,
        "diffing against a tree with no children must create the content slot"
    );

    tree.children[0].state = TreeState::new(SLOT_MARKER);
    widget.diff(&mut tree);

    assert_eq!(
        tree.children.len(),
        1,
        "diffing an unchanged content element must neither add nor drop a slot"
    );
    // `State::downcast_ref` panics instead of reporting a missing state, so the
    // survival check is spelled out to keep the failure message meaningful.
    let survived: Option<&u32> = match &tree.children[0].state {
        TreeState::Some(state) => state.downcast_ref::<u32>(),
        TreeState::None => None,
    };
    assert_eq!(
        survived,
        Some(&SLOT_MARKER),
        "the content widget did not change, so its child state must survive the diff"
    );
}

#[test]
fn test_only_wired_callbacks_become_hunk_actions() {
    let mut harness =
        Harness::headless(|| diff_viewer(DiffViewerModel::new(vec![sample_hunk("hunk-1")])).into());
    assert!(
        action_boxes(&harness.candidates()).is_empty(),
        "a viewer with no callbacks must not offer buttons that answer nothing"
    );

    harness.set_view(|| {
        let model = DiffViewerModel::new(vec![sample_hunk("hunk-1")]);
        diff_viewer(model)
            .on_mode_change(|commit: Commit<DiffDisplayMode>| DiffMsg::Mode(commit.into_value()))
            .into()
    });
    assert_eq!(
        action_boxes(&harness.candidates()).len(),
        1,
        "only the mode toggle is wired, so only the mode toggle is clickable"
    );

    harness.set_view(|| wired(DiffViewerModel::new(vec![sample_hunk("hunk-1")])).into());
    assert_eq!(
        action_boxes(&harness.candidates()).len(),
        3,
        "a wired hunk gets a mode toggle, an accept and a reject action"
    );

    harness.set_view(|| {
        let two_hunks = DiffViewerModel::new(vec![sample_hunk("hunk-1"), sample_hunk("hunk-2")]);
        wired(two_hunks).into()
    });
    assert_eq!(
        action_boxes(&harness.candidates()).len(),
        6,
        "every hunk carries its own actions rather than sharing one set"
    );
}

#[test]
fn test_the_mode_button_commits_the_mode_it_switches_to() {
    let mut harness =
        Harness::headless(|| wired(DiffViewerModel::new(vec![sample_hunk("hunk-1")])).into());

    let actions = action_boxes(&harness.candidates());
    assert_eq!(
        actions.len(),
        3,
        "a wired hunk publishes its mode toggle, its accept and its reject action"
    );
    assert_eq!(
        click(&mut harness, centre(actions[0])),
        vec![DiffMsg::Mode(DiffDisplayMode::SideBySide)],
        "the leftmost action is the mode toggle, and a unified viewer commits the \
         mode it switches to, not the mode it is already in"
    );

    harness.set_view(|| {
        wired(
            DiffViewerModel::new(vec![sample_hunk("hunk-1")])
                .with_mode(DiffDisplayMode::SideBySide),
        )
        .into()
    });
    let actions = action_boxes(&harness.candidates());
    assert_eq!(
        click(&mut harness, centre(actions[0])),
        vec![DiffMsg::Mode(DiffDisplayMode::Unified)],
        "a side-by-side viewer offers to go back to unified"
    );
}

#[test]
fn test_accept_and_reject_commit_the_hunk_they_belong_to() {
    let mut harness = Harness::headless(|| {
        let two_hunks = DiffViewerModel::new(vec![sample_hunk("hunk-1"), sample_hunk("hunk-2")]);
        wired(two_hunks).into()
    });

    let actions = action_boxes(&harness.candidates());
    assert_eq!(
        actions.len(),
        6,
        "two wired hunks publish mode, accept and reject each, in hunk order"
    );

    assert_eq!(
        click(&mut harness, centre(actions[1])),
        vec![DiffMsg::Accept("hunk-1")],
        "the first accept button must commit the hunk it was built from"
    );
    assert_eq!(
        click(&mut harness, centre(actions[2])),
        vec![DiffMsg::Reject("hunk-1")],
        "the first reject button must commit the hunk it was built from"
    );
    assert_eq!(
        click(&mut harness, centre(actions[4])),
        vec![DiffMsg::Accept("hunk-2")],
        "the second hunk's accept button must not reuse the first hunk's id"
    );
    assert_eq!(
        click(&mut harness, centre(actions[5])),
        vec![DiffMsg::Reject("hunk-2")],
        "the second hunk's reject button must not reuse the first hunk's id"
    );
}

#[test]
fn test_side_by_side_splits_the_width_and_keeps_one_row_per_diff_line() {
    let renderer = headless_renderer();
    let limits = layout::Limits::new(Size::ZERO, OFFERED);

    let unified_model = DiffViewerModel::new(vec![sample_hunk("hunk-1")]);
    let unified_twin: Element<'_, DiffMsg> = wired(unified_model.clone()).into();
    let mut unified_tree = Tree::new(&unified_twin);
    let mut unified = wired(unified_model.clone());
    let unified_node = unified.layout(&mut unified_tree, &renderer, &limits);

    let paired_model = unified_model.clone().with_mode(DiffDisplayMode::SideBySide);
    let paired_twin: Element<'_, DiffMsg> = wired(paired_model.clone()).into();
    let mut paired_tree = Tree::new(&paired_twin);
    let mut paired = wired(paired_model);
    let paired_node = paired.layout(&mut paired_tree, &renderer, &limits);
    let paired_layout = Layout::new(&paired_node);
    let published = published_boxes(&mut paired, &mut paired_tree, paired_layout, &renderer);

    // The panes are recovered from the tree rather than hard-coded, so the
    // assertions keep holding when the header padding or the viewport changes.
    let panes = side_by_side_panes(&published, OFFERED.width, SAMPLE_LINES);
    assert_eq!(
        panes.len(),
        2,
        "side-by-side publishes an old pane and a new pane, each half the width the \
         divider leaves over and each tall enough for one row per diff line"
    );
    assert_eq!(
        panes[0].width, panes[1].width,
        "the two panes split the width 50/50"
    );
    assert_eq!(panes[0].x, 0.0, "the old side is the left pane");
    assert!(
        panes[1].x > panes[0].x + panes[0].width,
        "the new side sits right of the old side and its divider: old {:?}, new {:?}",
        panes[0],
        panes[1]
    );
    assert_eq!(
        panes[0].height,
        unified_node.size().height - HEADER_HEIGHT,
        "each pane carries one row per diff line — a deletion and its addition are \
         two rows, one per side, and are not folded onto a single shared row: \
         side-by-side is no shorter than unified"
    );

    let mut longer = sample_hunk("hunk-1");
    longer
        .lines
        .push(DiffLine::context(13, 13, "// trailing context"));
    let grown_model = DiffViewerModel::new(vec![longer]);
    let grown_twin: Element<'_, DiffMsg> = wired(grown_model.clone()).into();
    let mut grown_tree = Tree::new(&grown_twin);
    let mut grown = wired(grown_model);
    let grown_node = grown.layout(&mut grown_tree, &renderer, &limits);

    assert!(
        grown_node.size().height > unified_node.size().height,
        "the unified body is as tall as its lines, so an extra line must grow the pane"
    );
}

/// Side-by-side lays out at the height of its rows — the same height unified
/// occupies for the same hunk — and the center divider spans those rows rather
/// than the height the host happened to offer (`UIF-COMP-015`).
///
/// The divider used to be a `Length::Fill` child of a `Shrink` row, and iced
/// hands a `Fill` child inside a `Shrink` parent the whole height that parent
/// was offered: a 72px diff stretched to 272px of empty `surface.code`, and the
/// viewer *requested* `Length::Shrink` (`size()`, pinned by
/// `test_the_viewer_asks_to_fill_the_width_and_shrink_to_its_hunks`) while
/// *taking* everything it was offered. The side-by-side row now carries the
/// height of its own rows, so the divider spans the rows it separates.
#[test]
fn side_by_side_lays_out_at_the_height_of_its_rows_not_the_offer() {
    let renderer = headless_renderer();
    let limits = layout::Limits::new(Size::ZERO, OFFERED);

    let unified_model = DiffViewerModel::new(vec![sample_hunk("hunk-1")]);
    let unified_twin: Element<'_, DiffMsg> = wired(unified_model.clone()).into();
    let mut unified_tree = Tree::new(&unified_twin);
    let mut unified = wired(unified_model.clone());
    let unified_node = unified.layout(&mut unified_tree, &renderer, &limits);

    let paired_model = unified_model.clone().with_mode(DiffDisplayMode::SideBySide);
    let paired_twin: Element<'_, DiffMsg> = wired(paired_model.clone()).into();
    let mut paired_tree = Tree::new(&paired_twin);
    let mut paired = wired(paired_model);
    let paired_node = paired.layout(&mut paired_tree, &renderer, &limits);
    let paired_layout = Layout::new(&paired_node);
    let published = published_boxes(&mut paired, &mut paired_tree, paired_layout, &renderer);

    assert_eq!(
        unified_node.size().height,
        HEADER_HEIGHT + body_height(SAMPLE_LINES),
        "unified hugs its header and its rows"
    );
    assert_eq!(
        paired_node.size().height,
        HEADER_HEIGHT + body_height(SAMPLE_LINES),
        "side-by-side takes the height of its rows — the same height unified \
         occupies for the same hunk — not the height it was offered"
    );
    assert!(
        paired_node.size().height < OFFERED.height,
        "the viewer must leave the height it does not use alone: it took {} of \
         the {} it was offered",
        paired_node.size().height,
        OFFERED.height
    );

    let divider = published
        .iter()
        .map(|candidate| candidate.bounds)
        .find(|bounds| bounds.width == DIVIDER_WIDTH)
        .expect("side-by-side publishes its 1px center divider");
    assert_eq!(
        divider.height,
        body_height(SAMPLE_LINES),
        "the divider spans the rows it separates, not the offer"
    );

    let panes = side_by_side_panes(&published, OFFERED.width, SAMPLE_LINES);
    assert_eq!(
        panes.len(),
        2,
        "the rows themselves are still only tall enough for one row per diff line"
    );
    assert_eq!(
        divider.height, panes[0].height,
        "the divider is exactly as tall as the rows it is meant to separate: \
         divider {}, pane {}",
        divider.height, panes[0].height
    );
}

#[test]
fn test_the_viewer_asks_for_a_pointer_only_over_its_actions() {
    let renderer = headless_renderer();
    let model = DiffViewerModel::new(vec![sample_hunk("hunk-1")]);
    let twin: Element<'_, DiffMsg> = wired(model.clone()).into();
    let mut tree = Tree::new(&twin);
    let mut widget = wired(model);
    let limits = layout::Limits::new(Size::ZERO, Size::new(400.0, 300.0));
    let node = widget.layout(&mut tree, &renderer, &limits);
    let layout = Layout::new(&node);
    let viewport = Rectangle::with_size(node.size());

    let widget_dyn: &mut dyn Widget<DiffMsg, iced::Theme, iced::Renderer> = &mut widget;
    let actions = action_boxes(&published_boxes(widget_dyn, &mut tree, layout, &renderer));
    assert_eq!(
        actions.len(),
        3,
        "the laid-out viewer must publish its three actions"
    );

    assert_eq!(
        widget.mouse_interaction(
            &tree,
            layout,
            mouse::Cursor::Available(centre(actions[1])),
            &viewport,
            &renderer,
        ),
        mouse::Interaction::Pointer,
        "a live hunk action must invite the click"
    );

    assert_eq!(
        widget.mouse_interaction(
            &tree,
            layout,
            mouse::Cursor::Available(Point::new(200.0, node.size().height - 4.0)),
            &viewport,
            &renderer,
        ),
        mouse::Interaction::None,
        "diff text is not a control and must not ask for the pointer"
    );

    assert!(
        widget
            .overlay(&mut tree, layout, &renderer, &viewport, Vector::ZERO)
            .is_none(),
        "the viewer has no popover to hand the host"
    );
}

#[test]
fn test_an_empty_diff_paints_nothing_and_a_mode_switch_repaints() {
    let mut capture = Capture::new();
    let viewport = Size::new(400.0, 300.0);
    let pane = Rectangle::new(Point::ORIGIN, viewport);

    let empty = capture.frame(
        viewport,
        diff_viewer::<DiffMsg, &'static str>(DiffViewerModel::new(Vec::new())).into(),
    );
    assert_eq!(
        empty.ink_count(pane),
        0,
        "a diff with no hunks has nothing to show and must paint no ink at all"
    );

    let unified = capture.frame(
        viewport,
        wired(DiffViewerModel::new(vec![sample_hunk("hunk-1")])).into(),
    );
    assert!(
        unified.has_ink(pane),
        "a wired hunk paints its header, its actions and its lines"
    );

    let paired = capture.frame(
        viewport,
        wired(
            DiffViewerModel::new(vec![sample_hunk("hunk-1")])
                .with_mode(DiffDisplayMode::SideBySide),
        )
        .into(),
    );
    assert_repainted(
        &unified,
        &paired,
        pane,
        "switching to side-by-side splits the pane into two columns and moves every line",
    );
}
