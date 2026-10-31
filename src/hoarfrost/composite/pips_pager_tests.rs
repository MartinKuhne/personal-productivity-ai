//! Unit tests for `pips_pager` composite (`UIF-COMP-011`, `UIF-PRIN-004`).

use super::*;
use crate::test_helpers::Harness;
use iced::Point;

#[test]
fn test_pips_pager_model_navigation_logic() {
    let empty = PipsPagerModel::new(0, 0);
    assert!(!empty.can_prev());
    assert!(!empty.can_next());
    assert!(empty.is_first_page());
    assert!(empty.is_last_page());

    let single_page = PipsPagerModel::new(0, 1);
    assert!(!single_page.can_prev());
    assert!(!single_page.can_next());
    assert!(single_page.is_first_page());
    assert!(single_page.is_last_page());

    let first_of_three = PipsPagerModel::new(0, 3);
    assert!(!first_of_three.can_prev());
    assert!(first_of_three.can_next());
    assert!(first_of_three.is_first_page());
    assert!(!first_of_three.is_last_page());

    let middle_of_three = PipsPagerModel::new(1, 3);
    assert!(middle_of_three.can_prev());
    assert!(middle_of_three.can_next());
    assert!(!middle_of_three.is_first_page());
    assert!(!middle_of_three.is_last_page());

    let last_of_three = PipsPagerModel::new(2, 3);
    assert!(last_of_three.can_prev());
    assert!(!last_of_three.can_next());
    assert!(!last_of_three.is_first_page());
    assert!(last_of_three.is_last_page());
}

#[test]
fn test_tokens_and_geometry() {
    assert_eq!(
        BAR_HEIGHT, 24.0,
        "bar height must be 24px (size.bar.status) per §3B.11"
    );
    assert_eq!(
        BUTTON_SIZE, 24.0,
        "navigation button dimensions must be 24x24px (size.hit_target.min) per §3B.11"
    );
    assert_eq!(
        PIP_SIZE_INACTIVE, 6.0,
        "inactive pip size must be 6px (size.indicator.dot) per §3B.11"
    );
    assert_eq!(
        PIP_WIDTH_ACTIVE, 16.0,
        "active pip width must be 16px (space.lg) per §3B.11"
    );
    assert_eq!(
        PIP_HEIGHT, 6.0,
        "pip height must be 6px (size.indicator.dot) per §3B.11"
    );
    assert_eq!(PIP_GAP, 4.0, "pip gap must be 4px (space.xs) per §3B.11");
    assert_eq!(
        PIP_RADIUS, 999.0,
        "pip radius must be radius.pill per §3B.11"
    );
}

#[test]
fn test_glyphs() {
    assert_eq!(GLYPH_PREV, "◀");
    assert_eq!(GLYPH_NEXT, "▶");
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum PipsMsg {
    Change(usize),
}

#[test]
fn test_widget_creation_and_element_conversion() {
    let model = PipsPagerModel::new(1, 5);

    let widget: PipsPager<'_, PipsMsg> = PipsPager::new(model, |c| PipsMsg::Change(c.into_value()));
    assert_eq!(widget.model(), &model);

    let _element: Element<'_, PipsMsg> = pips_pager(model, |c| PipsMsg::Change(c.into_value()));
}

#[test]
fn test_boundary_pages_do_not_collapse_buttons() {
    // Page 0: [◀] is disabled, but renders at full 24x24px maintaining 0px shift per UIF-PRIN-004
    let page_zero = PipsPagerModel::new(0, 4);
    assert!(!page_zero.can_prev());
    assert!(page_zero.can_next());
    let _first_page_element: Element<'_, PipsMsg> =
        pips_pager(page_zero, |c| PipsMsg::Change(c.into_value()));

    // Last page: [▶] is disabled, but renders at full 24x24px
    let page_last = PipsPagerModel::new(3, 4);
    assert!(page_last.can_prev());
    assert!(!page_last.can_next());
    let _last_page_element: Element<'_, PipsMsg> =
        pips_pager(page_last, |c| PipsMsg::Change(c.into_value()));
}

// ---------------------------------------------------------------------------
// Model boundaries the strip has to survive
// ---------------------------------------------------------------------------

#[test]
fn test_a_page_number_beyond_a_zero_page_total_offers_no_navigation() {
    let orphan = PipsPagerModel::new(3, 0);
    assert!(
        !orphan.can_prev(),
        "with no pages at all there is no previous page to reach"
    );
    assert!(
        !orphan.can_next(),
        "with no pages at all there is no next page to reach"
    );
    assert!(
        !orphan.is_first_page(),
        "page 3 is not the first page even when the total is empty"
    );
    assert!(
        orphan.is_last_page(),
        "an empty total is treated as being at the end"
    );
}

#[test]
fn test_a_page_index_past_the_total_still_offers_a_way_back() {
    let overrun = PipsPagerModel::new(5, 3);
    assert!(
        overrun.can_prev(),
        "an out-of-range page can still step back towards the list"
    );
    assert!(
        !overrun.can_next(),
        "an out-of-range page cannot step further forward"
    );
    assert!(
        overrun.is_last_page(),
        "an index at or past the total reports being on the last page"
    );
}

// ---------------------------------------------------------------------------
// Harness helpers
// ---------------------------------------------------------------------------

/// The distinct boxes the operation stream publishes at exactly `width` x
/// `height`, in traversal order.
///
/// The pager paints no quads of its own — every layer belongs to an iced
/// `button` or `container` — so the operation stream is where its hit
/// targets can be read back from.
fn boxes_of<Message: Send + 'static, Renderer: renderer::Renderer>(
    harness: &mut Harness<'_, Message, Renderer>,
    width: f32,
    height: f32,
) -> Vec<Rectangle> {
    let mut seen: Vec<Rectangle> = Vec::new();

    for candidate in harness.candidates() {
        let bounds = candidate.bounds;
        if candidate.kind != crate::test_helpers::KIND_CONTAINER {
            continue;
        }
        if (bounds.width - width).abs() > 0.5 || (bounds.height - height).abs() > 0.5 {
            continue;
        }
        if !seen
            .iter()
            .any(|seen| seen.x == bounds.x && seen.y == bounds.y)
        {
            seen.push(bounds);
        }
    }

    seen
}

fn pager(model: PipsPagerModel) -> Element<'static, PipsMsg> {
    pips_pager(model, |commit| PipsMsg::Change(commit.into_value()))
}

/// Press and release over `at`, returning everything the gesture published.
fn click<Message: Send + 'static, Renderer: renderer::Renderer>(
    harness: &mut Harness<'_, Message, Renderer>,
    at: iced::Point,
) -> Vec<Message> {
    harness.press(at);
    harness.release(at)
}

// ---------------------------------------------------------------------------
// Hit targets and the messages they commit
// ---------------------------------------------------------------------------

#[test]
fn test_both_navigation_buttons_keep_their_full_slot_on_a_middle_page() {
    let mut harness = Harness::<PipsMsg, _>::headless(move || pager(PipsPagerModel::new(1, 3)));

    let buttons = boxes_of(&mut harness, BUTTON_SIZE, BUTTON_SIZE);
    assert_eq!(
        buttons.len(),
        2,
        "both [◀] and [▶] are present and each occupies a full 24x24 hit target"
    );
    assert!(
        buttons[0].x < buttons[1].x,
        "[◀] is laid out to the left of [▶]"
    );
}

#[test]
fn test_the_next_button_commits_the_following_page() {
    let mut harness = Harness::<PipsMsg, _>::headless(move || pager(PipsPagerModel::new(1, 3)));

    let buttons = boxes_of(&mut harness, BUTTON_SIZE, BUTTON_SIZE);
    assert_eq!(buttons.len(), 2);
    let next = buttons[1].center();

    assert_eq!(
        click(&mut harness, next),
        vec![PipsMsg::Change(2)],
        "[▶] commits page + 1"
    );
}

#[test]
fn test_the_previous_button_commits_the_preceding_page() {
    let mut harness = Harness::<PipsMsg, _>::headless(move || pager(PipsPagerModel::new(2, 4)));

    let buttons = boxes_of(&mut harness, BUTTON_SIZE, BUTTON_SIZE);
    assert_eq!(buttons.len(), 2);
    let prev = buttons[0].center();

    assert_eq!(
        click(&mut harness, prev),
        vec![PipsMsg::Change(1)],
        "[◀] commits page - 1"
    );
}

#[test]
fn test_a_disabled_boundary_button_keeps_its_box_and_commits_nothing() {
    // UIF-PRIN-004: the button is disabled, it does not disappear.
    let mut harness = Harness::<PipsMsg, _>::headless(move || pager(PipsPagerModel::new(0, 3)));

    let buttons = boxes_of(&mut harness, BUTTON_SIZE, BUTTON_SIZE);
    assert_eq!(
        buttons.len(),
        2,
        "on page 0 [◀] still occupies its 24x24 slot so the strip does not shift"
    );

    assert_eq!(
        click(&mut harness, buttons[0].center()),
        Vec::<PipsMsg>::new(),
        "a disabled [◀] must not commit a page"
    );
    assert_eq!(
        click(&mut harness, buttons[1].center()),
        vec![PipsMsg::Change(1)],
        "the enabled half of the strip keeps working beside the disabled one"
    );
}

#[test]
fn test_the_disabled_next_button_on_the_last_page_still_holds_its_slot() {
    let mut harness = Harness::<PipsMsg, _>::headless(move || pager(PipsPagerModel::new(3, 4)));

    let buttons = boxes_of(&mut harness, BUTTON_SIZE, BUTTON_SIZE);
    assert_eq!(
        buttons.len(),
        2,
        "[▶] does not collapse away on the last page"
    );

    assert_eq!(
        click(&mut harness, buttons[1].center()),
        Vec::<PipsMsg>::new(),
        "a disabled [▶] must not commit a page past the end"
    );
    assert_eq!(
        click(&mut harness, buttons[0].center()),
        vec![PipsMsg::Change(2)],
        "[◀] still steps back from the last page"
    );
}

#[test]
fn test_an_inactive_pip_jumps_straight_to_its_own_page() {
    let mut harness = Harness::<PipsMsg, _>::headless(move || pager(PipsPagerModel::new(1, 3)));

    let pips = boxes_of(&mut harness, PIP_SIZE_INACTIVE, PIP_HEIGHT);
    assert_eq!(
        pips.len(),
        2,
        "the two inactive pips are clickable; the active one is not"
    );

    assert_eq!(
        click(&mut harness, pips[0].center()),
        vec![PipsMsg::Change(0)],
        "the pip left of the active one commits page 0"
    );
    assert_eq!(
        click(&mut harness, pips[1].center()),
        vec![PipsMsg::Change(2)],
        "the pip right of the active one commits page 2"
    );
}

#[test]
fn test_the_active_pip_is_not_a_button_and_a_single_page_strip_has_no_pip_buttons() {
    let mut middle = Harness::<PipsMsg, _>::headless(move || pager(PipsPagerModel::new(1, 3)));
    let active_wide = boxes_of(&mut middle, PIP_WIDTH_ACTIVE, PIP_HEIGHT);
    assert_eq!(
        active_wide.len(),
        1,
        "exactly one pip is drawn at the 16px active width"
    );
    assert_eq!(
        click(&mut middle, active_wide[0].center()),
        Vec::<PipsMsg>::new(),
        "the active pip is inert: it is where you already are"
    );

    let mut single = Harness::<PipsMsg, _>::headless(move || pager(PipsPagerModel::new(0, 1)));
    assert_eq!(
        boxes_of(&mut single, PIP_SIZE_INACTIVE, PIP_HEIGHT).len(),
        0,
        "a one-page strip has no inactive pip to click"
    );
    let nav_buttons = boxes_of(&mut single, BUTTON_SIZE, BUTTON_SIZE);
    assert_eq!(
        click(&mut single, nav_buttons[0].center()),
        Vec::<PipsMsg>::new(),
        "a one-page strip cannot navigate in either direction"
    );
}

#[test]
fn test_a_page_index_past_the_total_draws_every_pip_as_a_button() {
    // No pip matches the page, so none of them is the active 16px one.
    let mut harness = Harness::<PipsMsg, _>::headless(move || pager(PipsPagerModel::new(5, 3)));

    assert_eq!(
        boxes_of(&mut harness, PIP_WIDTH_ACTIVE, PIP_HEIGHT).len(),
        0,
        "an out-of-range page highlights no pip at all"
    );
    let pips = boxes_of(&mut harness, PIP_SIZE_INACTIVE, PIP_HEIGHT);
    assert_eq!(
        pips.len(),
        3,
        "every pip stays clickable when none of them is the current page"
    );
    assert_eq!(
        click(&mut harness, pips[2].center()),
        vec![PipsMsg::Change(2)],
        "the last pip still commits its own page"
    );
}

#[test]
fn test_a_release_outside_the_button_commits_nothing() {
    let mut harness = Harness::<PipsMsg, _>::headless(move || pager(PipsPagerModel::new(1, 3)));

    let buttons = boxes_of(&mut harness, BUTTON_SIZE, BUTTON_SIZE);
    let next = buttons[1].center();
    harness.press(next);

    assert_eq!(
        harness.release(iced::Point::new(next.x, next.y + 200.0)),
        Vec::<PipsMsg>::new(),
        "dragging off the button before releasing abandons the page change"
    );
}

// ---------------------------------------------------------------------------
// The widget's own delegation to its single content child
// ---------------------------------------------------------------------------

#[test]
fn test_the_pager_reports_a_fixed_strip_height() {
    let widget: PipsPager<'_, PipsMsg> = PipsPager::new(PipsPagerModel::new(1, 3), |c| {
        PipsMsg::Change(c.into_value())
    });

    assert_eq!(
        Widget::<PipsMsg, iced::Theme, iced::Renderer>::size(&widget).height,
        Length::Fixed(BAR_HEIGHT),
        "the strip is pinned to its 24px height regardless of page count"
    );
    assert_eq!(widget.model(), &PipsPagerModel::new(1, 3));
}

#[test]
fn test_children_and_diff_delegate_to_the_one_content_tree() {
    let mut widget: PipsPager<'_, PipsMsg> = PipsPager::new(PipsPagerModel::new(1, 3), |c| {
        PipsMsg::Change(c.into_value())
    });
    let mut tree = Tree::new(&widget as &dyn Widget<PipsMsg, iced::Theme, iced::Renderer>);
    assert_eq!(
        tree.children.len(),
        1,
        "the pager wraps exactly one content child"
    );

    widget.diff(&mut tree);
    assert_eq!(
        tree.children.len(),
        1,
        "diffing against the same content keeps the child tree rather than rebuilding it"
    );

    // The layout has to come from a real `layout()` pass: the strip's content
    // is an iced `container`, and `container::overlay` unwraps its child's
    // layout node — a hand-made `Node::new` with no children panics inside
    // iced before the pager's own forwarding is ever reached.
    let renderer = crate::test_helpers::headless_renderer();
    let limits = layout::Limits::new(Size::ZERO, Size::new(200.0, BAR_HEIGHT));
    let node = widget.layout(&mut tree, &renderer, &limits);
    let layout = Layout::new(&node);
    let viewport = Rectangle::new(Point::new(0.0, 0.0), node.size());

    assert!(
        widget
            .overlay(&mut tree, layout, &renderer, &viewport, Vector::ZERO)
            .is_none(),
        "the pager forwards overlay() down the real layout and nothing in the \
         strip — neither button nor pip — has a flyout to pop out"
    );
}

#[test]
fn test_layout_and_draw_run_through_the_real_renderer_for_every_page_shape() {
    for model in [
        PipsPagerModel::new(0, 0),
        PipsPagerModel::new(0, 1),
        PipsPagerModel::new(0, 3),
        PipsPagerModel::new(1, 3),
        PipsPagerModel::new(2, 3),
        PipsPagerModel::new(7, 8),
        PipsPagerModel::new(19, 20),
    ] {
        let mut harness = Harness::<PipsMsg, _>::headless(move || pager(model));
        harness.render();

        let buttons = boxes_of(&mut harness, BUTTON_SIZE, BUTTON_SIZE);
        assert_eq!(
            buttons.len(),
            2,
            "{model:?} must always lay out both navigation buttons"
        );
        assert_eq!(
            boxes_of(&mut harness, PIP_SIZE_INACTIVE, PIP_HEIGHT).len()
                + boxes_of(&mut harness, PIP_WIDTH_ACTIVE, PIP_HEIGHT).len(),
            model.total_pages,
            "{model:?} must lay out exactly one pip per page"
        );
    }
}

#[test]
fn test_hovering_the_strip_does_not_change_what_it_commits() {
    let mut harness = Harness::<PipsMsg, _>::headless(move || pager(PipsPagerModel::new(1, 3)));

    let buttons = boxes_of(&mut harness, BUTTON_SIZE, BUTTON_SIZE);
    let next = buttons[1].center();
    assert_eq!(
        harness.hover(next),
        Vec::<PipsMsg>::new(),
        "hover is a paint, never a page change"
    );
    harness.render();
    assert_eq!(
        harness.release(next),
        Vec::<PipsMsg>::new(),
        "a hover alone never arms the button, so releasing still commits nothing"
    );
}
