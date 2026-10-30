//! Unit tests for `side_sheet` composite (`UIF-COMP-012`, `UIF-PRIN-006`, `UIF-MOD-002`).

use super::*;
use crate::binding::GestureStart;
use crate::surface::zorder::ZBand;
use crate::test_helpers::Harness;
use crate::test_helpers::headless_renderer;
use crate::token::color::Palette;
use crate::token::contrast::contrast_ratio;
use iced::Event;
use iced::advanced::Widget;
use iced::keyboard::Key;
use iced::keyboard::key::Named;

// ---------------------------------------------------------------------------
// 1. Data Model Tests
// ---------------------------------------------------------------------------

#[test]
fn test_side_sheet_model_defaults_and_traits() {
    let model = SideSheetModel::default();
    assert_eq!(model.title, DEFAULT_TITLE);
    assert_eq!(model.mode, SheetMode::Docked);
    assert_eq!(model.placement, SheetPlacement::Right);
    assert_eq!(model.width, DEFAULT_WIDTH);
    assert!(model.open);
    assert!(!model.dirty);
    assert!(!model.busy);

    let custom = SideSheetModel::new("Properties");
    assert_eq!(custom.title, "Properties");
    assert_eq!(custom.mode, SheetMode::Docked);
    assert_eq!(custom.placement, SheetPlacement::Right);
    assert_eq!(custom.width, DEFAULT_WIDTH);

    let cloned = custom.clone();
    assert_eq!(custom, cloned);
    assert_eq!(format!("{:?}", custom.mode), "Docked");
    assert_eq!(format!("{:?}", custom.placement), "Right");
}

#[test]
fn test_side_sheet_model_builder_methods() {
    let model = SideSheetModel::new("Inspector")
        .with_mode(SheetMode::ModelessOverlay)
        .with_placement(SheetPlacement::Left)
        .with_width(400.0)
        .with_open(false)
        .with_dirty(true)
        .with_busy(true);

    assert_eq!(model.mode, SheetMode::ModelessOverlay);
    assert_eq!(model.placement, SheetPlacement::Left);
    assert_eq!(model.width, 400.0);
    assert!(!model.open);
    assert!(model.dirty);
    assert!(model.busy);
}

#[test]
fn test_side_sheet_width_clamping() {
    assert_eq!(SideSheetModel::clamp_width(200.0), MIN_WIDTH);
    assert_eq!(SideSheetModel::clamp_width(279.9), MIN_WIDTH);
    assert_eq!(SideSheetModel::clamp_width(MIN_WIDTH), MIN_WIDTH);

    assert_eq!(SideSheetModel::clamp_width(360.0), 360.0);
    assert_eq!(SideSheetModel::clamp_width(450.0), 450.0);

    assert_eq!(SideSheetModel::clamp_width(MAX_WIDTH), MAX_WIDTH);
    assert_eq!(SideSheetModel::clamp_width(520.1), MAX_WIDTH);
    assert_eq!(SideSheetModel::clamp_width(900.0), MAX_WIDTH);

    let model = SideSheetModel::new("Test").with_width(100.0);
    assert_eq!(model.width, MIN_WIDTH);

    let model_max = SideSheetModel::new("Test").with_width(1000.0);
    assert_eq!(model_max.width, MAX_WIDTH);
}

#[test]
fn test_responsive_breakpoint_and_effective_mode() {
    let docked = SideSheetModel::new("Test").with_mode(SheetMode::Docked);
    let overlay = SideSheetModel::new("Test").with_mode(SheetMode::ModelessOverlay);
    let scrim = SideSheetModel::new("Test").with_mode(SheetMode::ModalScrim);

    // Viewport >= 800.0 preserves configured mode
    assert_eq!(docked.effective_mode(1200.0), SheetMode::Docked);
    assert_eq!(overlay.effective_mode(1024.0), SheetMode::ModelessOverlay);
    assert_eq!(scrim.effective_mode(800.0), SheetMode::ModalScrim);

    // Viewport < 800.0 automatically promotes to ModalScrim
    assert_eq!(docked.effective_mode(799.9), SheetMode::ModalScrim);
    assert_eq!(overlay.effective_mode(640.0), SheetMode::ModalScrim);
    assert_eq!(scrim.effective_mode(480.0), SheetMode::ModalScrim);
}

#[test]
fn test_can_close_dirty_and_busy_gating() {
    // 1. Clean and not busy -> immediate dismissal allowed
    let clean = SideSheetModel::new("Clean");
    assert!(clean.can_close());

    // 2. Dirty and not busy -> confirmation required (UIF-MOD-002)
    let dirty = SideSheetModel::new("Dirty").with_dirty(true);
    assert!(!dirty.can_close());

    // 3. Busy and clean -> close blocked (UIF-MOD-002)
    let busy = SideSheetModel::new("Busy").with_busy(true);
    assert!(!busy.can_close());

    // 4. Busy and dirty -> close blocked
    let busy_dirty = SideSheetModel::new("Both").with_dirty(true).with_busy(true);
    assert!(!busy_dirty.can_close());
}

#[test]
fn test_z_band_mapping() {
    let docked = SideSheetModel::new("Docked").with_mode(SheetMode::Docked);
    let overlay = SideSheetModel::new("Overlay").with_mode(SheetMode::ModelessOverlay);
    let modal = SideSheetModel::new("Modal").with_mode(SheetMode::ModalScrim);

    // Viewport width >= 800px:
    assert_eq!(docked.z_band(1024.0), ZBand::Workspace);
    assert_eq!(overlay.z_band(1024.0), ZBand::UtilityOverlay);
    assert_eq!(modal.z_band(1024.0), ZBand::Modal);

    // Viewport width < 800px promotes all modes to Band 3 (Modal):
    assert_eq!(docked.z_band(768.0), ZBand::Modal);
    assert_eq!(overlay.z_band(640.0), ZBand::Modal);
    assert_eq!(modal.z_band(480.0), ZBand::Modal);
}

#[test]
fn test_escape_rank_mapping() {
    let docked = SideSheetModel::new("Docked").with_mode(SheetMode::Docked);
    let overlay = SideSheetModel::new("Overlay").with_mode(SheetMode::ModelessOverlay);
    let modal = SideSheetModel::new("Modal").with_mode(SheetMode::ModalScrim);

    // Viewport width >= 800px:
    assert_eq!(docked.escape_rank(1024.0), 0);
    assert_eq!(overlay.escape_rank(1024.0), 1);
    assert_eq!(modal.escape_rank(1024.0), 2);

    // Viewport width < 800px promotes all modes to Rank 2:
    assert_eq!(docked.escape_rank(768.0), 2);
    assert_eq!(overlay.escape_rank(640.0), 2);
    assert_eq!(modal.escape_rank(480.0), 2);
}

#[test]
fn test_sheet_mode_toggle() {
    assert_eq!(SheetMode::Docked.toggle(), SheetMode::ModelessOverlay);
    assert_eq!(SheetMode::ModelessOverlay.toggle(), SheetMode::Docked);
    assert_eq!(SheetMode::ModalScrim.toggle(), SheetMode::Docked);
}

#[test]
fn test_sheet_placement_traits() {
    assert_eq!(SheetPlacement::default(), SheetPlacement::Right);
    assert_eq!(SheetPlacement::Left, SheetPlacement::Left);
    assert_ne!(SheetPlacement::Left, SheetPlacement::Right);
    let cloned = SheetPlacement::Left;
    assert_eq!(cloned, SheetPlacement::Left);
}

// ---------------------------------------------------------------------------
// 2. Resize Gesture Engine Tests
// ---------------------------------------------------------------------------

#[test]
fn test_resize_gesture_initial_state() {
    let gesture = SideSheetResize::default();
    assert!(!gesture.is_active());
    assert_eq!(gesture.placement(), SheetPlacement::Right);
    assert_eq!(gesture.start_width(), DEFAULT_WIDTH);
    assert_eq!(gesture.current_width(), DEFAULT_WIDTH);
}

#[test]
fn test_resize_gesture_right_placement_drag() {
    let mut gesture = SideSheetResize::new(SheetPlacement::Right);

    // Start drag at x = 500.0, start_width = 360.0
    let start = gesture.begin(360.0, 500.0);
    assert_eq!(start, GestureStart::Started);
    assert!(gesture.is_active());
    assert_eq!(gesture.start_width(), 360.0);

    // Dragging left (x = 460.0, dx = -40.0) -> width increases: 360 - (-40) = 400.0
    let live1 = gesture.update(460.0).expect("should emit Live");
    assert_eq!(*live1.value(), 400.0);
    assert_eq!(gesture.current_width(), 400.0);

    // Dragging right (x = 550.0, dx = +50.0) -> width decreases: 360 - 50 = 310.0
    let live2 = gesture.update(550.0).expect("should emit Live");
    assert_eq!(*live2.value(), 310.0);
    assert_eq!(gesture.current_width(), 310.0);

    // Clamping to MIN_WIDTH (280px): x = 600.0 -> dx = +100 -> 360 - 100 = 260 -> clamps to 280
    let live_min = gesture.update(600.0).expect("should emit Live");
    assert_eq!(*live_min.value(), MIN_WIDTH);

    // Clamping to MAX_WIDTH (520px): x = 300.0 -> dx = -200 -> 360 - (-200) = 560 -> clamps to 520
    let live_max = gesture.update(300.0).expect("should emit Live");
    assert_eq!(*live_max.value(), MAX_WIDTH);

    // Commit settles at current width and closes slot
    let commit = gesture.commit().expect("should emit Commit");
    assert_eq!(*commit.value(), MAX_WIDTH);
    assert!(!gesture.is_active());

    // Second commit returns None
    assert!(gesture.commit().is_none());
}

#[test]
fn test_resize_gesture_left_placement_drag() {
    let mut gesture = SideSheetResize::new(SheetPlacement::Left);

    // Start drag at x = 360.0, start_width = 360.0
    let start = gesture.begin(360.0, 360.0);
    assert_eq!(start, GestureStart::Started);
    assert!(gesture.is_active());

    // Dragging right (x = 410.0, dx = +50.0) -> width increases: 360 + 50 = 410.0
    let live1 = gesture.update(410.0).expect("should emit Live");
    assert_eq!(*live1.value(), 410.0);

    // Dragging left (x = 300.0, dx = -60.0) -> width decreases: 360 - 60 = 300.0
    let live2 = gesture.update(300.0).expect("should emit Live");
    assert_eq!(*live2.value(), 300.0);

    // Commit
    let commit = gesture.commit().expect("should emit Commit");
    assert_eq!(*commit.value(), 300.0);
    assert!(!gesture.is_active());
}

#[test]
fn test_resize_gesture_cancel_reverts_to_pre_drag_snapshot() {
    let mut gesture = SideSheetResize::new(SheetPlacement::Right);
    let _ = gesture.begin(360.0, 500.0);

    // Mutate with drag
    let _ = gesture.update(450.0);
    assert_eq!(gesture.current_width(), 410.0);

    // Cancel reverts to 360.0
    let cancel = gesture.cancel().expect("should emit Cancel");
    assert_eq!(*cancel.restored(), 360.0);
    assert_eq!(gesture.current_width(), 360.0);
    assert!(!gesture.is_active());

    // Stray cancel returns None
    assert!(gesture.cancel().is_none());
}

#[test]
fn test_resize_gesture_already_open_refusal() {
    let mut gesture = SideSheetResize::new(SheetPlacement::Right);
    let start1 = gesture.begin(360.0, 500.0);
    assert_eq!(start1, GestureStart::Started);

    // Second begin is refused and keeps original snapshot
    let start2 = gesture.begin(400.0, 600.0);
    assert_eq!(start2, GestureStart::AlreadyOpen);
    assert_eq!(gesture.start_width(), 360.0);

    // Cancel restores original snapshot (360.0)
    let cancel = gesture.cancel().expect("should cancel");
    assert_eq!(*cancel.restored(), 360.0);
}

// ---------------------------------------------------------------------------
// 3. Constants and Token Consistency Tests
// ---------------------------------------------------------------------------

#[test]
fn test_side_sheet_constants() {
    assert_eq!(DEFAULT_WIDTH, 360.0);
    assert_eq!(MIN_WIDTH, 280.0);
    assert_eq!(MAX_WIDTH, 520.0);
    assert_eq!(HEADER_HEIGHT, size::CONTROL_LG);
    assert_eq!(FOOTER_HEIGHT, 48.0);
    assert_eq!(GRAB_WIDTH, size::SPLITTER_GRAB);
    assert_eq!(GRIP_PILL_WIDTH, space::XXS);
    assert_eq!(GRIP_PILL_HEIGHT, size::HIT_TARGET_MIN);
    assert_eq!(RESPONSIVE_BREAKPOINT, 800.0);
    assert_eq!(GRIP_PILL_RADIUS, shape::RADIUS_PILL);
    assert_eq!(DIVIDER_STROKE, shape::BORDER_HAIRLINE);
    assert_eq!(HEADER_BUTTON_SIZE, size::HIT_TARGET_MIN);
    assert_eq!(BODY_PADDING, space::LG);

    assert_eq!(DEFAULT_TITLE, "Inspector");
    assert_eq!(GLYPH_PINNED, "📌");
    assert_eq!(GLYPH_UNPINNED, "📍");
    assert_eq!(GLYPH_CLOSE, "×");
    assert_eq!(LABEL_PIN, "Pin");
    assert_eq!(LABEL_UNPIN, "Unpin");
    assert_eq!(LABEL_CLOSE, "Close");
}

#[test]
fn test_side_sheet_contrast_ratios() {
    let palette = Palette::dark();

    // 1. Grip pill border.strong on surface.base must clear active indicator floor (>= 3.0:1)
    let pill_contrast = contrast_ratio(palette.border_strong, palette.surface_base);
    assert!(
        pill_contrast >= 3.0,
        "grip pill contrast ({pill_contrast:.2}:1) must clear 3.0:1 on surface.base"
    );

    // 2. Promoted grip pill accent.border on surface.base must clear 3.0:1
    let promoted_pill = contrast_ratio(palette.accent_border, palette.surface_base);
    assert!(
        promoted_pill >= 3.0,
        "promoted grip pill contrast ({promoted_pill:.2}:1) must clear 3.0:1"
    );

    // 3. Title text.strong on surface.raised must clear body text contrast floor (>= 4.5:1)
    let title_contrast = contrast_ratio(palette.text_strong, palette.surface_raised);
    assert!(
        title_contrast >= 4.5,
        "title contrast ({title_contrast:.2}:1) must clear 4.5:1 on surface.raised"
    );
}

// ---------------------------------------------------------------------------
// 4. Widget Architecture and Layout Tests
// ---------------------------------------------------------------------------

#[test]
fn test_side_sheet_widget_construction_and_getters() {
    let model = SideSheetModel::new("Widget Inspector").with_width(380.0);
    let sheet: SideSheet<'_, ()> = side_sheet(model.clone(), iced::widget::text("Body content"));

    assert_eq!(sheet.model(), &model);
    assert_eq!(sheet.width(), 380.0);
    assert!(sheet.is_open());
}

#[test]
fn test_side_sheet_widget_size_open_vs_closed() {
    let open_model = SideSheetModel::new("Open")
        .with_width(360.0)
        .with_open(true);
    let open_sheet: SideSheet<'_, ()> = side_sheet(open_model, iced::widget::text("Body"));
    let size_open = open_sheet.size();
    assert_eq!(size_open.width, Length::Fixed(360.0));
    assert_eq!(size_open.height, Length::Fill);

    let closed_model = SideSheetModel::new("Closed")
        .with_width(360.0)
        .with_open(false);
    let closed_sheet: SideSheet<'_, ()> = side_sheet(closed_model, iced::widget::text("Body"));
    let size_closed = closed_sheet.size();
    assert_eq!(size_closed.width, Length::Fixed(0.0));
    assert_eq!(size_closed.height, Length::Fixed(0.0));
}

#[test]
fn test_side_sheet_widget_headless_render_and_layout_geometry() {
    // (case label, model, footer, expected node width, expected child nodes)
    let cases = [
        (
            "open docked",
            SideSheetModel::new("Headless Sheet").with_width(360.0),
            false,
            DEFAULT_WIDTH,
            1,
        ),
        (
            "modeless overlay with footer, left placement",
            SideSheetModel::new("Footer Sheet")
                .with_width(400.0)
                .with_mode(SheetMode::ModelessOverlay)
                .with_placement(SheetPlacement::Left),
            true,
            400.0,
            2,
        ),
        (
            "modal scrim, busy and dirty",
            SideSheetModel::new("Modal Scrim Sheet")
                .with_width(500.0)
                .with_mode(SheetMode::ModalScrim)
                .with_busy(true)
                .with_dirty(true),
            false,
            500.0,
            1,
        ),
        (
            "closed",
            SideSheetModel::new("Closed Sheet")
                .with_width(360.0)
                .with_open(false),
            false,
            0.0,
            0,
        ),
    ];

    let renderer = headless_renderer();
    let limits = layout::Limits::new(Size::ZERO, Size::new(1200.0, 400.0));

    for (label, model, with_footer, expected_width, expected_children) in cases {
        let mut sheet: SideSheet<'_, ()> =
            side_sheet(model.clone(), iced::widget::text("Body Content"));
        if with_footer {
            sheet = sheet.footer(iced::widget::text("Action Buttons"));
        }

        let mut tree = Tree::new(&sheet as &dyn Widget<(), iced::Theme, iced::Renderer>);
        let node = sheet.layout(&mut tree, &renderer, &limits);

        assert_eq!(
            node.size().width,
            expected_width,
            "[{label}] the layout node must be as wide as the model allows"
        );
        assert_eq!(
            node.children().len(),
            expected_children,
            "[{label}] the layout must hand out one node per child element"
        );
        if expected_children == 0 {
            assert_eq!(
                node.size(),
                Size::ZERO,
                "[{label}] a closed sheet must take up no space at all"
            );
        } else {
            assert_eq!(
                node.size().height,
                400.0,
                "[{label}] the sheet fills the height it is offered"
            );
            let content = node.children()[0].bounds();
            assert_eq!(
                content.x, BODY_PADDING,
                "[{label}] the body must be padded in from the sheet edge"
            );
            assert_eq!(
                content.y,
                HEADER_HEIGHT + BODY_PADDING,
                "[{label}] the body must be padded below the header"
            );
        }

        // The full render path must survive every mode combination.
        let mut harness = Harness::<(), _>::headless(move || {
            side_sheet(model.clone(), iced::widget::text("Body Content")).into()
        });
        harness.render();
    }
}

#[test]
fn test_side_sheet_widget_tag_and_state() {
    let sheet: SideSheet<'_, ()> = side_sheet(
        SideSheetModel::new("State Test"),
        iced::widget::text("Body"),
    );
    let tag = sheet.tag();
    assert_eq!(
        tag,
        iced::advanced::widget::tree::Tag::of::<SideSheetState>()
    );
    let state = sheet.state();
    let downcasted = state.downcast_ref::<SideSheetState>();
    assert!(!downcasted.resize.is_active());
    assert!(!downcasted.is_hovered_grab);
    assert!(!downcasted.is_hovered_pin);
    assert!(!downcasted.is_hovered_close);
}

#[test]
fn test_side_sheet_element_conversion() {
    let model = SideSheetModel::new("Convert");
    let sheet: SideSheet<'_, ()> = side_sheet(model, iced::widget::text("Body"));
    let _element: Element<'_, ()> = sheet.into();
}

#[test]
fn test_side_sheet_children_and_diff_track_the_footer() {
    let plain: SideSheet<'_, ()> =
        side_sheet(SideSheetModel::new("Plain"), iced::widget::text("Body"));
    let mut plain_tree = Tree::new(&plain as &dyn Widget<(), iced::Theme, iced::Renderer>);
    assert_eq!(
        plain.children().len(),
        1,
        "a sheet without a footer owns exactly one child element"
    );
    assert_eq!(
        plain_tree.children.len(),
        1,
        "the tree must be seeded with one child state"
    );
    plain.diff(&mut plain_tree);
    assert_eq!(
        plain_tree.children.len(),
        1,
        "diffing an unchanged sheet must not grow the tree"
    );

    let footed: SideSheet<'_, ()> =
        side_sheet(SideSheetModel::new("Footed"), iced::widget::text("Body"))
            .footer(iced::widget::text("Actions"));
    let mut footed_tree = Tree::new(&footed as &dyn Widget<(), iced::Theme, iced::Renderer>);
    assert_eq!(
        footed.children().len(),
        2,
        "a sheet with a footer owns a body and a footer element"
    );
    footed.diff(&mut footed_tree);
    assert_eq!(
        footed_tree.children.len(),
        2,
        "diffing must keep both child states"
    );
}

#[test]
fn test_side_sheet_state_carries_placement_into_resize() {
    let left = SideSheetState::new(SheetPlacement::Left);
    assert_eq!(
        left.resize.placement(),
        SheetPlacement::Left,
        "the resize gesture must inherit the sheet placement"
    );
    assert!(!left.resize.is_active());
    assert!(!left.is_hovered_grab);
    assert!(!left.is_hovered_pin);
    assert!(!left.is_hovered_close);

    assert_eq!(
        SideSheetState::default().resize.placement(),
        SheetPlacement::Right,
        "the default state must default to the right-hand drag direction"
    );
}

// ---------------------------------------------------------------------------
// 5. Resize Gesture Edge Cases (UIF-PRIN-006)
// ---------------------------------------------------------------------------

#[test]
fn test_side_sheet_resize_idle_gesture_emits_nothing() {
    let mut gesture = SideSheetResize::new(SheetPlacement::Left);

    assert!(
        gesture.update(400.0).is_none(),
        "a tick with no open gesture must emit no live value"
    );
    assert!(
        gesture.commit().is_none(),
        "committing an idle gesture must emit nothing"
    );
    assert!(
        gesture.cancel().is_none(),
        "cancelling an idle gesture must emit nothing"
    );
    assert!(!gesture.is_active(), "the slot must stay closed");
}

#[test]
fn test_side_sheet_resize_begin_clamps_start_width() {
    // (case label, requested start width, expected clamped start width)
    let cases = [
        ("past maximum", 900.0, MAX_WIDTH),
        ("past minimum", 100.0, MIN_WIDTH),
    ];

    for (label, requested, expected) in cases {
        let mut gesture = SideSheetResize::new(SheetPlacement::Right);
        assert_eq!(
            gesture.begin(requested, requested),
            GestureStart::Started,
            "[{label}] begin must open the slot"
        );
        assert_eq!(
            gesture.start_width(),
            expected,
            "[{label}] the captured start width must be clamped into the spec range"
        );
        assert_eq!(
            gesture.current_width(),
            expected,
            "[{label}] the live width must start at the clamped width"
        );

        let live = gesture
            .update(requested)
            .unwrap_or_else(|| panic!("[{label}] an open slot must emit a live value"));
        assert_eq!(
            *live.value(),
            expected,
            "[{label}] a zero-travel tick must report the clamped width"
        );
    }
}

// ---------------------------------------------------------------------------
// 6. Direct Widget::update Event Tests
// ---------------------------------------------------------------------------

/// Host message emitted by the driven sheet.
#[derive(Debug, Clone, PartialEq)]
enum TestMsg {
    /// Live resize width.
    ResizeLive(f32),
    /// Settled resize width.
    ResizeCommit(f32),
    /// Reverted resize width.
    ResizeCancel(f32),
    /// The pin button requested a mode change.
    Mode(SheetMode),
    /// Dismissal was requested.
    CloseRequested,
    /// The body element received a press the sheet did not claim.
    ContentPressed,
}

/// A body element that reports what the sheet forwards to it.
///
/// It fills whatever box the sheet offers, so a test can press anywhere in the
/// body without depending on how wide a glyph measured.
struct Probe;

impl Widget<TestMsg, iced::Theme, iced::Renderer> for Probe {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::Node::new(limits.max())
    }

    fn update(
        &mut self,
        _tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, TestMsg>,
        _viewport: &Rectangle,
    ) {
        if matches!(
            event,
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
        ) && cursor.is_over(layout.bounds())
        {
            shell.publish(TestMsg::ContentPressed);
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Grabbing
        } else {
            mouse::Interaction::None
        }
    }

    fn draw(
        &self,
        _tree: &Tree,
        _renderer: &mut iced::Renderer,
        _theme: &iced::Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
    }
}

impl From<Probe> for Element<'_, TestMsg, iced::Theme, iced::Renderer> {
    fn from(widget: Probe) -> Self {
        Element::new(widget)
    }
}

/// Build a `KeyPressed` event for a named key.
fn key_event(named: Named) -> Event {
    let key = Key::Named(named);
    Event::Keyboard(iced::keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: iced::keyboard::key::Physical::Unidentified(
            iced::keyboard::key::NativeCode::Unidentified,
        ),
        location: iced::keyboard::Location::Standard,
        modifiers: iced::keyboard::Modifiers::default(),
        repeat: false,
        text: None,
    })
}

/// Drives [`SideSheet`] through the real `Widget::update` entry point so a test
/// can read the widget's internal state back out of its [`Tree`].
struct SheetDriver<'a> {
    widget: SideSheet<'a, TestMsg>,
    tree: Tree,
    renderer: iced::Renderer,
    node: layout::Node,
    offset: Vector,
    bounds: Rectangle,
    messages: Vec<TestMsg>,
}

impl<'a> SheetDriver<'a> {
    /// Lay `widget` out at `offset` and keep the resulting node for every event.
    fn build(mut widget: SideSheet<'a, TestMsg>, offset: Vector) -> Self {
        let mut tree = Tree::new(&widget as &dyn Widget<TestMsg, iced::Theme, iced::Renderer>);
        let renderer = headless_renderer();
        let limits = layout::Limits::new(Size::ZERO, Size::new(1200.0, 400.0));
        let node = widget.layout(&mut tree, &renderer, &limits);
        let bounds = Rectangle::new(Point::new(offset.x, offset.y), node.size());
        Self {
            widget,
            tree,
            renderer,
            node,
            offset,
            bounds,
            messages: Vec::new(),
        }
    }

    /// Attach every host callback so the sheet's live, settled, reverted, mode
    /// and dismissal messages are all observable from a test.
    fn wired(sheet: SideSheet<'a, TestMsg>) -> SideSheet<'a, TestMsg> {
        sheet
            .on_resize_change(|live| TestMsg::ResizeLive(*live.value()))
            .on_resize_commit(|commit| TestMsg::ResizeCommit(*commit.value()))
            .on_resize_cancel(|cancel| TestMsg::ResizeCancel(*cancel.restored()))
            .on_mode_toggle(|commit| TestMsg::Mode(*commit.value()))
            .on_close_request(|_| TestMsg::CloseRequested)
    }

    /// A right-docked sheet whose body is a [`Probe`].
    fn right(model: SideSheetModel) -> Self {
        let model = model.with_placement(SheetPlacement::Right);
        Self::build(
            Self::wired(side_sheet(model, Probe)),
            Vector::new(600.0, 20.0),
        )
    }

    /// A left-docked sheet whose body is a [`Probe`].
    fn left(model: SideSheetModel) -> Self {
        let model = model.with_placement(SheetPlacement::Left);
        Self::build(
            Self::wired(side_sheet(model, Probe)),
            Vector::new(20.0, 20.0),
        )
    }

    /// A point on the resize edge, which sits outside the sheet for a
    /// right-docked drawer and on its inner edge for a left-docked one.
    fn grab_point(&self) -> Point {
        let x = match self.widget.model().placement {
            SheetPlacement::Right => self.bounds.x,
            SheetPlacement::Left => self.bounds.x + self.bounds.width - 1.0,
        };
        Point::new(x, self.bounds.y + 200.0)
    }

    /// Centre of the `[Pin/Unpin]` button.
    fn pin_point(&self) -> Point {
        Point::new(
            self.bounds.x + self.bounds.width - space::MD - HEADER_BUTTON_SIZE * 1.5 - space::XS,
            self.bounds.y + (HEADER_HEIGHT - HEADER_BUTTON_SIZE) / 2.0 + HEADER_BUTTON_SIZE / 2.0,
        )
    }

    /// Centre of the `[Close ×]` button.
    fn close_point(&self) -> Point {
        Point::new(
            self.bounds.x + self.bounds.width - space::MD - HEADER_BUTTON_SIZE / 2.0,
            self.bounds.y + (HEADER_HEIGHT - HEADER_BUTTON_SIZE) / 2.0 + HEADER_BUTTON_SIZE / 2.0,
        )
    }

    /// A point inside the padded body, clear of the header and the grab band.
    fn body_point(&self) -> Point {
        Point::new(
            self.bounds.x + BODY_PADDING + 20.0,
            self.bounds.y + HEADER_HEIGHT + BODY_PADDING + 20.0,
        )
    }

    /// Deliver `event` with the pointer at `cursor`; report whether the widget
    /// claimed the event.
    fn drive(&mut self, event: &Event, cursor: mouse::Cursor) -> bool {
        let Self {
            widget,
            tree,
            renderer,
            node,
            offset,
            bounds,
            messages,
        } = self;

        let layout = Layout::with_offset(*offset, node);
        let mut clipboard = iced::advanced::clipboard::Null;
        let mut shell = Shell::new(messages);
        widget.update(
            tree,
            event,
            layout,
            cursor,
            renderer,
            &mut clipboard,
            &mut shell,
            bounds,
        );
        shell.is_event_captured()
    }

    /// Left button press with the pointer at `at`.
    fn press(&mut self, at: Point) -> bool {
        self.drive(
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            mouse::Cursor::Available(at),
        )
    }

    /// Left button release with the pointer at `at`.
    fn release(&mut self, at: Point) -> bool {
        self.drive(
            &Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)),
            mouse::Cursor::Available(at),
        )
    }

    /// Pointer move with the pointer at `at`.
    fn hover(&mut self, at: Point) -> bool {
        self.drive(
            &Event::Mouse(mouse::Event::CursorMoved { position: at }),
            mouse::Cursor::Available(at),
        )
    }

    /// Key press with no pointer position reported to the widget.
    fn key(&mut self, named: Named) -> bool {
        self.drive(&key_event(named), mouse::Cursor::Unavailable)
    }

    /// The widget's live internal state.
    fn state(&self) -> &SideSheetState {
        self.tree.state.downcast_ref::<SideSheetState>()
    }

    /// Take every message emitted so far.
    fn take_messages(&mut self) -> Vec<TestMsg> {
        std::mem::take(&mut self.messages)
    }

    /// Ask the widget what cursor shape it wants at `at`.
    fn interaction(&self, at: Point) -> mouse::Interaction {
        let layout = Layout::with_offset(self.offset, &self.node);
        self.widget.mouse_interaction(
            &self.tree,
            layout,
            mouse::Cursor::Available(at),
            &self.bounds,
            &self.renderer,
        )
    }
}

#[test]
fn test_right_docked_resize_drag_commits() {
    let mut driver = SheetDriver::right(SideSheetModel::new("Inspector").with_width(360.0));
    let start = driver.grab_point();

    assert!(
        driver.press(start),
        "a press on the resize edge must claim the event"
    );
    assert!(
        driver.state().resize.is_active(),
        "the gesture slot must open on the edge press"
    );
    assert!(
        driver.state().is_hovered_grab,
        "the grab band must report itself hovered"
    );

    // Dragging a right-docked sheet leftwards widens it: 360 - (-20) = 380.
    assert!(driver.hover(Point::new(start.x - 20.0, start.y)));
    assert_eq!(
        driver.take_messages(),
        vec![TestMsg::ResizeLive(380.0)],
        "the drag must emit the live width"
    );

    assert!(driver.release(Point::new(start.x - 20.0, start.y)));
    assert_eq!(
        driver.take_messages(),
        vec![TestMsg::ResizeCommit(380.0)],
        "release must commit the settled width exactly once"
    );
    assert!(
        !driver.state().resize.is_active(),
        "the gesture slot must close after commit"
    );
}

#[test]
fn test_left_docked_resize_drag_grows_rightwards() {
    let mut driver = SheetDriver::left(SideSheetModel::new("Inspector").with_width(360.0));
    let start = driver.grab_point();

    assert!(driver.press(start), "the left edge press must be claimed");
    assert_eq!(
        driver.state().resize.placement(),
        SheetPlacement::Left,
        "the gesture must inherit the placement on every event"
    );

    // Dragging a left-docked sheet rightwards widens it: 360 + 20 = 380.
    assert!(driver.hover(Point::new(start.x + 20.0, start.y)));
    assert_eq!(
        driver.take_messages(),
        vec![TestMsg::ResizeLive(380.0)],
        "a left-docked drag must grow the sheet towards the workspace"
    );

    assert!(driver.release(Point::new(start.x + 20.0, start.y)));
    assert_eq!(
        driver.take_messages(),
        vec![TestMsg::ResizeCommit(380.0)],
        "release must commit the settled width"
    );
}

#[test]
fn test_escape_cancels_active_resize() {
    let mut driver = SheetDriver::right(SideSheetModel::new("Inspector").with_width(360.0));
    let start = driver.grab_point();

    assert!(driver.press(start));
    assert!(driver.hover(Point::new(start.x - 40.0, start.y)));
    driver.take_messages();

    assert!(
        driver.key(Named::Escape),
        "Escape must claim the event while a resize is open"
    );
    assert_eq!(
        driver.take_messages(),
        vec![TestMsg::ResizeCancel(360.0)],
        "Escape must restore the pre-drag width"
    );
    assert!(!driver.state().resize.is_active());
    assert!(
        !driver.state().is_hovered_grab,
        "cancel with no pointer over the band must drop the grab hover"
    );
}

#[test]
fn test_escape_requests_close_only_when_the_sheet_can_close() {
    // (case label, model, expected message)
    let cases = [
        (
            "clean sheet",
            SideSheetModel::new("Clean"),
            Some(TestMsg::CloseRequested),
        ),
        (
            "dirty sheet needs confirmation",
            SideSheetModel::new("Dirty").with_dirty(true),
            None,
        ),
        (
            "busy sheet refuses dismissal",
            SideSheetModel::new("Busy").with_busy(true),
            None,
        ),
    ];

    for (label, model, expected) in cases {
        let mut driver = SheetDriver::right(model);
        let captured = driver.key(Named::Escape);
        let messages = driver.take_messages();
        assert_eq!(
            messages.first().cloned(),
            expected,
            "[{label}] Escape emitted the wrong message"
        );
        assert_eq!(
            captured,
            expected.is_some(),
            "[{label}] Escape must only be claimed when dismissal was requested"
        );
    }
}

#[test]
fn test_close_button_requests_close_unless_busy() {
    // (case label, model, expected message)
    let cases = [
        (
            "idle sheet",
            SideSheetModel::new("Clean"),
            Some(TestMsg::CloseRequested),
        ),
        (
            "busy sheet ignores the close button",
            SideSheetModel::new("Busy").with_busy(true),
            None,
        ),
    ];

    for (label, model, expected) in cases {
        let mut driver = SheetDriver::right(model);
        let point = driver.close_point();
        let captured = driver.press(point);
        let messages = driver.take_messages();
        assert_eq!(
            messages.first().cloned(),
            expected,
            "[{label}] the close button emitted the wrong message"
        );
        assert_eq!(
            captured,
            expected.is_some(),
            "[{label}] the close press must only be claimed when it fired"
        );
        assert!(
            driver.state().is_hovered_close,
            "[{label}] hovering the close button must be tracked"
        );
    }
}

#[test]
fn test_pin_button_commits_toggled_mode() {
    // (case label, configured mode, expected commit)
    let cases = [
        (
            "docked unpins",
            SheetMode::Docked,
            SheetMode::ModelessOverlay,
        ),
        (
            "modeless overlay pins",
            SheetMode::ModelessOverlay,
            SheetMode::Docked,
        ),
        (
            "modal sheet docks",
            SheetMode::ModalScrim,
            SheetMode::Docked,
        ),
    ];

    for (label, mode, expected) in cases {
        let mut driver = SheetDriver::right(SideSheetModel::new("Inspector").with_mode(mode));
        let point = driver.pin_point();
        assert!(
            driver.press(point),
            "[{label}] a press on the pin button must claim the event"
        );
        assert_eq!(
            driver.take_messages(),
            vec![TestMsg::Mode(expected)],
            "[{label}] the pin button must commit the toggled mode"
        );
        assert!(
            driver.state().is_hovered_pin,
            "[{label}] hovering the pin button must be tracked"
        );
    }
}

#[test]
fn test_header_buttons_without_callbacks_emit_nothing() {
    let model = SideSheetModel::new("No Callbacks");
    let mut driver = SheetDriver::build(side_sheet(model, Probe), Vector::new(600.0, 20.0));

    driver.press(driver.pin_point());
    driver.press(driver.close_point());
    driver.key(Named::Escape);

    assert!(
        driver.take_messages().is_empty(),
        "a sheet with no host callbacks must not invent messages"
    );
}

#[test]
fn test_body_events_forward_to_the_content_element() {
    let mut driver = SheetDriver::right(SideSheetModel::new("Inspector"));
    let body = driver.body_point();

    driver.press(body);
    assert_eq!(
        driver.take_messages(),
        vec![TestMsg::ContentPressed],
        "a press inside the body must reach the content element"
    );
}

#[test]
fn test_closed_sheet_handles_no_events_at_all() {
    let mut driver = SheetDriver::right(
        SideSheetModel::new("Closed")
            .with_width(360.0)
            .with_open(false),
    );

    assert!(
        !driver.press(driver.grab_point()),
        "a closed sheet must not claim an edge press"
    );
    assert!(
        !driver.press(driver.close_point()),
        "a closed sheet must not claim a close press"
    );
    assert!(
        !driver.press(driver.body_point()),
        "a closed sheet must not forward events to hidden content"
    );
    assert!(
        !driver.key(Named::Escape),
        "a closed sheet must not claim Escape"
    );
    assert!(
        driver.take_messages().is_empty(),
        "a closed sheet must emit nothing"
    );
    assert_eq!(
        driver.interaction(driver.grab_point()),
        mouse::Interaction::None,
        "a closed sheet must not offer a resize cursor"
    );
}

#[test]
fn test_move_and_release_without_active_resize_are_ignored() {
    let mut driver = SheetDriver::right(SideSheetModel::new("Inspector"));

    assert!(
        !driver.hover(Point::new(500.0, 200.0)),
        "a move with no open gesture must not claim the event"
    );
    assert!(
        !driver.release(Point::new(600.0, 200.0)),
        "a release with no open gesture must not claim the event"
    );
    assert!(
        driver.take_messages().is_empty(),
        "an idle sheet must not emit resize traffic"
    );
}

#[test]
fn test_mouse_interaction_per_region() {
    let driver = SheetDriver::right(SideSheetModel::new("Inspector"));

    // (case label, cursor, expected interaction)
    let cases = [
        (
            "resize edge",
            driver.grab_point(),
            mouse::Interaction::ResizingHorizontally,
        ),
        (
            "pin button",
            driver.pin_point(),
            mouse::Interaction::Pointer,
        ),
        (
            "close button",
            driver.close_point(),
            mouse::Interaction::Pointer,
        ),
        (
            "body forwards the child's own cursor",
            driver.body_point(),
            mouse::Interaction::Grabbing,
        ),
        (
            "header title area",
            Point::new(driver.bounds.x + 40.0, driver.bounds.y + 18.0),
            mouse::Interaction::None,
        ),
    ];

    for (label, at, expected) in cases {
        assert_eq!(
            driver.interaction(at),
            expected,
            "[{label}] reported the wrong cursor at {at:?}"
        );
    }
}

#[test]
fn test_busy_sheet_shows_no_pointer_over_the_close_button() {
    let driver = SheetDriver::right(SideSheetModel::new("Busy").with_busy(true));

    assert_eq!(
        driver.interaction(driver.close_point()),
        mouse::Interaction::None,
        "a busy sheet must not invite a click it will refuse"
    );
    assert_eq!(
        driver.interaction(driver.pin_point()),
        mouse::Interaction::Pointer,
        "busy must not disable the pin button's cursor"
    );
}

#[test]
fn test_mouse_interaction_keeps_resize_cursor_while_dragging() {
    let mut driver = SheetDriver::right(SideSheetModel::new("Inspector"));
    assert!(driver.press(driver.grab_point()));

    assert_eq!(
        driver.interaction(driver.body_point()),
        mouse::Interaction::ResizingHorizontally,
        "an open resize gesture must keep the resize cursor everywhere"
    );
}

#[test]
fn test_operate_forwards_focusables_only_while_open() {
    // (case label, open, expected focusable count)
    let cases = [("open sheet", true, 1usize), ("closed sheet", false, 0)];

    for (label, open, expected) in cases {
        let model = SideSheetModel::new("Inspector").with_open(open);
        let mut harness = Harness::<(), _>::headless(move || {
            side_sheet(model.clone(), iced::widget::text_input("Filter", "value")).into()
        });

        assert_eq!(
            harness.focus_count().total,
            expected,
            "[{label}] the sheet must forward focus registration to its body"
        );
    }
}
