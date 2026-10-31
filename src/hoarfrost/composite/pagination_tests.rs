//! Unit tests for `pagination` bar composite (`UIF-COMP-014`, `UIF-PRIN-003`).

use super::*;
use crate::test_helpers::{
    Candidate, Capture, Harness, RecordCandidates, centre, headless_renderer,
};
use iced::advanced::widget::operation::black_box;

#[test]
fn test_pagination_model_pages_and_ranges() {
    let empty = PaginationModel::new(0, 50, 0, vec![25, 50, 100]);
    assert_eq!(empty.total_pages(), 0);
    assert_eq!(empty.start_item(), 0);
    assert_eq!(empty.end_item(), 0);
    assert!(!empty.can_first());
    assert!(!empty.can_prev());
    assert!(!empty.can_next());
    assert!(!empty.can_last());
    assert_eq!(empty.range_summary(), "Showing 0 of 0");
    assert_eq!(empty.page_indicator(), "Page 0 of 0");

    let first_page = PaginationModel::new(0, 50, 1240, vec![25, 50, 100]);
    assert_eq!(first_page.total_pages(), 25);
    assert_eq!(first_page.start_item(), 1);
    assert_eq!(first_page.end_item(), 50);
    assert!(!first_page.can_first());
    assert!(!first_page.can_prev());
    assert!(first_page.can_next());
    assert!(first_page.can_last());
    assert_eq!(first_page.range_summary(), "Showing 1–50 of 1,240");
    assert_eq!(first_page.page_indicator(), "Page 1 of 25");

    let mid_page = PaginationModel::new(1, 50, 1240, vec![25, 50, 100]);
    assert_eq!(mid_page.start_item(), 51);
    assert_eq!(mid_page.end_item(), 100);
    assert!(mid_page.can_first());
    assert!(mid_page.can_prev());
    assert!(mid_page.can_next());
    assert!(mid_page.can_last());
    assert_eq!(mid_page.range_summary(), "Showing 51–100 of 1,240");
    assert_eq!(mid_page.page_indicator(), "Page 2 of 25");

    let last_page = PaginationModel::new(24, 50, 1240, vec![25, 50, 100]);
    assert_eq!(last_page.start_item(), 1201);
    assert_eq!(last_page.end_item(), 1240);
    assert!(last_page.can_first());
    assert!(last_page.can_prev());
    assert!(!last_page.can_next());
    assert!(!last_page.can_last());
    assert_eq!(last_page.range_summary(), "Showing 1,201–1,240 of 1,240");
    assert_eq!(last_page.page_indicator(), "Page 25 of 25");
}

#[test]
fn test_format_thousands() {
    assert_eq!(format_thousands(0), "0");
    assert_eq!(format_thousands(9), "9");
    assert_eq!(format_thousands(99), "99");
    assert_eq!(format_thousands(999), "999");
    assert_eq!(format_thousands(1000), "1,000");
    assert_eq!(format_thousands(1240), "1,240");
    assert_eq!(format_thousands(10000), "10,000");
    assert_eq!(format_thousands(100000), "100,000");
    assert_eq!(format_thousands(1000000), "1,000,000");
    assert_eq!(format_thousands(12345678), "12,345,678");
}

#[test]
fn test_tokens_and_geometry() {
    assert_eq!(
        BAR_HEIGHT, 32.0,
        "bar height must be 32px (size.control.md) per §3B.14"
    );
    assert_eq!(
        BORDER_WIDTH, 1.0,
        "border width must be 1px hairline per §3B.14"
    );
    assert_eq!(
        BUTTON_SIZE, 28.0,
        "navigation button size must be 28x28px (size.control.sm) per §3B.14"
    );
    assert_eq!(
        PADDING_X, 12.0,
        "horizontal padding must be 12px (space.md) per §3B.14"
    );
    assert_eq!(
        NAV_SPACING, 8.0,
        "navigation spacing must be 8px (space.sm) per §3B.14"
    );
    assert_eq!(
        OPTION_SPACING, 4.0,
        "option spacing must be 4px (space.xs) per §3B.14"
    );
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum TestMsg {
    Page(usize),
    PageSize(usize),
}

#[test]
fn test_widget_creation_and_element_conversion() {
    let model = PaginationModel::new(0, 50, 100, vec![25, 50, 100]);

    let widget: PaginationBar<'_, TestMsg> = PaginationBar::new(
        model.clone(),
        |commit| TestMsg::Page(commit.into_value()),
        |commit| TestMsg::PageSize(commit.into_value()),
    );
    assert_eq!(widget.model(), &model);

    let _element: Element<'_, TestMsg> = pagination_bar(
        model,
        |commit| TestMsg::Page(commit.into_value()),
        |commit| TestMsg::PageSize(commit.into_value()),
    );
}

// ---------------------------------------------------------------------------
// Helpers for the widget-level contract
// ---------------------------------------------------------------------------

/// Commit the page a navigation button targets.
fn page_commit(commit: Commit<usize>) -> TestMsg {
    TestMsg::Page(commit.into_value())
}

/// Commit the page size an option button selects.
fn size_commit(commit: Commit<usize>) -> TestMsg {
    TestMsg::PageSize(commit.into_value())
}

/// A bar over three pages, so every navigation control is reachable.
fn three_pages(page: usize) -> PaginationModel {
    PaginationModel::new(page, 50, 120, vec![25, 50, 100])
}

/// Whether `outer` fully covers `inner` while being wider than it.
fn encloses(outer: Rectangle, inner: Rectangle) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.width <= outer.x + outer.width
        && inner.y + inner.height <= outer.y + outer.height
        && inner.width < outer.width
}

/// The boxes of the controls themselves.
///
/// Rows and containers publish the same `container` hook as the buttons they
/// wrap, so a test that clicks every published box would click a wrapper and
/// land on whichever button happened to sit under it. A control is a published
/// box that no *other* published box sits inside.
fn control_boxes(published: &[Candidate]) -> Vec<Rectangle> {
    let mut boxes: Vec<Rectangle> = Vec::new();
    for candidate in published {
        if !boxes.contains(&candidate.bounds) {
            boxes.push(candidate.bounds);
        }
    }

    boxes
        .clone()
        .into_iter()
        .filter(|outer| {
            !boxes
                .iter()
                .any(|inner| inner != outer && encloses(*outer, *inner))
        })
        .collect()
}

/// The four navigation slots — `[⏮] [◀] [▶] [⏭]` — in traversal order.
///
/// Height alone separates them from everything else the bar publishes: the
/// range summary and the page indicator are caption-height text, and a page size
/// option's own box is `BUTTON_SIZE` tall while the box [`control_boxes`] keeps
/// for it is the label one `space::XXS` shorter.
fn nav_slots(published: &[Candidate]) -> Vec<Rectangle> {
    control_boxes(published)
        .into_iter()
        .filter(|bounds| bounds.height == BUTTON_SIZE)
        .collect()
}

/// Press and release a box's centre, keeping every message the bar answered.
fn click<Message: Send + 'static>(
    harness: &mut Harness<'_, Message, iced::Renderer>,
    bounds: Rectangle,
) -> Vec<Message> {
    let at = centre(bounds);
    let mut emitted = harness.hover(at);
    emitted.extend(harness.press(at));
    emitted.extend(harness.release(at));
    emitted
}

/// Every box a laid-out widget publishes through the operation stream.
fn published_boxes<Message, Theme, Renderer: renderer::Renderer>(
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

// ---------------------------------------------------------------------------
// The wide leg
// ---------------------------------------------------------------------------
//
// The bar needs ~443px to lay out everything §3B.14 asks for at once, and the
// harness's viewport is 400px wide, so a harness-built bar is already squeezing
// its navigation group (see
// `defect_the_trailing_navigation_slots_collapse_when_the_bar_is_narrow`). The
// behavioural legs below therefore lay the bar out, and click it, at a width
// where its own geometry contract holds; the collapse is pinned separately.

/// The width the behavioural legs lay the bar out and click it at.
const WIDE: Size = Size::new(900.0, 60.0);

/// The navigation slots a bar publishes when it is laid out `width` wide.
fn nav_slots_at(model: &PaginationModel, renderer: &iced::Renderer, width: f32) -> Vec<Rectangle> {
    let twin: Element<'_, TestMsg> = pagination_bar(model.clone(), page_commit, size_commit);
    let mut tree = Tree::new(&twin);
    let mut widget = PaginationBar::new(model.clone(), page_commit, size_commit);
    let limits = layout::Limits::new(Size::ZERO, Size::new(width, BAR_HEIGHT));
    let node = widget.layout(&mut tree, renderer, &limits);
    let layout = Layout::new(&node);
    let widget_dyn: &mut dyn Widget<TestMsg, iced::Theme, iced::Renderer> = &mut widget;

    nav_slots(&published_boxes(widget_dyn, &mut tree, layout, renderer))
}

/// Press and release a navigation slot on a bar over `page`, at [`WIDE`],
/// keeping every message the bar answered.
fn click_wide(capture: &mut Capture, page: usize, bounds: Rectangle) -> Vec<TestMsg> {
    capture
        .click_and_frame(
            WIDE,
            pagination_bar(three_pages(page), page_commit, size_commit),
            centre(bounds),
        )
        .1
}

// ---------------------------------------------------------------------------
// Model boundaries the bar has to survive
// ---------------------------------------------------------------------------

#[test]
fn test_a_page_size_of_zero_offers_no_navigation() {
    let model = PaginationModel::new(0, 0, 100, vec![25, 50, 100]);

    assert_eq!(
        model.total_pages(),
        0,
        "a zero page size cannot divide the item count into pages"
    );
    assert!(
        !model.can_next(),
        "with no pages there is no next page to step to"
    );
    assert!(
        !model.can_last(),
        "with no pages there is no last page to jump to"
    );
    assert_eq!(
        model.page_indicator(),
        "Page 0 of 0",
        "the indicator reports an empty list rather than a phantom first page"
    );
}

#[test]
fn test_a_page_past_the_last_one_is_reported_but_clamped() {
    let stale = PaginationModel::new(5, 50, 100, vec![25, 50, 100]);

    assert_eq!(stale.total_pages(), 2);
    assert_eq!(
        stale.end_item(),
        100,
        "the range end never runs past the number of items on hand"
    );
    assert!(
        !stale.can_next(),
        "a page already past the last one cannot step further"
    );
    assert!(
        !stale.can_last(),
        "a page already past the last one has no last page to jump to"
    );
    assert!(
        stale.can_prev(),
        "a stale page can still step back toward the list"
    );
    assert_eq!(
        stale.page_indicator(),
        "Page 6 of 2",
        "the indicator shows the page the host asked for, not a clamped one"
    );
}

#[test]
fn test_an_empty_total_disables_the_back_buttons_even_on_a_later_page() {
    let orphan = PaginationModel::new(2, 50, 0, vec![25, 50, 100]);

    assert!(
        !orphan.can_first(),
        "there is no first page to jump to when nothing is loaded"
    );
    assert!(
        !orphan.can_prev(),
        "there is no previous page to step to when nothing is loaded"
    );
    assert_eq!(
        orphan.range_summary(),
        "Showing 0 of 0",
        "an empty total reads as an empty range regardless of the page index"
    );
}

// ---------------------------------------------------------------------------
// The bar as a real control surface
// ---------------------------------------------------------------------------

#[test]
fn test_the_navigation_group_does_not_move_between_the_first_and_last_page() {
    let mut harness =
        Harness::headless(move || pagination_bar(three_pages(0), page_commit, size_commit));

    let first_page = nav_slots(&harness.candidates());
    assert_eq!(
        first_page.len(),
        4,
        "the first page must still publish [⏮] and [◀] rather than drop them (UIF-PRIN-003)"
    );
    harness.render();

    harness.set_view(move || pagination_bar(three_pages(2), page_commit, size_commit));
    let last_page = nav_slots(&harness.candidates());
    assert_eq!(
        first_page, last_page,
        "every navigation slot must sit in exactly the same box on the last page as \
         on the first: a boundary page changes which buttons are live, never where \
         they are (UIF-PRIN-003)"
    );
    assert_eq!(
        first_page[3].x + first_page[3].width,
        400.0 - PADDING_X,
        "the navigation group is right-aligned against the bar's own padding"
    );
    harness.render();
}

#[test]
fn defect_the_trailing_navigation_slots_collapse_when_the_bar_is_narrow() {
    // DEFECT. §3B.14 and this module's own header pin the navigation buttons as
    // "each 28x28px" and say boundary buttons "render as Disabled rather than
    // disappearing". They do that at every page index — but the *bar* makes two
    // of them disappear at an ordinary footer width.
    //
    // `PaginationBar::new` puts the range summary in a `Length::Fill` column and
    // the navigation group in a second `Length::Fill` column, so each takes half
    // the width the page-size options leave over however little it actually
    // needs. In the harness's 400px viewport the right column gets 138.4px while
    // the group needs 221.3px, and iced's row layout hands the shortfall to the
    // children it reaches last: `[▶ Next]` is squeezed to a sliver and `[⏭ Last]`
    // to nothing at all. They are the two *live* buttons on page 0, so this is
    // not the disabled state hiding them — the bar never reserves the group's
    // natural width. The right column should shrink-wrap the group (or the row
    // should carry a minimum width) so the range summary is what gives way.
    let mut harness =
        Harness::headless(move || pagination_bar(three_pages(0), page_commit, size_commit));

    let slots = nav_slots(&harness.candidates());
    assert_eq!(
        slots.len(),
        4,
        "all four slots stay published at a narrow width — it is their boxes that go"
    );
    assert_eq!(
        (slots[0].width, slots[1].width),
        (BUTTON_SIZE, BUTTON_SIZE),
        "[⏮ First] and [◀ Prev] keep their 28x28 hit target"
    );
    assert!(
        slots[2].width < BUTTON_SIZE,
        "actual: [▶ Next] is squeezed to {}px, below its {}px target — the group \
         overflowed its column and iced trimmed the trailing children",
        slots[2].width,
        BUTTON_SIZE
    );
    assert!(
        slots[3].width < BUTTON_SIZE,
        "actual: [⏭ Last] is squeezed to {}px — at that width it cannot be seen, \
         let alone clicked, whatever §3B.14 promises",
        slots[3].width
    );

    // The same bar with room to spare keeps all four boxes, which is what pins
    // the collapse to the bar's own width allocation rather than to the model.
    let capture = Capture::new();
    let wide = nav_slots_at(&three_pages(0), capture.renderer(), WIDE.width);
    assert_eq!(
        wide.iter().map(|bounds| bounds.width).collect::<Vec<_>>(),
        vec![BUTTON_SIZE; 4],
        "the four slots are each 28px wide once the bar is given the width the \
         group needs, so the squeeze above is the allocation, not the state"
    );
}

#[test]
fn test_the_navigation_buttons_commit_their_target_pages() {
    let mut capture = Capture::new();
    let nav = nav_slots_at(&three_pages(1), capture.renderer(), WIDE.width);
    assert_eq!(nav.len(), 4, "a middle page offers all four moves");

    assert_eq!(
        click_wide(&mut capture, 1, nav[0]),
        vec![TestMsg::Page(0)],
        "[⏮ First] commits page 0"
    );
    assert_eq!(
        click_wide(&mut capture, 1, nav[1]),
        vec![TestMsg::Page(0)],
        "[◀ Prev] commits the page before the current one"
    );
    assert_eq!(
        click_wide(&mut capture, 1, nav[2]),
        vec![TestMsg::Page(2)],
        "[▶ Next] commits the page after the current one"
    );
    assert_eq!(
        click_wide(&mut capture, 1, nav[3]),
        vec![TestMsg::Page(2)],
        "[⏭ Last] commits the final page index"
    );
}

#[test]
fn test_the_disabled_boundary_buttons_answer_nothing_on_the_first_page() {
    let mut capture = Capture::new();
    let nav = nav_slots_at(&three_pages(0), capture.renderer(), WIDE.width);
    assert_eq!(nav.len(), 4, "page 0 still offers all four slots");

    assert_eq!(
        click_wide(&mut capture, 0, nav[0]),
        Vec::<TestMsg>::new(),
        "[⏮ First] is disabled on page 0 and must commit nothing"
    );
    assert_eq!(
        click_wide(&mut capture, 0, nav[1]),
        Vec::<TestMsg>::new(),
        "[◀ Prev] is disabled on page 0 and must commit nothing"
    );
    assert_eq!(
        click_wide(&mut capture, 0, nav[2]),
        vec![TestMsg::Page(1)],
        "[▶ Next] stays live on page 0"
    );
    assert_eq!(
        click_wide(&mut capture, 0, nav[3]),
        vec![TestMsg::Page(2)],
        "[⏭ Last] stays live on page 0"
    );
}

#[test]
fn test_the_disabled_boundary_buttons_answer_nothing_on_the_last_page() {
    let mut capture = Capture::new();
    let nav = nav_slots_at(&three_pages(2), capture.renderer(), WIDE.width);
    assert_eq!(nav.len(), 4, "the last page still offers all four slots");

    assert_eq!(
        click_wide(&mut capture, 2, nav[0]),
        vec![TestMsg::Page(0)],
        "[⏮ First] stays live on the last page"
    );
    assert_eq!(
        click_wide(&mut capture, 2, nav[1]),
        vec![TestMsg::Page(1)],
        "[◀ Prev] steps back from the last page"
    );
    assert_eq!(
        click_wide(&mut capture, 2, nav[2]),
        Vec::<TestMsg>::new(),
        "[▶ Next] is disabled on the last page and must commit nothing"
    );
    assert_eq!(
        click_wide(&mut capture, 2, nav[3]),
        Vec::<TestMsg>::new(),
        "[⏭ Last] is disabled on the last page and must commit nothing"
    );
}

#[test]
fn test_each_page_size_option_commits_its_own_size() {
    let mut harness =
        Harness::headless(move || pagination_bar(three_pages(0), page_commit, size_commit));

    // Every box the bar publishes is pressed, in the order it publishes them. The
    // options are pressed through their labels: a button's own box is `BUTTON_SIZE`
    // tall and its label sits strictly inside it, so [`control_boxes`] keeps the
    // label and a press there still lands on the button that wraps it.
    let mut committed: Vec<usize> = Vec::new();
    for bounds in control_boxes(&harness.candidates()) {
        for message in click(&mut harness, bounds) {
            if let TestMsg::PageSize(size) = message {
                committed.push(size);
            }
        }
    }

    assert_eq!(
        committed,
        vec![25, 50, 100],
        "each page size option must commit the size it is labelled with, in the order \
         the bar publishes them, and no other control may commit a size — the middle \
         option is the selected one, so re-selecting it has to stay a valid commit"
    );
}

// ---------------------------------------------------------------------------
// Delegation
// ---------------------------------------------------------------------------

#[test]
fn test_the_bar_reports_its_fixed_height_and_one_child_slot() {
    let widget: PaginationBar<'_, TestMsg> =
        PaginationBar::new(three_pages(0), page_commit, size_commit);

    let requested = widget.size();
    assert_eq!(
        requested.width,
        Length::Fill,
        "the footer spans the width it is given (UIF-PRIN-004)"
    );
    assert_eq!(
        requested.height,
        Length::Fixed(BAR_HEIGHT),
        "the footer is exactly 32px tall (UIF-COMP-014)"
    );
    assert_eq!(
        widget.children().len(),
        1,
        "the composite wraps a single content element, so it publishes one child tree"
    );
}

#[test]
fn test_diff_creates_the_content_slot_a_fresh_tree_lacks() {
    let widget: PaginationBar<'_, TestMsg> =
        PaginationBar::new(three_pages(0), page_commit, size_commit);

    let mut tree = Tree::empty();
    widget.diff(&mut tree);

    assert_eq!(
        tree.children.len(),
        1,
        "diffing against a tree with no children must create the content slot"
    );
}

#[test]
fn test_the_bar_asks_for_a_pointer_only_over_its_buttons_and_offers_no_overlay() {
    let model = three_pages(0);
    let twin: Element<'_, TestMsg> = pagination_bar(model.clone(), page_commit, size_commit);
    let mut tree = Tree::new(&twin);

    let mut widget = PaginationBar::new(model, page_commit, size_commit);
    let renderer = headless_renderer();
    // Laid out at [`WIDE`], not at the harness's 400px: a squeezed `[⏭ Last]` is 0px
    // wide, so a cursor "over" it would be over nothing and the cursor read-back
    // below would be measuring the collapse rather than the cursor.
    let limits = layout::Limits::new(Size::ZERO, WIDE);
    let node = widget.layout(&mut tree, &renderer, &limits);
    let layout = Layout::new(&node);
    let viewport = Rectangle::with_size(node.size());

    let widget_dyn: &mut dyn Widget<TestMsg, iced::Theme, iced::Renderer> = &mut widget;
    let nav: Vec<Rectangle> =
        control_boxes(&published_boxes(widget_dyn, &mut tree, layout, &renderer))
            .into_iter()
            .filter(|bounds| bounds.width == BUTTON_SIZE && bounds.height == BUTTON_SIZE)
            .collect();

    assert_eq!(
        nav.len(),
        4,
        "the laid-out bar publishes four full-size navigation boxes"
    );

    assert_eq!(
        widget.mouse_interaction(
            &tree,
            layout,
            mouse::Cursor::Available(centre(nav[2])),
            &viewport,
            &renderer,
        ),
        mouse::Interaction::Pointer,
        "a live navigation button must ask for the pointer cursor"
    );

    assert_eq!(
        widget.mouse_interaction(
            &tree,
            layout,
            mouse::Cursor::Available(centre(nav[0])),
            &viewport,
            &renderer,
        ),
        mouse::Interaction::None,
        "a disabled navigation button must not invite a click"
    );

    assert!(
        widget
            .overlay(&mut tree, layout, &renderer, &viewport, Vector::ZERO)
            .is_none(),
        "the footer has no popover to hand the host"
    );
}
