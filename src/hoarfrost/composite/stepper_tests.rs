//! Unit tests for `stepper` composite (`UIF-COMP-011`, `UIF-PRIN-005`, `UIF-PRIN-004`).

use super::*;
use crate::test_helpers::{
    Candidate, Harness, KIND_CONTAINER, RecordCandidates, centre, headless_renderer,
};
use crate::token::color::Palette;
use crate::token::contrast::contrast_ratio;
use iced::advanced::widget::operation::black_box;

#[test]
fn test_step_status_traits() {
    assert_eq!(StepStatus::Pending, StepStatus::Pending);
    assert_ne!(StepStatus::Pending, StepStatus::Current);
    assert_ne!(StepStatus::Current, StepStatus::Completed);
    assert_ne!(StepStatus::Completed, StepStatus::Error);

    let cloned = StepStatus::Completed;
    assert_eq!(cloned, StepStatus::Completed);
    assert_eq!(format!("{:?}", StepStatus::Error), "Error");
}

#[test]
fn test_stepper_orientation_default_and_traits() {
    assert_eq!(
        StepperOrientation::default(),
        StepperOrientation::Horizontal
    );
    assert_eq!(
        StepperOrientation::Horizontal,
        StepperOrientation::Horizontal
    );
    assert_ne!(StepperOrientation::Horizontal, StepperOrientation::Vertical);
    assert_eq!(format!("{:?}", StepperOrientation::Vertical), "Vertical");
}

#[test]
fn test_step_item_creation_and_disabled_modifier() {
    let item = StepItem::new(1, "Account Details", StepStatus::Pending);
    assert_eq!(item.id, 1);
    assert_eq!(item.label, "Account Details");
    assert_eq!(item.status, StepStatus::Pending);
    assert!(!item.disabled);

    let disabled_item = item.disabled(true);
    assert!(disabled_item.disabled);
    assert_eq!(disabled_item.id, 1);
}

#[test]
fn test_stepper_model_navigation_and_indices() {
    let steps = vec![
        StepItem::new(1, "First", StepStatus::Completed),
        StepItem::new(2, "Second", StepStatus::Current),
        StepItem::new(3, "Third", StepStatus::Pending),
    ];
    let model = StepperModel::new(steps, 2, true);

    assert_eq!(model.current_index(), Some(1));
    assert!(model.can_previous());
    assert!(model.can_next());
    assert!(!model.is_last_step());
    assert_eq!(model.previous_step_id(), Some(1));
    assert_eq!(model.next_step_id(), Some(3));
}

#[test]
fn test_stepper_model_boundary_steps() {
    let steps = vec![
        StepItem::new("a", "Step A", StepStatus::Current),
        StepItem::new("b", "Step B", StepStatus::Pending),
    ];
    let first_step_model = StepperModel::new(steps.clone(), "a", true);
    assert_eq!(first_step_model.current_index(), Some(0));
    assert!(!first_step_model.can_previous());
    assert!(first_step_model.can_next());
    assert!(!first_step_model.is_last_step());
    assert_eq!(first_step_model.previous_step_id(), None);
    assert_eq!(first_step_model.next_step_id(), Some("b"));

    let last_step_model = StepperModel::new(steps, "b", true);
    assert_eq!(last_step_model.current_index(), Some(1));
    assert!(last_step_model.can_previous());
    assert!(!last_step_model.can_next());
    assert!(last_step_model.is_last_step());
    assert_eq!(last_step_model.previous_step_id(), Some("a"));
    assert_eq!(last_step_model.next_step_id(), None);
}

#[test]
fn test_stepper_model_empty_and_missing_current() {
    let empty_model: StepperModel<usize> = StepperModel::new(vec![], 0, false);
    assert_eq!(empty_model.current_index(), None);
    assert!(!empty_model.can_previous());
    assert!(!empty_model.can_next());
    assert!(!empty_model.is_last_step());
    assert_eq!(empty_model.previous_step_id(), None);
    assert_eq!(empty_model.next_step_id(), None);
    assert!(!empty_model.can_select(&0));

    let steps = vec![StepItem::new(1, "One", StepStatus::Pending)];
    let missing_model = StepperModel::new(steps, 999, false);
    assert_eq!(missing_model.current_index(), None);
    assert!(!missing_model.can_previous());
    assert!(!missing_model.can_next());
    assert!(!missing_model.is_last_step());
}

#[test]
fn test_can_select_non_linear_mode() {
    let steps = vec![
        StepItem::new(10, "Profile", StepStatus::Pending),
        StepItem::new(20, "Billing", StepStatus::Current),
        StepItem::new(30, "Confirm", StepStatus::Pending).disabled(true),
    ];
    let model = StepperModel::new(steps, 20, false);

    assert!(
        model.can_select(&10),
        "non-disabled step can be selected in non-linear mode"
    );
    assert!(
        model.can_select(&20),
        "current step can be selected in non-linear mode"
    );
    assert!(
        !model.can_select(&30),
        "disabled step cannot be selected in non-linear mode"
    );
    assert!(!model.can_select(&999), "unknown step returns false");
}

#[test]
fn test_can_select_linear_mode() {
    let steps = vec![
        StepItem::new(1, "Step 1", StepStatus::Completed),
        StepItem::new(2, "Step 2", StepStatus::Current),
        StepItem::new(3, "Step 3", StepStatus::Pending),
        StepItem::new(4, "Step 4", StepStatus::Pending),
        StepItem::new(5, "Step 5", StepStatus::Pending).disabled(true),
    ];
    let model = StepperModel::new(steps, 2, true);

    // Completed step can be selected
    assert!(model.can_select(&1), "completed step is selectable");
    // Immediate next step (current_index + 1 = index 2 = step 3) can be selected
    assert!(model.can_select(&3), "immediate next step is selectable");
    // Non-completed future step (index 3 = step 4) cannot be selected
    assert!(
        !model.can_select(&4),
        "future steps beyond immediate next cannot be selected"
    );
    // Disabled step cannot be selected even if eligible
    assert!(!model.can_select(&5), "disabled step cannot be selected");
    // Unknown step cannot be selected
    assert!(!model.can_select(&100), "unknown step cannot be selected");
}

#[test]
fn test_can_select_linear_prior_incomplete_step() {
    let steps = vec![
        StepItem::new(1, "Step 1", StepStatus::Error),
        StepItem::new(2, "Step 2", StepStatus::Current),
        StepItem::new(3, "Step 3", StepStatus::Pending),
    ];
    let model = StepperModel::new(steps, 2, true);

    // Step 1 is in Error, not Completed -> cannot be selected in linear mode
    assert!(
        !model.can_select(&1),
        "error status prior step is not completed, so cannot be selected"
    );
    assert!(model.can_select(&3), "immediate next is selectable");
}

#[test]
fn test_tokens_and_geometry() {
    assert_eq!(
        BAR_HEIGHT, 36.0,
        "horizontal stepper height must be 36px (size.control.lg) per §3B.11"
    );
    assert_eq!(
        NODE_SIZE, 24.0,
        "step node dimensions must be 24x24px (size.hit_target.min) per §3B.11"
    );
    assert_eq!(
        NODE_RADIUS, 999.0,
        "step node radius must be radius.pill per §3B.11"
    );
    assert_eq!(
        CONNECTOR_STROKE, 1.0,
        "connector stroke must be 1px hairline per §3B.11"
    );
    assert_eq!(
        CURRENT_BORDER_WIDTH, 2.0,
        "current step border must be 2px (shape.border_focus) per §3B.11"
    );
    assert_eq!(
        LABEL_GAP, 8.0,
        "label gap must be 8px (space.sm) per §3B.11"
    );
    assert_eq!(
        STEP_GAP, 12.0,
        "step gap must be 12px (space.md) per §3B.11"
    );
    assert_eq!(
        FOOTER_BUTTON_HEIGHT, 32.0,
        "footer button height must be 32px (size.control.md)"
    );
}

#[test]
fn test_redundant_cues_contrast_and_glyphs() {
    let palette = Palette::dark();

    // 1. Completed: BG: accent.fill, FG: text.strong (>= 4.5:1), glyph '✓'
    assert_eq!(GLYPH_COMPLETED, "✓");
    let completed_contrast = contrast_ratio(palette.text_strong, palette.accent_fill);
    assert!(
        completed_contrast >= 4.5,
        "Completed step contrast ({:.2}:1) must clear 4.5:1 WCAG AA (nominal 4.80:1)",
        completed_contrast
    );

    // 2. Current: BG: surface.active, Border: 2px accent.border, FG: text.strong (>= 4.5:1), number
    let current_fg_contrast = contrast_ratio(palette.text_strong, palette.surface_active);
    assert!(
        current_fg_contrast >= 4.5,
        "Current step text contrast ({:.2}:1) must clear 4.5:1 (nominal 14.29:1)",
        current_fg_contrast
    );
    let current_border_contrast = contrast_ratio(palette.accent_border, palette.surface_active);
    assert!(
        current_border_contrast >= 3.0,
        "Current step border contrast ({:.2}:1) must clear 3.0:1 active chrome (nominal 4.79:1)",
        current_border_contrast
    );

    // 3. Pending: BG: surface.raised, Border: 1px border.strong, FG: text.muted (>= 4.5:1), number
    let pending_fg_contrast = contrast_ratio(palette.text_muted, palette.surface_raised);
    assert!(
        pending_fg_contrast >= 4.5,
        "Pending step text contrast ({:.2}:1) must clear 4.5:1 (nominal 7.16:1)",
        pending_fg_contrast
    );
    let pending_border_contrast = contrast_ratio(palette.border_strong, palette.surface_raised);
    assert!(
        pending_border_contrast >= 3.0,
        "Pending step border contrast ({:.2}:1) must clear 3.0:1 (nominal 4.24:1)",
        pending_border_contrast
    );

    // 4. Error: BG: status.danger.fill, FG: text.strong (>= 4.5:1), glyph '!'
    assert_eq!(GLYPH_ERROR, "!");
    let error_contrast = contrast_ratio(palette.text_strong, palette.status_danger_fill);
    assert!(
        error_contrast >= 4.5,
        "Error step contrast ({:.2}:1) must clear 4.5:1 (nominal 5.13:1)",
        error_contrast
    );
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum StepperMsg {
    Select(usize),
    Next(usize),
    Previous(usize),
    Finish,
    Cancel,
}

#[test]
fn test_widget_creation_and_builder_chain() {
    let steps = vec![
        StepItem::new(1, "Account", StepStatus::Completed),
        StepItem::new(2, "Profile", StepStatus::Current),
        StepItem::new(3, "Finish", StepStatus::Pending),
    ];
    let model = StepperModel::new(steps, 2, true);

    let widget: Stepper<'_, StepperMsg, usize> = stepper(model.clone())
        .orientation(StepperOrientation::Vertical)
        .on_step_select(|c| StepperMsg::Select(c.into_value()))
        .on_next(|c| StepperMsg::Next(c.into_value()))
        .on_previous(|c| StepperMsg::Previous(c.into_value()))
        .on_finish(|_c| StepperMsg::Finish)
        .on_cancel(|_c| StepperMsg::Cancel);

    assert_eq!(widget.model(), &model);
    assert_eq!(widget.orientation_value(), StepperOrientation::Vertical);

    // Conversion into Element
    let _element: Element<'_, StepperMsg> = widget.into();
}

#[test]
fn test_horizontal_stepper_element_conversion() {
    let steps = vec![
        StepItem::new("a", "Alpha", StepStatus::Completed),
        StepItem::new("b", "Beta", StepStatus::Current),
    ];
    let model = StepperModel::new(steps, "b", false);

    let element: Element<'_, StepperMsg> = stepper(model)
        .orientation(StepperOrientation::Horizontal)
        .on_step_select(|_| StepperMsg::Select(1))
        .into();

    let _ = element;
}

#[test]
fn test_wizard_footer_non_shifting_step_zero() {
    let steps = vec![
        StepItem::new(1, "Step 1", StepStatus::Current),
        StepItem::new(2, "Step 2", StepStatus::Pending),
    ];
    let model = StepperModel::new(steps, 1, true);

    let config = WizardFooterConfig::new()
        .on_previous(|c| StepperMsg::Previous(c.into_value()))
        .on_next(|c| StepperMsg::Next(c.into_value()))
        .on_finish(|_| StepperMsg::Finish)
        .on_cancel(|_| StepperMsg::Cancel);

    // On step 0, can_previous() is false, so [← Previous] renders disabled rather than disappearing
    assert!(!model.can_previous());
    assert!(model.can_next());
    assert!(!model.is_last_step());

    let _footer_element: Element<'_, StepperMsg> = wizard_footer(&model, config);
}

#[test]
fn test_wizard_footer_final_step() {
    let steps = vec![
        StepItem::new(1, "Step 1", StepStatus::Completed),
        StepItem::new(2, "Step 2", StepStatus::Current),
    ];
    let model = StepperModel::new(steps, 2, true);

    let config = WizardFooterConfig::new()
        .on_previous(|c| StepperMsg::Previous(c.into_value()))
        .on_finish(|_| StepperMsg::Finish);

    assert!(model.can_previous());
    assert!(!model.can_next());
    assert!(model.is_last_step());

    let _footer_element: Element<'_, StepperMsg> = wizard_footer(&model, config);
}

// ---------------------------------------------------------------------------
// Helpers for the widget-level contract
// ---------------------------------------------------------------------------

/// Four steps covering every status the node renderer knows about.
fn four_steps() -> Vec<StepItem<usize>> {
    vec![
        StepItem::new(1, "Account", StepStatus::Completed),
        StepItem::new(2, "Profile", StepStatus::Current),
        StepItem::new(3, "Review", StepStatus::Pending),
        StepItem::new(4, "Confirm", StepStatus::Error).disabled(true),
    ]
}

/// A linear wizard sitting on its second step.
fn linear_model() -> StepperModel<usize> {
    StepperModel::new(four_steps(), 2, true)
}

/// The same wizard with its last step current and finishable.
fn last_step_model() -> StepperModel<usize> {
    let mut steps = four_steps();
    steps[2].status = StepStatus::Completed;
    steps[3].status = StepStatus::Current;
    steps[3].disabled = false;

    StepperModel::new(steps, 4, true)
}

fn select_commit(commit: Commit<usize>) -> StepperMsg {
    StepperMsg::Select(commit.into_value())
}

fn next_commit(commit: Commit<usize>) -> StepperMsg {
    StepperMsg::Next(commit.into_value())
}

fn previous_commit(commit: Commit<usize>) -> StepperMsg {
    StepperMsg::Previous(commit.into_value())
}

fn finish_commit(_: Commit<()>) -> StepperMsg {
    StepperMsg::Finish
}

fn cancel_commit(_: Cancel<()>) -> StepperMsg {
    StepperMsg::Cancel
}

/// A footer wired for every action the wizard offers.
fn full_footer() -> WizardFooterConfig<'static, StepperMsg, usize> {
    WizardFooterConfig::new()
        .on_previous(previous_commit)
        .on_next(next_commit)
        .on_finish(finish_commit)
        .on_cancel(cancel_commit)
}

/// Whether `outer` fully covers `inner` while being wider than it.
fn encloses(outer: Rectangle, inner: Rectangle) -> bool {
    inner.x >= outer.x
        && inner.y >= outer.y
        && inner.x + inner.width <= outer.x + outer.width
        && inner.y + inner.height <= outer.y + outer.height
        && inner.width < outer.width
}

/// The boxes of the controls themselves — a published box that no *other*
/// published box sits inside. Rows, connectors and step wrappers all publish
/// the same `container` hook as the controls they wrap.
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

/// The step boxes of the sequence, in traversal order — exactly one per step.
///
/// A step is the wrapper the widget builds around its node and its label: the
/// [`NODE_SIZE`]-tall box that is wider than the node it leads. A selectable
/// step publishes that box as a button and a locked step publishes it as a plain
/// container, so every step reports one; the row inside each wrapper repeats the
/// wrapper's bounds exactly, so a box is only counted once.
fn step_boxes(published: &[Candidate]) -> Vec<Rectangle> {
    let mut steps: Vec<Rectangle> = Vec::new();

    for candidate in published {
        let bounds = candidate.bounds;
        if candidate.kind == KIND_CONTAINER
            && bounds.height == NODE_SIZE
            && bounds.width > NODE_SIZE
            && !steps.contains(&bounds)
        {
            steps.push(bounds);
        }
    }

    steps
}

/// The step node indicators — the only 24x24 boxes in the sequence.
///
/// They are read straight off the published containers rather than through
/// [`control_boxes`]: a node is itself a wrapper, of its glyph, so the
/// innermost-box rule would drop it together with the step wrapper above it.
fn node_boxes(published: &[Candidate]) -> Vec<Rectangle> {
    published
        .iter()
        .filter(|candidate| candidate.kind == KIND_CONTAINER)
        .map(|candidate| candidate.bounds)
        .filter(|bounds| bounds.width == NODE_SIZE && bounds.height == NODE_SIZE)
        .collect()
}

/// Press and release a box's centre, keeping every message the widget answered.
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

// ---------------------------------------------------------------------------
// Step selection as a real hit target
// ---------------------------------------------------------------------------

#[test]
fn test_only_the_steps_the_linear_rules_allow_answer_a_click() {
    let model = linear_model();
    let mut harness = Harness::headless(move || {
        let bar: Element<'_, StepperMsg> =
            stepper(model.clone()).on_step_select(select_commit).into();
        bar
    });

    let steps = step_boxes(&harness.candidates());
    assert_eq!(
        steps.len(),
        4,
        "every step publishes its own box, the locked ones too, so a silent click \
         means the step refused rather than went missing"
    );

    let mut selected: Vec<usize> = Vec::new();
    for step in steps {
        selected.extend(click(&mut harness, step).into_iter().filter_map(
            |message| match message {
                StepperMsg::Select(id) => Some(id),
                _ => None,
            },
        ));
    }

    assert_eq!(
        selected,
        vec![1, 3],
        "the completed step and the immediate next step are clickable; the current, \
         the beyond-next and the disabled step are not"
    );
}

#[test]
fn test_a_stepper_without_a_select_callback_answers_no_click_at_all() {
    let model = linear_model();
    let mut harness = Harness::headless(move || {
        let bar: Element<'_, StepperMsg> = stepper(model.clone()).into();
        bar
    });

    let steps = control_boxes(&harness.candidates());
    assert!(
        !steps.is_empty(),
        "the steps still publish their boxes even with nothing to commit"
    );

    for step in steps {
        assert!(
            click(&mut harness, step).is_empty(),
            "a stepper built without `on_step_select` must not fabricate a message"
        );
    }
}

#[test]
fn test_the_horizontal_band_is_fixed_and_the_vertical_stack_is_not() {
    let model = linear_model();
    let stacked_model = model.clone();
    let mut harness = Harness::headless(move || {
        let bar: Element<'_, StepperMsg> = stepper(model.clone()).into();
        bar
    });

    let band = harness
        .candidates()
        .first()
        .expect("the stepper publishes its own box")
        .bounds;
    assert_eq!(
        band.height, BAR_HEIGHT,
        "the horizontal stepper is a 36px band (UIF-COMP-011)"
    );
    harness.render();

    harness.set_view(move || {
        let stack: Element<'_, StepperMsg> = stepper(stacked_model.clone())
            .orientation(StepperOrientation::Vertical)
            .into();
        stack
    });
    let stack = harness
        .candidates()
        .first()
        .expect("the vertical stepper publishes its own box")
        .bounds;
    assert!(
        stack.height > BAR_HEIGHT,
        "stacking four steps vertically must exceed the 36px band"
    );
    harness.render();
}

#[test]
fn test_the_stepper_reports_its_band_height_and_one_child_slot() {
    let widget: Stepper<'_, StepperMsg, usize> = stepper(linear_model());

    let requested = widget.size();
    assert_eq!(
        requested.width,
        Length::Fill,
        "the stepper spans the width it is given (UIF-PRIN-004)"
    );
    assert_eq!(
        requested.height,
        Length::Fixed(BAR_HEIGHT),
        "the horizontal stepper reports its fixed 36px band"
    );
    assert_eq!(
        widget.children().len(),
        1,
        "the composite wraps a single content element, so it publishes one child tree"
    );

    let mut tree = Tree::empty();
    widget.diff(&mut tree);
    assert_eq!(
        tree.children.len(),
        1,
        "diffing against a tree with no children must create the content slot"
    );

    let stacked: Stepper<'_, StepperMsg, usize> =
        stepper(linear_model()).orientation(StepperOrientation::Vertical);
    assert_eq!(
        stacked.size().height,
        Length::Shrink,
        "the vertical stack is as tall as its steps and connectors"
    );
}

// ---------------------------------------------------------------------------
// The non-shifting wizard footer
// ---------------------------------------------------------------------------

#[test]
fn test_the_footer_keeps_the_disabled_previous_slot_on_step_zero() {
    let mut harness = Harness::headless(move || {
        wizard_footer(&StepperModel::new(four_steps(), 1, true), full_footer())
    });

    let controls = control_boxes(&harness.candidates());
    assert_eq!(
        controls.len(),
        3,
        "cancel, previous and next are the three footer controls"
    );
    assert_eq!(
        click(&mut harness, controls[0]),
        vec![StepperMsg::Cancel],
        "[Cancel] commits the cancel binding"
    );
    assert_eq!(
        click(&mut harness, controls[1]),
        Vec::<StepperMsg>::new(),
        "[← Previous] stays disabled on step 0 but keeps its slot (UIF-PRIN-004)"
    );
    assert_eq!(
        click(&mut harness, controls[2]),
        vec![StepperMsg::Next(2)],
        "[Next →] commits the id of the step it advances to"
    );
    harness.render();
}

#[test]
fn test_the_footer_swaps_next_for_finish_on_the_last_step() {
    assert!(last_step_model().is_last_step());

    let mut harness = Harness::headless(move || wizard_footer(&last_step_model(), full_footer()));

    let controls = control_boxes(&harness.candidates());
    assert_eq!(
        controls.len(),
        3,
        "the last step still shows three controls"
    );
    assert_eq!(
        click(&mut harness, controls[1]),
        vec![StepperMsg::Previous(3)],
        "[← Previous] commits the step it steps back to"
    );
    assert_eq!(
        click(&mut harness, controls[2]),
        vec![StepperMsg::Finish],
        "the last step swaps [Next →] for [Finish ✓] and commits the finish binding"
    );
    harness.render();
}

#[test]
fn test_a_footer_without_callbacks_still_shows_its_action_slots() {
    let mut harness = Harness::headless(move || {
        let footer: Element<'_, StepperMsg> = wizard_footer(
            &StepperModel::new(four_steps(), 2, true),
            WizardFooterConfig::default(),
        );
        footer
    });

    let controls = control_boxes(&harness.candidates());
    assert_eq!(
        controls.len(),
        2,
        "with no cancel handler the cancel slot collapses, but previous and next stay put"
    );
    for control in controls {
        assert!(
            click(&mut harness, control).is_empty(),
            "a footer built without handlers must not fabricate a message"
        );
    }
}

// ---------------------------------------------------------------------------
// Cursor and overlay delegation
// ---------------------------------------------------------------------------

#[test]
fn test_the_stepper_asks_for_a_pointer_only_over_a_selectable_step() {
    let model = linear_model();
    let twin: Element<'_, StepperMsg> = stepper(model.clone()).on_step_select(select_commit).into();
    let mut tree = Tree::new(&twin);

    let mut widget = stepper(model).on_step_select(select_commit);
    let renderer = headless_renderer();
    let limits = layout::Limits::new(Size::ZERO, Size::new(400.0, 300.0));
    let node = widget.layout(&mut tree, &renderer, &limits);
    let layout = Layout::new(&node);
    let viewport = Rectangle::with_size(node.size());

    let widget_dyn: &mut dyn Widget<StepperMsg, iced::Theme, iced::Renderer> = &mut widget;
    let nodes = node_boxes(&published_boxes(widget_dyn, &mut tree, layout, &renderer));
    assert_eq!(
        nodes.len(),
        4,
        "every step publishes its 24px node indicator"
    );

    assert_eq!(
        widget.mouse_interaction(
            &tree,
            layout,
            mouse::Cursor::Available(centre(nodes[0])),
            &viewport,
            &renderer,
        ),
        mouse::Interaction::Pointer,
        "the completed step is selectable, so it must ask for the pointer cursor"
    );

    assert_eq!(
        widget.mouse_interaction(
            &tree,
            layout,
            mouse::Cursor::Available(centre(nodes[1])),
            &viewport,
            &renderer,
        ),
        mouse::Interaction::None,
        "the current step is not a navigation target, so it must not invite a click"
    );

    assert!(
        widget
            .overlay(&mut tree, layout, &renderer, &viewport, Vector::ZERO)
            .is_none(),
        "the stepper has no popover to hand the host"
    );
}
