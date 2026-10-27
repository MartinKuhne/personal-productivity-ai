//! Unit tests for `side_sheet` composite (`UIF-COMP-012`, `UIF-PRIN-006`, `UIF-MOD-002`).

use super::*;
use crate::binding::GestureStart;
use crate::surface::zorder::ZBand;
use crate::test_helpers::Harness;
use crate::token::color::Palette;
use crate::token::contrast::contrast_ratio;
use iced::advanced::Widget;

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
fn test_side_sheet_widget_headless_render() {
    let model = SideSheetModel::new("Headless Sheet").with_width(360.0);
    let mut harness = Harness::<(), _>::headless(move || {
        side_sheet(model.clone(), iced::widget::text("Inner Content")).into()
    });
    harness.render();
}

#[test]
fn test_side_sheet_widget_headless_render_with_footer() {
    let model = SideSheetModel::new("Footer Sheet")
        .with_width(400.0)
        .with_mode(SheetMode::ModelessOverlay)
        .with_placement(SheetPlacement::Left);
    let mut harness = Harness::<(), _>::headless(move || {
        side_sheet(model.clone(), iced::widget::text("Body Content"))
            .footer(iced::widget::text("Action Buttons"))
            .into()
    });
    harness.render();
}

#[test]
fn test_side_sheet_widget_headless_render_modal_scrim_busy() {
    let model = SideSheetModel::new("Modal Scrim Sheet")
        .with_width(500.0)
        .with_mode(SheetMode::ModalScrim)
        .with_busy(true)
        .with_dirty(true);
    let mut harness = Harness::<(), _>::headless(move || {
        side_sheet(model.clone(), iced::widget::text("Modal Body")).into()
    });
    harness.render();
}

#[test]
fn test_side_sheet_widget_headless_render_closed() {
    let model = SideSheetModel::new("Closed Sheet")
        .with_width(360.0)
        .with_open(false);
    let mut harness = Harness::<(), _>::headless(move || {
        side_sheet(model.clone(), iced::widget::text("Hidden Content")).into()
    });
    harness.render();
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
