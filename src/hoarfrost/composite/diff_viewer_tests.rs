use super::*;
use crate::binding::Commit;
use crate::token::{shape, size};

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
