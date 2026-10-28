//! Render-level ("ink") coverage for the wizard stepper (`UIF-COMP-011`).
//!
//! The step indicator paints **entirely from `StepItem::status`**: glyph,
//! fill, border weight, foreground and label colour are one `match` on
//! that field (`hoarfrost/composite/stepper.rs:656`, label at `:726`).
//! `current_step` is never read by the paint at all. That makes the
//! stepper the sharpest repaint target in this demo — a host that moves
//! `current_step` without rewriting the statuses leaves a frame
//! byte-identical to the one before it, which is precisely the defect
//! these tests were written against.
//!
//! Every assertion reads the framebuffer. None of them consults
//! `StepItem::status` to decide whether the screen changed: [`Painted`]
//! is derived from which palette fill actually covers the node, so "this
//! node painted `Completed`" is a statement about ink, not about the
//! model. Transitions are driven through
//! [`CompositesState::apply`](crate::pane_composites::CompositesState::apply),
//! the same path the live footer's buttons take, rather than by
//! hand-building a model that already looks right.
//!
//! Node rects come out of the layout probe rather than being computed: the
//! step nodes are the only `NODE_SIZE` squares the section draws, so they
//! are addressable by geometry even though the framework gives them no
//! [`iced::widget::Id`].

use iced::Rectangle;

use hoarfrost::composite::stepper::{BAR_HEIGHT, FOOTER_BUTTON_HEIGHT, NODE_SIZE, StepStatus};
use hoarfrost::token::color::Palette;

use crate::layout_probe::Probe;
use crate::pane_composites::{self, CompositesMessage, CompositesState};
use crate::render_capture::{Frame, PANE_VIEWPORT, Studio, assert_repainted, to_rgb};
use crate::render_section::{Captured, capture_diff};

/// How far a pixel may sit from a status fill and still count as that fill.
///
/// Eight is above the blending noise the rasteriser leaves on the pill's
/// edge and far below the closest pair of status fills — `surface_raised`
/// `(18, 18, 20)` and `surface_active` `(42, 42, 46)`, twenty-four apart.
const FILL_TOLERANCE: u8 = 8;

/// The fewest pixels of a node a fill must cover before it identifies it.
///
/// Measured fills run from 1 079 pixels (`Current`) to 1 833 (`Pending`)
/// on a 24px node. Six hundred sits below the weakest real fill and well
/// above anything a node that painted nothing could produce.
const MIN_FILL_PIXELS: usize = 600;

/// Slack when comparing a recorded rect against a layout token.
const GEOMETRY_EPSILON: f32 = 0.01;

/// Which status a painted node reads as, by the fill covering it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Painted {
    /// The `surface_raised` fill of a step not yet reached.
    Pending,
    /// The `surface_active` fill of the step under the cursor.
    Current,
    /// The `accent_fill` of a step the ladder has passed.
    Completed,
    /// The `status_danger_fill` of a step that failed.
    Error,
}

/// The status the node's painted fill corresponds to.
///
/// The winner is whichever palette fill covers the most of the node, so
/// the reading never depends on where the glyph or the border happens to
/// land. `None` when no fill covers enough of the node to name it, which
/// keeps "nothing was painted" a distinct answer rather than a wrong one.
/// Ties go to the last candidate declared, so the result is deterministic.
fn painted(frame: &Frame, node: Rectangle) -> Option<Painted> {
    let palette = Palette::dark();
    let candidates = [
        (Painted::Pending, to_rgb(palette.surface_raised)),
        (Painted::Current, to_rgb(palette.surface_active)),
        (Painted::Completed, to_rgb(palette.accent_fill)),
        (Painted::Error, to_rgb(palette.status_danger_fill)),
    ];

    let (status, coverage) = candidates
        .into_iter()
        .map(|(status, fill)| (status, frame.matching_count(node, fill, FILL_TOLERANCE)))
        .max_by_key(|(_, coverage)| *coverage)?;

    if coverage >= MIN_FILL_PIXELS {
        Some(status)
    } else {
        None
    }
}

/// The step-node rects, left to right, as the probe recorded them.
fn step_nodes(probe: &Probe) -> Vec<Rectangle> {
    let mut nodes: Vec<Rectangle> = probe
        .all_entries()
        .iter()
        .map(|entry| entry.bounds)
        .filter(|rect| {
            (rect.width - NODE_SIZE).abs() < GEOMETRY_EPSILON
                && (rect.height - NODE_SIZE).abs() < GEOMETRY_EPSILON
        })
        .collect();
    nodes.sort_by(|a, b| a.x.total_cmp(&b.x));
    nodes
}

/// The bar the step row is laid out in.
///
/// # Panics
/// When the section drew no `BAR_HEIGHT` row, which means the stepper is
/// not on screen at all.
fn bar_rect(probe: &Probe) -> Rectangle {
    probe
        .all_entries()
        .iter()
        .map(|entry| entry.bounds)
        .find(|rect| (rect.height - BAR_HEIGHT).abs() < GEOMETRY_EPSILON)
        .unwrap_or_else(|| panic!("the stepper drew no {BAR_HEIGHT}px bar to measure"))
}

/// The wizard footer's own row — the control that drives these transitions.
///
/// Read as the widest `FOOTER_BUTTON_HEIGHT` box, which separates the
/// footer row from the buttons inside it.
///
/// # Panics
/// When no such row was drawn.
fn footer_rect(probe: &Probe) -> Rectangle {
    probe
        .all_entries()
        .iter()
        .map(|entry| entry.bounds)
        .filter(|rect| (rect.height - FOOTER_BUTTON_HEIGHT).abs() < GEOMETRY_EPSILON)
        .max_by(|a, b| a.width.total_cmp(&b.width))
        .unwrap_or_else(|| panic!("the wizard footer drew no {}px row", FOOTER_BUTTON_HEIGHT))
}

/// A clone of `state` after the demo's own update path folded `message` in.
///
/// This is the point of the harness: the render tests drive the
/// transition the footer's "Next" actually triggers, so a model that is
/// only ever correct on the test bench cannot pass.
fn driven(before: &CompositesState, message: CompositesMessage) -> CompositesState {
    let mut after = before.clone();
    after.apply(&message);
    after
}

/// `state` after the same message has been applied `times` times.
fn walked(state: &CompositesState, message: CompositesMessage, times: usize) -> CompositesState {
    (0..times).fold(state.clone(), |acc, _| driven(&acc, message.clone()))
}

/// The stepper section's layout, as the probe recorded it.
fn stepper_probe(studio: &mut Studio, state: &CompositesState) -> Probe {
    studio.probe(
        PANE_VIEWPORT,
        pane_composites::stepper_section(state).element,
    )
}

/// The status each step node actually paints, left to right.
fn painted_ladder(studio: &mut Studio, state: &CompositesState) -> Vec<Option<Painted>> {
    let probe = stepper_probe(studio, state);
    let frame = studio.frame(
        PANE_VIEWPORT,
        pane_composites::stepper_section(state).element,
    );
    step_nodes(&probe)
        .iter()
        .map(|node| painted(&frame, *node))
        .collect()
}

/// A step model with step `index` forced into `status`.
fn with_error(state: &CompositesState, index: usize) -> CompositesState {
    let mut broken = state.clone();
    broken.stepper.steps[index].status = StepStatus::Error;
    broken
}

// ---------------------------------------------------------------------------
// The repaint itself
// ---------------------------------------------------------------------------

#[test]
fn pressing_next_repaints_the_nodes_the_ladder_moves() {
    let mut studio = Studio::new();
    let before = CompositesState::new();
    let after = driven(&before, CompositesMessage::StepNext);
    let nodes = step_nodes(&stepper_probe(&mut studio, &before));

    let diff = capture_diff(
        &mut studio,
        PANE_VIEWPORT,
        pane_composites::stepper_section,
        &before,
        &after,
    );

    // The step left and the step entered are the two the ladder moves,
    // and both must genuinely repaint. Against the unfixed transition
    // this measures exactly zero at every node: the model advanced, the
    // bar did not.
    assert_repainted(
        &diff.before,
        &diff.after,
        nodes[1],
        "pressing Next on the wizard (step 2 left behind)",
    );
    assert_repainted(
        &diff.before,
        &diff.after,
        nodes[2],
        "pressing Next on the wizard (step 3 entered)",
    );

    // And only those two. A step whose status did not move must not move
    // a pixel, which is what stops a blanket invalidate from passing for
    // a correct repaint.
    for index in [0, 3] {
        assert_eq!(
            diff.before.differing_pixels(&diff.after, nodes[index]),
            0,
            "step {index} changed no status yet repainted inside {:?}",
            nodes[index]
        );
    }
}

#[test]
fn the_step_left_paints_completed_and_the_step_entered_paints_current() {
    let mut studio = Studio::new();
    let before = CompositesState::new();
    let after = driven(&before, CompositesMessage::StepNext);

    let fixture = painted_ladder(&mut studio, &before);
    let advanced = painted_ladder(&mut studio, &after);

    assert_eq!(
        fixture,
        vec![
            Some(Painted::Completed),
            Some(Painted::Current),
            Some(Painted::Pending),
            Some(Painted::Pending)
        ],
        "the fixture's own painted ladder is not what these tests assume"
    );

    // One step on: the node the cursor left now carries the accent fill
    // of a completed step and the newly entered node the active fill.
    // Read off the ink, so a status field that moved without ever
    // reaching the framebuffer still fails here.
    assert_eq!(
        advanced,
        vec![
            Some(Painted::Completed),
            Some(Painted::Completed),
            Some(Painted::Current),
            Some(Painted::Pending)
        ],
        "after Next the painted ladder never became Completed / Current"
    );
}

#[test]
fn pressing_back_reverses_what_the_indicator_shows() {
    let mut studio = Studio::new();
    let at_three = driven(&CompositesState::new(), CompositesMessage::StepNext);
    let back_to_two = driven(&at_three, CompositesMessage::StepPrevious);
    let nodes = step_nodes(&stepper_probe(&mut studio, &at_three));

    let diff = capture_diff(
        &mut studio,
        PANE_VIEWPORT,
        pane_composites::stepper_section,
        &at_three,
        &back_to_two,
    );

    assert_repainted(
        &diff.before,
        &diff.after,
        nodes[1],
        "pressing Back on the wizard (step 2 re-entered)",
    );
    assert_repainted(
        &diff.before,
        &diff.after,
        nodes[2],
        "pressing Back on the wizard (step 3 left behind)",
    );

    assert_eq!(
        painted_ladder(&mut studio, &back_to_two),
        vec![
            Some(Painted::Completed),
            Some(Painted::Current),
            Some(Painted::Pending),
            Some(Painted::Pending)
        ],
        "Back did not put the painted ladder back the way it came"
    );
}

// ---------------------------------------------------------------------------
// Clamping
// ---------------------------------------------------------------------------

#[test]
fn stepping_past_either_end_leaves_the_indicator_byte_identical() {
    let mut studio = Studio::new();
    let base = CompositesState::new();

    // Pinned at the last step: another Next is a no-op and the frame
    // must say so. A ladder that corrupted itself past the end would
    // show up here as ink moving on a transition that changed nothing.
    let at_last = walked(&base, CompositesMessage::StepNext, 4);
    let past_last = driven(&at_last, CompositesMessage::StepNext);
    let last_diff = capture_diff(
        &mut studio,
        PANE_VIEWPORT,
        pane_composites::stepper_section,
        &at_last,
        &past_last,
    );
    assert_eq!(
        last_diff
            .before
            .differing_pixels(&last_diff.after, last_diff.body),
        0,
        "Next past the last step repainted the wizard"
    );
    assert_eq!(
        painted_ladder(&mut studio, &at_last),
        vec![
            Some(Painted::Completed),
            Some(Painted::Completed),
            Some(Painted::Completed),
            Some(Painted::Current)
        ],
        "the ladder at the last step is not the full Completed run it must be"
    );

    // Pinned at the first step: the ladder restarts cleanly, with no
    // Completed node left behind to the left of the cursor.
    let at_first = walked(&base, CompositesMessage::StepPrevious, 4);
    let past_first = driven(&at_first, CompositesMessage::StepPrevious);
    let first_diff = capture_diff(
        &mut studio,
        PANE_VIEWPORT,
        pane_composites::stepper_section,
        &at_first,
        &past_first,
    );
    assert_eq!(
        first_diff
            .before
            .differing_pixels(&first_diff.after, first_diff.body),
        0,
        "Back before the first step repainted the wizard"
    );
    assert_eq!(
        painted_ladder(&mut studio, &at_first),
        vec![
            Some(Painted::Current),
            Some(Painted::Pending),
            Some(Painted::Pending),
            Some(Painted::Pending)
        ],
        "the ladder at the first step still carries Completed nodes"
    );
}

// ---------------------------------------------------------------------------
// Geometry — the ladder must not move the indicator around
// ---------------------------------------------------------------------------

#[test]
fn the_indicator_neither_grows_nor_jumps_when_the_ladder_moves() {
    let mut studio = Studio::new();
    let before = CompositesState::new();
    let after = driven(&before, CompositesMessage::StepNext);

    let before_nodes = step_nodes(&stepper_probe(&mut studio, &before));
    let after_nodes = step_nodes(&stepper_probe(&mut studio, &after));

    // Same number of steps, so the ladder cannot add or drop a node.
    assert_eq!(
        before_nodes.len(),
        after.stepper.steps.len(),
        "the step count changed when the ladder moved"
    );
    assert_eq!(before_nodes.len(), after_nodes.len());

    // Every node stays exactly NODE_SIZE square: a status change must not
    // grow or shrink the thing it is painted on.
    for (index, node) in after_nodes.iter().enumerate() {
        assert!(
            (node.width - NODE_SIZE).abs() < GEOMETRY_EPSILON
                && (node.height - NODE_SIZE).abs() < GEOMETRY_EPSILON,
            "step {index} node is {node:?}, not the {NODE_SIZE}px square it must stay"
        );
    }

    // The row the nodes sit in keeps the same footprint: the head of the
    // sequence does not move and neither does its tail. Interior nodes
    // drift a pixel or two because the framework gives the current
    // step's *label* a heavier weight, but the indicator as a whole
    // occupies exactly the same ground before and after.
    assert!(
        (before_nodes[0].x - after_nodes[0].x).abs() < GEOMETRY_EPSILON,
        "the first step node moved from {} to {}",
        before_nodes[0].x,
        after_nodes[0].x
    );
    let before_tail = before_nodes[3].x + before_nodes[3].width;
    let after_tail = after_nodes[3].x + after_nodes[3].width;
    assert!(
        (before_tail - after_tail).abs() < GEOMETRY_EPSILON,
        "the last step node's trailing edge moved from {before_tail} to {after_tail}"
    );
    assert_eq!(
        bar_rect(&stepper_probe(&mut studio, &before)),
        bar_rect(&stepper_probe(&mut studio, &after))
    );
    assert_eq!(
        footer_rect(&stepper_probe(&mut studio, &before)),
        footer_rect(&stepper_probe(&mut studio, &after))
    );

    // And the section's own card and body region are unmoved, so nothing
    // below the stepper shifts when a status changes either.
    let before_card = Captured::capture(&mut studio, PANE_VIEWPORT, || {
        pane_composites::stepper_section(&before)
    });
    let after_card = Captured::capture(&mut studio, PANE_VIEWPORT, || {
        pane_composites::stepper_section(&after)
    });
    assert_eq!(before_card.card, after_card.card);
    assert_eq!(before_card.body, after_card.body);
}

// ---------------------------------------------------------------------------
// Error stickiness
// ---------------------------------------------------------------------------

#[test]
fn an_error_step_keeps_its_ink_until_the_wizard_returns_to_it() {
    let mut studio = Studio::new();
    let broken = with_error(&CompositesState::new(), 0);

    // Navigating *away* from a failed step must not launder it: step 0
    // keeps the danger fill while the cursor sits on step 3.
    let onward = driven(&broken, CompositesMessage::StepNext);
    assert_eq!(
        painted_ladder(&mut studio, &onward),
        vec![
            Some(Painted::Error),
            Some(Painted::Completed),
            Some(Painted::Current),
            Some(Painted::Pending)
        ],
        "Next laundered the failed step out of the indicator"
    );

    // Coming *back* one step is still not the same as coming back *to*
    // the failed step: the badge stays while the cursor is anywhere else.
    let one_back = walked(&onward, CompositesMessage::StepPrevious, 1);
    assert_eq!(
        painted_ladder(&mut studio, &one_back),
        vec![
            Some(Painted::Error),
            Some(Painted::Current),
            Some(Painted::Pending),
            Some(Painted::Pending)
        ],
        "stepping back to a neighbour must not clear a step that was never re-entered"
    );

    // Reaching it is the moment the user gets to fix it, so the badge is
    // released and the node paints as the current step.
    let returned = walked(&onward, CompositesMessage::StepPrevious, 2);
    assert_eq!(
        painted_ladder(&mut studio, &returned),
        vec![
            Some(Painted::Current),
            Some(Painted::Pending),
            Some(Painted::Pending),
            Some(Painted::Pending)
        ],
        "re-entering the failed step did not clear its error ink"
    );
}

#[test]
fn an_error_step_still_paints_error_when_the_wizard_jumps_to_the_end() {
    let mut studio = Studio::new();
    let broken = with_error(&CompositesState::new(), 0);

    let jumped = driven(&broken, CompositesMessage::StepSelected(3));
    assert_eq!(
        painted_ladder(&mut studio, &jumped),
        vec![
            Some(Painted::Error),
            Some(Painted::Completed),
            Some(Painted::Completed),
            Some(Painted::Current)
        ],
        "jumping to the last step silently completed the failed one"
    );
}
