//! Unit tests for the context-menu row model, grouping, identity and
//! pointer-anchored card geometry.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use iced::{Point, Size};

use iced_pro::surface::menu::model::{MenuId, MenuViolation, RowKind};
use iced_pro::token::space;

use crate::config::SkillFile;
use crate::ui::strings;
use crate::ui_iced::context_menu::*;
use crate::ui_iced::message::{MenuMessage, Message};
use crate::ui_iced::panels::{center, left};

/// A stand-in intent for rows whose message a test does not care about.
///
/// `InteractionIgnored` is genuinely inert in the reducer, so a test that
/// only inspects grouping cannot accidentally drive the app.
fn message_noop() -> Message {
    Message::Menu(MenuMessage::InteractionIgnored)
}

/// A skill file with a recognisable name and path.
fn skill(name: &str) -> SkillFile {
    SkillFile {
        name: name.to_owned(),
        path: PathBuf::from(format!(r"C:\work\Skills\{name}.md")),
    }
}

fn dir_path() -> PathBuf {
    PathBuf::from(r"C:\work\docs")
}

fn file_path() -> PathBuf {
    PathBuf::from(r"C:\work\docs\note.md")
}

/// The four §4.9.1 targets, each with one skill present so the automation
/// group is never empty.
fn all_targets() -> Vec<(ContextTarget, Vec<ContextRow>)> {
    let skills = [skill("FormatMarkdown")];
    vec![
        (
            ContextTarget::TreeDirectory { path: dir_path() },
            left::directory_menu_items(&dir_path(), &skills),
        ),
        (
            ContextTarget::TreeFile { path: file_path() },
            left::file_menu_items(&file_path(), &skills, true),
        ),
        (
            ContextTarget::TreeMultiSelect {
                paths: vec![file_path(), dir_path()],
            },
            left::multi_select_menu_items(&[file_path(), dir_path()]),
        ),
        (
            ContextTarget::Tab {
                index: 2,
                path: file_path(),
            },
            center::tab_context_menu_items(2, &file_path(), &skills),
        ),
    ]
}

/// The group sequence a row list declares, separators excluded.
fn group_sequence(rows: &[ContextRow]) -> Vec<ContextGroup> {
    rows.iter().map(|row| row.group).collect()
}

#[test]
fn test_menu_from_rows_inserts_one_separator_per_group_change() {
    let rows = [
        ContextRow::new("a", "A", ContextGroup::Primary, message_noop()),
        ContextRow::new("b", "B", ContextGroup::Primary, message_noop()),
        ContextRow::new("c", "C", ContextGroup::Management, message_noop()),
    ];
    let menu = menu_from_rows(MenuId::new("t"), &rows);

    let separators: Vec<usize> = menu
        .rows()
        .iter()
        .enumerate()
        .filter(|(_, row)| row.is_separator())
        .map(|(index, _)| index)
        .collect();
    assert_eq!(
        separators,
        vec![2],
        "two runs of rows means exactly one boundary, and it sits between the runs"
    );
    assert_eq!(menu.rows().len(), 4, "3 rows + 1 separator");
}

#[test]
fn test_menu_from_rows_never_leads_or_trails_with_a_separator() {
    let rows = [
        ContextRow::new("a", "A", ContextGroup::Primary, message_noop()),
        ContextRow::new("b", "B", ContextGroup::Destructive, message_noop()),
        ContextRow::new("c", "C", ContextGroup::Destructive, message_noop()),
    ];
    let menu = menu_from_rows(MenuId::new("t"), &rows);
    assert!(!menu.rows()[0].is_separator());
    assert!(!menu.rows()[menu.rows().len() - 1].is_separator());
}

#[test]
fn test_menu_from_rows_marks_only_the_destructive_group_destructive() {
    let rows = [
        ContextRow::new("a", "A", ContextGroup::Primary, message_noop()),
        ContextRow::new("d", "D", ContextGroup::Destructive, message_noop()),
    ];
    let menu = menu_from_rows(MenuId::new("t"), &rows);
    assert!(!menu.rows()[0].is_destructive());
    assert!(menu.rows()[2].is_destructive());
}

#[test]
fn test_groups_report_the_declared_runs() {
    let rows = [
        ContextRow::new("a", "A", ContextGroup::Primary, message_noop()),
        ContextRow::new("b", "B", ContextGroup::Primary, message_noop()),
        ContextRow::new("c", "C", ContextGroup::Management, message_noop()),
    ];
    let groups = menu_from_rows(MenuId::new("t"), &rows).groups();
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].interactive.len(), 2);
    assert_eq!(groups[1].interactive.len(), 1);
    assert!(groups[0].destructive.is_empty());
}

#[test]
fn test_every_shipped_target_validates_clean() {
    for (target, rows) in all_targets() {
        let menu = menu_from_rows(root_id(&target), &rows);
        assert_eq!(
            menu.validate(),
            Vec::<MenuViolation>::new(),
            "{target:?} must satisfy every §4.9.1 structural rule"
        );
    }
}

#[test]
fn test_every_shipped_target_keeps_destructive_rows_in_the_final_group() {
    for (target, rows) in all_targets() {
        let menu = menu_from_rows(root_id(&target), &rows);
        let groups = menu.groups();
        let destructive_groups: Vec<usize> = groups
            .iter()
            .enumerate()
            .filter(|(_, group)| !group.destructive.is_empty())
            .map(|(index, _)| index)
            .collect();
        if destructive_groups.is_empty() {
            continue;
        }
        assert_eq!(
            destructive_groups,
            vec![groups.len() - 1],
            "{target:?} puts destructive rows only in the last group"
        );
    }
}

#[test]
fn test_row_keys_are_unique_within_every_target() {
    for (target, rows) in all_targets() {
        let keys: HashSet<&str> = rows.iter().map(|row| row.key.as_str()).collect();
        assert_eq!(
            keys.len(),
            rows.len(),
            "{target:?} reuses a row key, which would collide on both the \
             focus id and the routing table: {:?}",
            rows.iter().map(|row| row.key.as_str()).collect::<Vec<_>>()
        );
    }
}

#[test]
fn test_routes_cover_every_row_key_and_only_those_keys() {
    for (target, rows) in all_targets() {
        let routes = routes_from_rows(&rows);
        assert_eq!(routes.len(), rows.len(), "{target:?} route table size");
        for row in &rows {
            assert_eq!(
                routes.get(&row.key),
                Some(&row.message),
                "{target:?} row {:?} must route to its own message",
                row.key
            );
        }
    }
}

#[test]
fn test_root_id_is_stable_for_the_same_target() {
    let a = root_id(&ContextTarget::TreeFile { path: file_path() });
    let b = root_id(&ContextTarget::TreeFile { path: file_path() });
    assert_eq!(a, b);
}

#[test]
fn test_root_id_differs_across_every_target_so_a_re_open_is_not_refused() {
    let ids: Vec<MenuId> = all_targets()
        .iter()
        .map(|(target, _)| root_id(target))
        .collect();
    let unique: HashSet<MenuId> = ids.iter().cloned().collect();
    assert_eq!(
        unique.len(),
        ids.len(),
        "each of the four §4.9.1 targets must own a distinct root id, otherwise \
         MenuCascade::open would answer AlreadyOpen and the card would stay \
         anchored at the previous target: {ids:?}"
    );
}

#[test]
fn test_root_id_separates_a_directory_from_a_file_at_the_same_path() {
    let path = file_path();
    assert_ne!(
        root_id(&ContextTarget::TreeDirectory { path: path.clone() }),
        root_id(&ContextTarget::TreeFile { path }),
        "the same path under two different target kinds is two different menus"
    );
}

#[test]
fn test_skill_row_keys_carry_the_skill_path_and_land_in_automation() {
    let file = skill("FormatMarkdown");
    let row = ContextRow::skill(&file, None, Path::new(r"C:\work\docs\note.md"));
    assert_eq!(row.group, ContextGroup::Automation);
    assert_eq!(row.label, "FormatMarkdown");
    assert!(
        row.key.starts_with(SKILL_KEY_PREFIX),
        "a skill row is keyed on its own path so it cannot collide with a fixed row"
    );
    assert!(
        row.key.contains("FormatMarkdown"),
        "the key must name the skill file it came from: {}",
        row.key
    );
}

#[test]
fn test_two_skills_never_share_a_row_key() {
    let a = ContextRow::skill(&skill("Alpha"), None, Path::new("x.md"));
    let b = ContextRow::skill(&skill("Beta"), None, Path::new("x.md"));
    assert_ne!(a.key, b.key);
}

#[test]
fn test_only_the_destructive_group_is_destructive() {
    assert!(ContextGroup::Destructive.is_destructive());
    for group in [
        ContextGroup::Primary,
        ContextGroup::Automation,
        ContextGroup::Management,
    ] {
        assert!(!group.is_destructive(), "{group:?} must not be destructive");
    }
}

#[test]
fn test_pointer_anchor_opens_just_below_and_right_of_the_pointer() {
    let menu = menu_from_rows(
        MenuId::new("t"),
        &[ContextRow::new(
            "a",
            "A",
            ContextGroup::Primary,
            message_noop(),
        )],
    );
    let point = Point::new(100.0, 100.0);
    let placement = root_placement(&menu, point, Size::new(1200.0, 800.0));

    assert_eq!(placement.origin.x, point.x + space::XXS);
    assert_eq!(placement.origin.y, point.y + space::XXS);
    assert!(
        !placement.was_flipped(),
        "a card with room on both axes must not flip"
    );
}

#[test]
fn test_pointer_anchor_flips_to_the_leading_side_at_the_right_edge() {
    let menu = menu_from_rows(
        MenuId::new("t"),
        &[ContextRow::new(
            "a",
            "A",
            ContextGroup::Primary,
            message_noop(),
        )],
    );
    let card = card_size(&menu);
    let viewport = Size::new(card.width + 60.0, 800.0);
    let point = Point::new(viewport.width - 10.0, 50.0);
    let placement = root_placement(&menu, point, viewport);

    assert!(
        placement.flipped_horizontal,
        "the preferred side cannot hold the card, so it must mirror"
    );
    assert_eq!(
        placement.origin.x,
        point.x - card.width - space::XXS,
        "the mirrored side puts the card's right edge just left of the cursor"
    );
    assert!(
        placement.origin.x > 0.0,
        "the mirrored card is still on screen: {}",
        placement.origin.x
    );
}

#[test]
fn test_pointer_anchor_flips_above_the_pointer_at_the_bottom_edge() {
    let menu = menu_from_rows(
        MenuId::new("t"),
        &[ContextRow::new(
            "a",
            "A",
            ContextGroup::Primary,
            message_noop(),
        )],
    );
    let card = card_size(&menu);
    // Roomy enough that the mirrored position is fully legal inside the
    // viewport margins: the card's bottom edge lands 2px above the cursor,
    // well clear of the bottom margin, so no clamp interferes with the rule.
    let viewport = Size::new(1200.0, card.height + 200.0);
    let point = Point::new(50.0, viewport.height - 10.0);
    let placement = root_placement(&menu, point, viewport);

    assert!(
        placement.flipped_vertical,
        "no room below the cursor, so the card opens above it"
    );
    assert_eq!(placement.origin.y, point.y - card.height - space::XXS);
    assert!(
        !placement.clamped_vertical,
        "the mirrored card fits, so the margin must not move it again"
    );
    assert!(
        placement.origin.x > 0.0,
        "the horizontal axis is untouched by a vertical flip"
    );
}

#[test]
fn test_open_card_rects_returns_one_rectangle_per_open_card() {
    let rows = [
        ContextRow::new("a", "A", ContextGroup::Primary, message_noop()),
        ContextRow::new("b", "B", ContextGroup::Management, message_noop()),
    ];
    let menu = menu_from_rows(MenuId::new("t"), &rows);
    let mut cascade = iced_pro::surface::menu::cascade::MenuCascade::new();
    let mut focus = iced_pro::focus::FocusState::new();
    let _ = cascade.open(
        &menu,
        iced_pro::surface::menu::cascade::MenuTreeKind::Context,
        &mut focus,
    );

    let rects = open_card_rects(&cascade, Point::new(30.0, 40.0), Size::new(1200.0, 800.0));
    assert_eq!(rects.len(), cascade.open_cards().len());
    assert_eq!(Size::new(rects[0].width, rects[0].height), card_size(&menu));
    assert_eq!(rects[0].x, 30.0 + space::XXS);
}

#[test]
fn test_a_target_with_no_rows_opens_nothing() {
    let menu = menu_from_rows(root_id(&ContextTarget::TreeFile { path: file_path() }), &[]);
    let mut cascade = iced_pro::surface::menu::cascade::MenuCascade::new();
    let mut focus = iced_pro::focus::FocusState::new();
    let outcome = cascade.open(
        &menu,
        iced_pro::surface::menu::cascade::MenuTreeKind::Context,
        &mut focus,
    );
    assert!(
        !cascade.is_open() || outcome.is_ignored(),
        "an empty card cannot hold a menu: {outcome:?}"
    );
}

#[test]
fn test_the_tree_file_menu_groups_match_the_spec_runs() {
    let rows = left::file_menu_items(&file_path(), &[skill("FormatMarkdown")], true);
    assert_eq!(
        group_sequence(&rows),
        vec![
            ContextGroup::Primary,
            ContextGroup::Primary,
            ContextGroup::Primary,
            ContextGroup::Automation,
            ContextGroup::Automation,
            ContextGroup::Management,
            ContextGroup::Management,
            ContextGroup::Management,
            ContextGroup::Management,
            ContextGroup::Destructive,
        ],
        "Open & Output | AI & Automation | File Management | Destructive"
    );
}

#[test]
fn test_the_directory_menu_groups_match_the_spec_runs() {
    let rows = left::directory_menu_items(&dir_path(), &[skill("FolderSkill")]);
    assert_eq!(
        group_sequence(&rows),
        vec![
            ContextGroup::Primary,
            ContextGroup::Primary,
            ContextGroup::Automation,
            ContextGroup::Automation,
            ContextGroup::Automation,
            ContextGroup::Automation,
            ContextGroup::Automation,
            ContextGroup::Destructive,
        ],
        "Creation | Folder Automation & Management | Destructive"
    );
}

#[test]
fn test_the_tab_menu_groups_match_the_spec_runs() {
    let rows = center::tab_context_menu_items(0, &file_path(), &[skill("FormatMarkdown")]);
    assert_eq!(
        group_sequence(&rows),
        vec![
            ContextGroup::Primary,
            ContextGroup::Primary,
            ContextGroup::Primary,
            ContextGroup::Automation,
            ContextGroup::Automation,
            ContextGroup::Automation,
            ContextGroup::Management,
            ContextGroup::Management,
        ],
        "Tab Lifecycle | Edit & Automation | Location"
    );
}

#[test]
fn test_the_multi_select_menu_groups_match_the_spec_runs() {
    let rows = left::multi_select_menu_items(&[file_path(), dir_path()]);
    assert_eq!(
        group_sequence(&rows),
        vec![ContextGroup::Primary, ContextGroup::Destructive]
    );
}

#[test]
fn test_labels_are_the_unchanged_strings_module_values() {
    let rows = left::file_menu_items(&file_path(), &[], true);
    let labels: Vec<&str> = rows.iter().map(|row| row.label.as_str()).collect();
    assert_eq!(
        labels,
        vec![
            strings::EDIT_BUTTON,
            strings::PRINT_ACTION,
            strings::SAVE_AS_PDF_ACTION,
            strings::RUN_AS_PROMPT_ACTION,
            strings::RENAME_ACTION,
            strings::MOVE_ACTION,
            strings::COPY_PATH_ACTION,
            strings::SHOW_IN_EXPLORER_ACTION,
            strings::DELETE_ACTION,
        ],
        "INT-3b changed row order and grouping, not a single label"
    );
}

#[test]
fn test_typst_absence_drops_the_save_as_pdf_row_rather_than_disabling_it() {
    let without = left::file_menu_items(&file_path(), &[], false);
    assert!(
        !without.iter().any(|row| row.key == KEY_SAVE_AS_PDF),
        "the row is absent, not greyed out"
    );
    assert_eq!(
        without
            .iter()
            .filter(|row| row.group == ContextGroup::Primary)
            .count(),
        2
    );
}

#[test]
fn test_row_kind_is_a_plain_action_for_every_shipped_row() {
    for (target, rows) in all_targets() {
        let menu = menu_from_rows(root_id(&target), &rows);
        for row in menu.rows() {
            if row.is_separator() {
                continue;
            }
            assert!(
                matches!(row.kind(), RowKind::Action),
                "{target:?} row {:?} must be an activatable action row",
                row.key()
            );
        }
    }
}
