//! Unit tests for the context-menu row model, grouping, identity and
//! pointer-anchored card geometry.
//!
//! Cases that share a fixture are grouped: the four §4.9.1 targets are walked
//! once by the structural test and once by the group-sequence table, and the
//! placement cases share one single-row menu. Every assertion names the target
//! or case it belongs to, so a merged failure still identifies itself.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use iced::{Point, Size};

use hoarfrost::surface::menu::model::{MenuId, MenuViolation, RowKind};
use hoarfrost::token::space;

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

/// A row list whose group changes once: two `Primary` rows then one
/// `Management` row.
fn two_run_rows() -> [ContextRow; 3] {
    [
        ContextRow::new("a", "A", ContextGroup::Primary, message_noop()),
        ContextRow::new("b", "B", ContextGroup::Primary, message_noop()),
        ContextRow::new("c", "C", ContextGroup::Management, message_noop()),
    ]
}

/// A single-row menu, the fixture every placement case anchors.
fn one_row_menu() -> hoarfrost::surface::menu::model::Menu {
    menu_from_rows(
        MenuId::new("t"),
        &[ContextRow::new(
            "a",
            "A",
            ContextGroup::Primary,
            message_noop(),
        )],
    )
}

// --- menu construction from rows -------------------------------------------

/// One row list, both structural readings of it: the separator the group
/// change produces, and the group runs the menu reports.
#[test]
fn test_menu_from_rows_separates_and_groups_the_declared_runs() {
    let rows = two_run_rows();
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
        "separators: two runs of rows means exactly one boundary, and it sits between the runs"
    );
    assert_eq!(menu.rows().len(), 4, "row count: 3 rows + 1 separator");

    let groups = menu.groups();
    assert_eq!(groups.len(), 2, "groups: two declared runs");
    assert_eq!(
        groups[0].interactive.len(),
        2,
        "groups: the first run carries its two rows"
    );
    assert_eq!(
        groups[1].interactive.len(),
        1,
        "groups: the second run carries its one row"
    );
    assert!(
        groups[0].destructive.is_empty(),
        "groups: a Primary run holds no destructive rows"
    );
}

/// Separators only ever sit *between* runs, and only the `Destructive` group
/// is marked destructive.
#[test]
fn test_menu_from_rows_never_leads_or_trails_with_a_separator_and_marks_only_destructive_rows() {
    // Boundary placement: a group change after the first row.
    let boundary = [
        ContextRow::new("a", "A", ContextGroup::Primary, message_noop()),
        ContextRow::new("b", "B", ContextGroup::Destructive, message_noop()),
        ContextRow::new("c", "C", ContextGroup::Destructive, message_noop()),
    ];
    let menu = menu_from_rows(MenuId::new("t"), &boundary);
    assert!(
        !menu.rows()[0].is_separator(),
        "boundary placement: the card must never lead with a separator"
    );
    assert!(
        !menu.rows()[menu.rows().len() - 1].is_separator(),
        "boundary placement: the card must never trail with a separator"
    );

    // Destructive marking: only the destructive group's rows carry it.
    let mixed = [
        ContextRow::new("a", "A", ContextGroup::Primary, message_noop()),
        ContextRow::new("d", "D", ContextGroup::Destructive, message_noop()),
    ];
    let menu = menu_from_rows(MenuId::new("t"), &mixed);
    assert!(
        !menu.rows()[0].is_destructive(),
        "destructive marking: a Primary row is not destructive"
    );
    assert!(
        menu.rows()[2].is_destructive(),
        "destructive marking: the Destructive row is destructive"
    );
}

/// Every shipped target satisfies every §4.9.1 structural rule at once: it
/// validates clean, keeps destructive rows in the final group, keys its rows
/// uniquely, routes every key to its own message, and carries only
/// activatable action rows.
#[test]
fn test_every_shipped_target_satisfies_the_structural_rules() {
    for (target, rows) in all_targets() {
        let menu = menu_from_rows(root_id(&target), &rows);

        assert_eq!(
            menu.validate(),
            Vec::<MenuViolation>::new(),
            "{target:?} must satisfy every §4.9.1 structural rule"
        );

        let groups = menu.groups();
        let destructive_groups: Vec<usize> = groups
            .iter()
            .enumerate()
            .filter(|(_, group)| !group.destructive.is_empty())
            .map(|(index, _)| index)
            .collect();
        if !destructive_groups.is_empty() {
            assert_eq!(
                destructive_groups,
                vec![groups.len() - 1],
                "{target:?} puts destructive rows only in the last group"
            );
        }

        let keys: HashSet<&str> = rows.iter().map(|row| row.key.as_str()).collect();
        assert_eq!(
            keys.len(),
            rows.len(),
            "{target:?} reuses a row key, which would collide on both the \
             focus id and the routing table: {:?}",
            rows.iter().map(|row| row.key.as_str()).collect::<Vec<_>>()
        );

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

// --- menu identity ---------------------------------------------------------

/// The root id is a stable function of the target, distinct across the four
/// §4.9.1 targets, and separates a directory from a file at the same path.
#[test]
fn test_root_id_is_stable_per_target_and_distinct_across_targets() {
    let a = root_id(&ContextTarget::TreeFile { path: file_path() });
    let b = root_id(&ContextTarget::TreeFile { path: file_path() });
    assert_eq!(a, b, "stability: the same target must hash to the same id");

    let ids: Vec<MenuId> = all_targets()
        .iter()
        .map(|(target, _)| root_id(target))
        .collect();
    let unique: HashSet<MenuId> = ids.iter().cloned().collect();
    assert_eq!(
        unique.len(),
        ids.len(),
        "distinctness: each of the four §4.9.1 targets must own a distinct root id, otherwise \
         MenuCascade::open would answer AlreadyOpen and the card would stay \
         anchored at the previous target: {ids:?}"
    );

    let path = file_path();
    assert_ne!(
        root_id(&ContextTarget::TreeDirectory { path: path.clone() }),
        root_id(&ContextTarget::TreeFile { path }),
        "target kind: the same path under two different target kinds is two different menus"
    );
}

// --- skill rows ------------------------------------------------------------

/// A skill row keys on its own path, lands in the automation group, and two
/// skills can never collide.
#[test]
fn test_skill_rows_are_keyed_on_their_own_path_in_the_automation_group() {
    let file = skill("FormatMarkdown");
    let row = ContextRow::skill(&file, None, Path::new(r"C:\work\docs\note.md"));
    assert_eq!(
        row.group,
        ContextGroup::Automation,
        "skill row: §4.9.1 puts contextual skills in Automation"
    );
    assert_eq!(
        row.label, "FormatMarkdown",
        "skill row: the label is the skill name"
    );
    assert!(
        row.key.starts_with(SKILL_KEY_PREFIX),
        "skill row: a skill row is keyed on its own path so it cannot collide with a fixed row"
    );
    assert!(
        row.key.contains("FormatMarkdown"),
        "skill row: the key must name the skill file it came from: {}",
        row.key
    );

    let a = ContextRow::skill(&skill("Alpha"), None, Path::new("x.md"));
    let b = ContextRow::skill(&skill("Beta"), None, Path::new("x.md"));
    assert_ne!(a.key, b.key, "two skills: two distinct keys");
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

// --- pointer-anchored geometry ---------------------------------------------

/// The card opens below-and-right of the cursor, mirrors at the right edge and
/// flips above it at the bottom edge. All three cases anchor the same
/// single-row menu.
#[test]
fn test_pointer_anchor_places_the_card_relative_to_the_cursor() {
    let menu = one_row_menu();

    // Room on both axes: just below and right of the pointer, no flip.
    let point = Point::new(100.0, 100.0);
    let placement = root_placement(&menu, point, Size::new(1200.0, 800.0));
    assert_eq!(
        placement.origin.x,
        point.x + space::XXS,
        "roomy viewport: the card opens just right of the pointer"
    );
    assert_eq!(
        placement.origin.y,
        point.y + space::XXS,
        "roomy viewport: the card opens just below the pointer"
    );
    assert!(
        !placement.was_flipped(),
        "roomy viewport: a card with room on both axes must not flip"
    );

    // Right edge: the preferred side cannot hold the card, so it mirrors.
    let card = card_size(&menu);
    let viewport = Size::new(card.width + 60.0, 800.0);
    let point = Point::new(viewport.width - 10.0, 50.0);
    let placement = root_placement(&menu, point, viewport);
    assert!(
        placement.flipped_horizontal,
        "right edge: the preferred side cannot hold the card, so it must mirror"
    );
    assert_eq!(
        placement.origin.x,
        point.x - card.width - space::XXS,
        "right edge: the mirrored side puts the card's right edge just left of the cursor"
    );
    assert!(
        placement.origin.x > 0.0,
        "right edge: the mirrored card is still on screen: {}",
        placement.origin.x
    );

    // Bottom edge: no room below, so the card opens above the pointer.
    // The viewport is roomy enough that the mirrored position is fully legal
    // inside the viewport margins: the card's bottom edge lands 2px above the
    // cursor, well clear of the bottom margin, so no clamp interferes.
    let viewport = Size::new(1200.0, card.height + 200.0);
    let point = Point::new(50.0, viewport.height - 10.0);
    let placement = root_placement(&menu, point, viewport);
    assert!(
        placement.flipped_vertical,
        "bottom edge: no room below the cursor, so the card opens above it"
    );
    assert_eq!(
        placement.origin.y,
        point.y - card.height - space::XXS,
        "bottom edge: the mirrored card sits its own height above the cursor"
    );
    assert!(
        !placement.clamped_vertical,
        "bottom edge: the mirrored card fits, so the margin must not move it again"
    );
    assert!(
        placement.origin.x > 0.0,
        "bottom edge: the horizontal axis is untouched by a vertical flip"
    );
}

/// An opened card produces exactly one rectangle, at the anchor plus the
/// offset and with the menu's own size; a target with no rows opens nothing.
#[test]
fn test_open_card_rects_track_the_cascade_and_an_empty_target_opens_nothing() {
    let rows = [
        ContextRow::new("a", "A", ContextGroup::Primary, message_noop()),
        ContextRow::new("b", "B", ContextGroup::Management, message_noop()),
    ];
    let menu = menu_from_rows(MenuId::new("t"), &rows);
    let mut cascade = hoarfrost::surface::menu::cascade::MenuCascade::new();
    let mut focus = hoarfrost::focus::FocusState::new();
    let _ = cascade.open(
        &menu,
        hoarfrost::surface::menu::cascade::MenuTreeKind::Context,
        &mut focus,
    );

    let rects = open_card_rects(&cascade, Point::new(30.0, 40.0), Size::new(1200.0, 800.0));
    assert_eq!(
        rects.len(),
        cascade.open_cards().len(),
        "open card: one rectangle per open card"
    );
    assert_eq!(
        Size::new(rects[0].width, rects[0].height),
        card_size(&menu),
        "open card: the rectangle carries the menu's own size"
    );
    assert_eq!(
        rects[0].x,
        30.0 + space::XXS,
        "open card: the rectangle is anchored at the pointer plus the offset"
    );

    let empty = menu_from_rows(root_id(&ContextTarget::TreeFile { path: file_path() }), &[]);
    let mut cascade = hoarfrost::surface::menu::cascade::MenuCascade::new();
    let mut focus = hoarfrost::focus::FocusState::new();
    let outcome = cascade.open(
        &empty,
        hoarfrost::surface::menu::cascade::MenuTreeKind::Context,
        &mut focus,
    );
    assert!(
        !cascade.is_open() || outcome.is_ignored(),
        "empty target: an empty card cannot hold a menu: {outcome:?}"
    );
}

// --- the spec's group runs per target --------------------------------------

/// Each target's rows declare exactly the §4.9.1 runs, in order.
#[test]
fn test_every_target_groups_its_rows_into_the_declared_spec_runs() {
    let cases: Vec<(&str, Vec<ContextRow>, Vec<ContextGroup>)> = vec![
        (
            "tree file",
            left::file_menu_items(&file_path(), &[skill("FormatMarkdown")], true),
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
        ),
        (
            "tree directory",
            left::directory_menu_items(&dir_path(), &[skill("FolderSkill")]),
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
        ),
        (
            "tab",
            center::tab_context_menu_items(0, &file_path(), &[skill("FormatMarkdown")]),
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
        ),
        (
            "multi-selection",
            left::multi_select_menu_items(&[file_path(), dir_path()]),
            vec![ContextGroup::Primary, ContextGroup::Destructive],
        ),
    ];

    for (label, rows, expected) in cases {
        assert_eq!(
            group_sequence(&rows),
            expected,
            "{label}: the declared runs must match the spec's grouping"
        );
    }
}

// --- the file menu's rows --------------------------------------------------

/// INT-3b changed row order and grouping, not a single label; and a missing
/// Typst drops the Save-as-PDF row rather than disabling it.
#[test]
fn test_the_file_menu_labels_are_unchanged_and_typst_absence_drops_its_row() {
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
        "labels: INT-3b changed row order and grouping, not a single label"
    );

    let without = left::file_menu_items(&file_path(), &[], false);
    assert!(
        !without.iter().any(|row| row.key == KEY_SAVE_AS_PDF),
        "typst absent: the row is absent, not greyed out"
    );
    assert_eq!(
        without
            .iter()
            .filter(|row| row.group == ContextGroup::Primary)
            .count(),
        2,
        "typst absent: only the two remaining primary rows are left"
    );
}
