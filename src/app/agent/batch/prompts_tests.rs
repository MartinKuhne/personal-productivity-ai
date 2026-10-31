//! Tests for prompt discovery (`agent::batch::prompts`).

use super::*;
use crate::ui_iced::menu_surface::log_capture::{Capture, with_capture};
use std::fs;

#[test]
fn test_read_prompt_content_with_front_matter() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("test-prompt.md");
    fs::write(
        &path,
        "---\ntitle: My Prompt\ntags: [prompt]\n---\nThis is the prompt body.",
    )
    .unwrap();

    let content = read_prompt_content(&path).unwrap();
    assert_eq!(content, "This is the prompt body.");
}

#[test]
fn test_read_prompt_content_no_front_matter() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("simple.md");
    fs::write(&path, "Just plain content without front matter.").unwrap();

    let content = read_prompt_content(&path).unwrap();
    assert_eq!(content, "Just plain content without front matter.");
}

#[test]
fn test_read_prompt_content_empty_body() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.md");
    fs::write(&path, "---\ntitle: Empty\n---\n   ").unwrap();

    let content = read_prompt_content(&path).unwrap();
    assert_eq!(content, "");
}

#[test]
fn test_read_prompt_content_nonexistent_file() {
    let result = read_prompt_content(Path::new("/nonexistent/file.md"));
    assert!(result.is_err());
}

#[test]
fn test_discover_batch_skills() {
    let dir = tempfile::tempdir().unwrap();
    let sys_dir = dir.path().join("system");
    let batch_dir = sys_dir.join("Skills").join("Batch");
    fs::create_dir_all(&batch_dir).unwrap();

    fs::write(
        batch_dir.join("SummarizeAll.md"),
        "---\ntitle: Summarize\n---\nSummarize this file.",
    )
    .unwrap();
    fs::write(
        batch_dir.join("ExtractEntities.txt"),
        "Extract entities from this file.",
    )
    .unwrap();
    fs::write(batch_dir.join(".hidden.md"), "Ignore me").unwrap();

    let libs = vec![ContentLibrary {
        root_folder: sys_dir.to_string_lossy().to_string(),
        name: "System".to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 0,
    }];

    let skills = discover_batch_skills(&libs);
    assert_eq!(skills.len(), 2);
    assert_eq!(
        skills[0].display_name,
        "System / Skills/Batch/ExtractEntities"
    );
    assert_eq!(skills[0].content, "Extract entities from this file.");
    assert_eq!(skills[1].display_name, "System / Skills/Batch/SummarizeAll");
    assert_eq!(skills[1].content, "Summarize this file.");

    let config = AppConfig {
        content_libraries: libs.clone(),
        ..Default::default()
    };
    let discovered = discover_prompts(&config);
    assert_eq!(discovered.len(), 2);

    let resolved = resolve_prompts(&BTreeSet::new(), &libs);
    assert_eq!(resolved.len(), 2);
}

#[test]
fn test_discover_batch_skills_deterministic_sorting() {
    let dir = tempfile::tempdir().unwrap();
    let sys_dir = dir.path().join("system");
    let batch_dir = sys_dir.join("Skills").join("Batch");
    fs::create_dir_all(&batch_dir).unwrap();

    fs::write(batch_dir.join("Z_Skill.md"), "Z content").unwrap();
    fs::write(batch_dir.join("A_Skill.md"), "A content").unwrap();
    fs::write(batch_dir.join("M_Skill.md"), "M content").unwrap();

    let libs = vec![ContentLibrary {
        root_folder: sys_dir.to_string_lossy().to_string(),
        name: "System".to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 0,
    }];

    let skills = discover_batch_skills(&libs);
    assert_eq!(skills.len(), 3);
    assert_eq!(skills[0].display_name, "System / Skills/Batch/A_Skill");
    assert_eq!(skills[0].content, "A content");
    assert_eq!(skills[1].display_name, "System / Skills/Batch/M_Skill");
    assert_eq!(skills[1].content, "M content");
    assert_eq!(skills[2].display_name, "System / Skills/Batch/Z_Skill");
    assert_eq!(skills[2].content, "Z content");
}

#[test]
fn test_resolve_prompts_success() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prompt.md");
    fs::write(
        &path,
        "---\ntitle: Resolved\ntags: [prompt]\n---\nResolved prompt body.",
    )
    .unwrap();

    let libs = vec![ContentLibrary {
        root_folder: dir.path().to_string_lossy().to_string(),
        name: "TestLib".to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 0,
    }];
    let prompt_paths = BTreeSet::from([path.clone()]);

    let resolved = resolve_prompts(&prompt_paths, &libs);

    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].path, path);
    assert_eq!(resolved[0].display_name, "TestLib / prompt.md");
    assert_eq!(resolved[0].content, "Resolved prompt body.");
    assert_eq!(resolved[0].library_name, "TestLib");
}

#[test]
fn test_resolve_prompts_missing_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing.md");
    let libs = vec![ContentLibrary {
        root_folder: dir.path().to_string_lossy().to_string(),
        name: "TestLib".to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 0,
    }];
    let prompt_paths = BTreeSet::from([path]);

    let resolved = resolve_prompts(&prompt_paths, &libs);

    assert_eq!(resolved.len(), 0);
}

#[test]
fn test_resolve_prompts_without_prompt_tag() {
    // resolve_prompts does NOT filter by prompt tag — it trusts the caller
    // (tag manager) to supply only prompt-tagged paths. It just reads the
    // file and builds PromptInfo.
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("not-a-prompt.md");
    fs::write(
        &path,
        "---\ntitle: Not a Prompt\ntags: [other]\n---\nBody without prompt tag.",
    )
    .unwrap();

    let libs = vec![ContentLibrary {
        root_folder: dir.path().to_string_lossy().to_string(),
        name: "TestLib".to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 0,
    }];
    let prompt_paths = BTreeSet::from([path]);

    let resolved = resolve_prompts(&prompt_paths, &libs);

    // The path is returned even without a prompt tag because resolve_prompts
    // trusts the caller's prompt_paths set.
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].content, "Body without prompt tag.");
}

#[test]
fn test_resolve_prompts_invalid_utf8() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("invalid.md");
    // Write invalid UTF-8
    std::fs::write(&path, [0xFF, 0xFE, 0xFD]).unwrap();

    let libs = vec![ContentLibrary {
        root_folder: dir.path().to_string_lossy().to_string(),
        name: "TestLib".to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 0,
    }];
    let prompt_paths = BTreeSet::from([path]);

    let resolved = resolve_prompts(&prompt_paths, &libs);

    assert_eq!(resolved.len(), 0);
}

/// A prompt library that cannot be read leaves no other trace, so the coded
/// error event *is* the observable contract ([NFR-001], [NFR-004]).
#[test]
fn test_log_prompt_discovery_error_emits_structured_fields() {
    let capture = Capture::new();
    let path = Path::new("test/path.md");
    with_capture(&capture, || {
        log_prompt_discovery_error(path, "test error message");
        log_prompt_discovery_error(
            path,
            std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
        );
    });

    let events = capture.events();
    assert_eq!(
        events.len(),
        2,
        "each discovery failure must emit exactly one event, got {events:?}"
    );
    for event in &events {
        assert_eq!(
            event.level,
            tracing::Level::ERROR,
            "a prompt that cannot be discovered is a failure ([NFR-001]), got {:?}",
            event.level
        );
        assert_eq!(
            event.target, "batch",
            "the event must be attributable to the batch subsystem"
        );
        assert_eq!(
            event.error_code(),
            Some(PROMPT_DISCOVERY_ERROR_CODE),
            "the event must carry the coded error ([NFR-004]), got {:?}",
            event.fields
        );
        assert_eq!(
            event.message, PROMPT_DISCOVERY_FAILURE_MESSAGE,
            "the rendered message must name the failure"
        );
        assert_eq!(
            event.field("path"),
            Some("test/path.md"),
            "the failing path must be recorded, got {:?}",
            event.fields
        );
    }
    assert_eq!(
        events[0].field("error"),
        Some("test error message"),
        "the error's own text must be carried through verbatim"
    );
    assert_eq!(
        events[1].field("error"),
        Some("not found"),
        "an `io::Error` must be recorded by its display text, got {:?}",
        events[1].fields
    );
}
// ---------------------------------------------------------------------------
// `has_prompt_tag` — the front-matter predicate that decides what is a prompt
// ---------------------------------------------------------------------------

/// Parse a YAML document into the value shape `Document::front_matter()` hands
/// to the tag check.
fn front_matter(yaml: &str) -> Value {
    serde_norway::from_str::<Value>(yaml).expect("the YAML fixture must parse")
}

/// A `tags` sequence containing `prompt` marks the file as a prompt, whatever
/// case the author used ([REQ: batch dialog prompt discovery]).
#[test]
fn test_has_prompt_tag_accepts_a_sequence_containing_prompt() {
    for yaml in [
        "tags: [prompt]",
        "tags: [Prompt]",
        "tags: [PROMPT]",
        "tags: [note, prompt]",
        "tags:\n  - note\n  - Prompt\n",
    ] {
        assert!(
            has_prompt_tag(&front_matter(yaml)),
            "a `tags` sequence containing `prompt` (any case) must match, got: {yaml:?}"
        );
    }
}

/// The single-string form `tags: prompt` is the other shape authors write.
#[test]
fn test_has_prompt_tag_accepts_a_bare_string_tag() {
    assert!(
        has_prompt_tag(&front_matter("tags: prompt")),
        "a scalar `tags: prompt` must match"
    );
    assert!(
        has_prompt_tag(&front_matter("tags: Prompt")),
        "the scalar form must be case-insensitive too"
    );
}

/// Everything that is not a `prompt` tag must be rejected, including tags whose
/// value merely looks prompt-ish.
#[test]
fn test_has_prompt_tag_rejects_documents_without_a_prompt_tag() {
    for yaml in [
        "title: note",
        "tags: []",
        "tags: note",
        "tags: [note, template]",
        "tags: promptish",
        "tags: [promptish]",
        "tags: 42",
        "tags: {name: prompt}",
        "tags: [1, 2]",
    ] {
        assert!(
            !has_prompt_tag(&front_matter(yaml)),
            "only an exact `prompt` tag may match, got: {yaml:?}"
        );
    }
}

/// Front matter that is not a mapping at all has no `tags` key to read.
#[test]
fn test_has_prompt_tag_rejects_non_mapping_documents() {
    for yaml in ["just a string", "42", "- prompt\n- other\n"] {
        assert!(
            !has_prompt_tag(&front_matter(yaml)),
            "a non-mapping front matter must never match, got: {yaml:?}"
        );
    }

    assert!(
        !has_prompt_tag(&Value::Null),
        "an empty document has no tags and must not match"
    );
}

// ---------------------------------------------------------------------------
// `discover_prompts` — walking a library for tagged markdown
// ---------------------------------------------------------------------------

/// A tagged prompt anywhere under a library root is discovered, labelled with
/// the library and its path relative to that root, and sorted by display name.
#[test]
fn test_discover_prompts_collects_tagged_markdown_labelled_by_library() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    fs::write(
        root.join("alpha.md"),
        "---\ntitle: Alpha\ntags: [prompt]\n---\nAlpha body\n",
    )
    .unwrap();
    fs::create_dir_all(root.join("sub")).unwrap();
    fs::write(
        root.join("sub").join("beta.markdown"),
        "---\ntitle: Beta\ntags: [Prompt]\n---\nBeta body\n",
    )
    .unwrap();
    // Not a prompt: untagged, front-matter-less, and wrong extension.
    fs::write(
        root.join("plain.md"),
        "---\ntags: [note]\n---\nNot a prompt\n",
    )
    .unwrap();
    fs::write(root.join("bare.md"), "no front matter at all\n").unwrap();
    fs::write(
        root.join("ignored.txt"),
        "---\ntags: [prompt]\n---\nNot markdown\n",
    )
    .unwrap();

    let config = AppConfig {
        content_libraries: vec![ContentLibrary {
            root_folder: root.to_string_lossy().to_string(),
            name: "Vault".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        }],
        ..Default::default()
    };

    let prompts = discover_prompts(&config);

    let expected_root = "Vault / alpha.md".to_string();
    let expected_nested = format!(
        "Vault / {}",
        Path::new("sub").join("beta.markdown").display()
    );

    let names: Vec<&str> = prompts.iter().map(|p| p.display_name.as_str()).collect();
    assert_eq!(
        names,
        vec![expected_root.as_str(), expected_nested.as_str()],
        "only tagged markdown is discovered, sorted by display name"
    );
    assert_eq!(
        prompts[0].library_name, "Vault",
        "the owning library must be recorded for the batch dialog"
    );
    assert_eq!(
        prompts[0].content, "Alpha body",
        "the body is stored without its front matter and trimmed"
    );
    assert_eq!(
        prompts[1].content, "Beta body",
        "the nested prompt body must be read from its own file"
    );
    assert_eq!(
        prompts[0].path,
        root.join("alpha.md"),
        "the discovered path must point at the file itself"
    );
}

/// A library whose root does not exist contributes nothing instead of failing
/// the whole discovery ([FMD-064]).
#[test]
fn test_discover_prompts_skips_a_library_whose_root_is_missing() {
    let dir = tempfile::tempdir().unwrap();
    let missing = dir.path().join("does-not-exist");

    let config = AppConfig {
        content_libraries: vec![ContentLibrary {
            root_folder: missing.to_string_lossy().to_string(),
            name: "Ghost".to_string(),
            kind: "text".to_string(),
            readonly: false,
            priority: 0,
        }],
        ..Default::default()
    };

    assert!(
        discover_prompts(&config).is_empty(),
        "an unreachable library root must be skipped, not fatal"
    );
}
// ---------------------------------------------------------------------------
// Failure and deduplication paths of the discovery walk
// ---------------------------------------------------------------------------

/// A content-library entry pointing at `root` under the given display name.
fn library(root: &Path, name: &str) -> ContentLibrary {
    ContentLibrary {
        root_folder: root.to_string_lossy().to_string(),
        name: name.to_string(),
        kind: "text".to_string(),
        readonly: false,
        priority: 0,
    }
}

/// Bytes that are not valid UTF-8, so `read_to_string` fails on them.
const NOT_UTF8: &[u8] = &[0xff, 0xfe, 0x00, 0x9c];

/// A markdown file the walk cannot read is skipped *and* reported, while the
/// readable prompts in the same library are still discovered ([FMD-064],
/// [NFR-001], [NFR-004]).
#[test]
fn test_discover_prompts_skips_a_markdown_file_it_cannot_read_and_logs_it() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::write(
        root.join("good.md"),
        "---\ntags: [prompt]\n---\nGood body\n",
    )
    .unwrap();
    fs::write(root.join("broken.md"), NOT_UTF8).unwrap();

    let config = AppConfig {
        content_libraries: vec![library(root, "Vault")],
        ..Default::default()
    };

    let capture = Capture::new();
    let prompts = with_capture(&capture, || discover_prompts(&config));

    assert_eq!(
        prompts.len(),
        1,
        "one unreadable prompt must not cost the readable one, got {prompts:?}"
    );
    assert_eq!(
        prompts[0].display_name, "Vault / good.md",
        "the readable prompt must still be labelled by its library"
    );
    let failure = capture
        .with_code(PROMPT_DISCOVERY_ERROR_CODE)
        .unwrap_or_else(|| {
            panic!(
                "an unreadable prompt must log a coded error, got {:?}",
                capture.codes()
            )
        });
    assert_eq!(
        failure.level,
        tracing::Level::ERROR,
        "a dropped prompt is a failure ([NFR-001])"
    );
    assert!(
        failure
            .field("path")
            .is_some_and(|p| p.ends_with("broken.md")),
        "the log must name the file that could not be read, got {:?}",
        failure.fields
    );
}

/// A file that is both a tagged prompt and a `Skills/Batch` skill is listed
/// once — the skill pass must not duplicate what the walk already found.
#[test]
fn test_discover_prompts_lists_a_tagged_batch_skill_only_once() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let batch = root.join("Skills").join("Batch");
    fs::create_dir_all(&batch).unwrap();
    fs::write(
        batch.join("Weekly.md"),
        "---\ntags: [prompt]\n---\nWeekly body\n",
    )
    .unwrap();

    let config = AppConfig {
        content_libraries: vec![library(root, "Vault")],
        ..Default::default()
    };

    let prompts = discover_prompts(&config);
    assert_eq!(
        prompts.len(),
        1,
        "a skill that is also a tagged prompt must appear exactly once, got {prompts:?}"
    );
    assert_eq!(
        prompts[0].path,
        batch.join("Weekly.md"),
        "the single entry must point at the skill file"
    );
    assert_eq!(prompts[0].content, "Weekly body");
}

/// `Skills/Batch` is only scanned when it is actually a directory; a regular
/// file sitting at that path is skipped instead of aborting discovery.
#[test]
fn test_discover_batch_skills_ignores_a_batch_folder_that_is_a_regular_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    fs::create_dir_all(root.join("Skills")).unwrap();
    fs::write(root.join("Skills").join("Batch"), "not a directory").unwrap();

    let skills = discover_batch_skills(&[library(root, "Sys")]);
    assert!(
        skills.is_empty(),
        "a non-directory `Skills/Batch` must be skipped, got {skills:?}"
    );
}

/// An unreadable skill file is logged and dropped; the readable skills around it
/// are still offered to the batch dialog.
#[test]
fn test_discover_batch_skills_skips_a_skill_it_cannot_read_and_logs_it() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let batch = root.join("Skills").join("Batch");
    fs::create_dir_all(&batch).unwrap();
    fs::write(batch.join("good.md"), "Good skill body").unwrap();
    fs::write(batch.join("broken.md"), NOT_UTF8).unwrap();

    let capture = Capture::new();
    let skills = with_capture(&capture, || discover_batch_skills(&[library(root, "Sys")]));

    assert_eq!(
        skills.len(),
        1,
        "one unreadable skill must not cost the readable one, got {skills:?}"
    );
    assert_eq!(skills[0].display_name, "Sys / Skills/Batch/good");
    assert_eq!(skills[0].content, "Good skill body");
    assert!(
        capture
            .with_code(PROMPT_DISCOVERY_ERROR_CODE)
            .is_some_and(|event| event
                .field("path")
                .is_some_and(|p| p.ends_with("broken.md"))),
        "the unreadable skill must be reported with its path, got {:?}",
        capture.codes()
    );
}

/// A path the tag index holds but that belongs to no configured library is
/// still resolved — it just falls back to the bare file name as its label.
#[test]
fn test_resolve_prompts_labels_a_path_outside_every_library_by_its_file_name() {
    let dir = tempfile::tempdir().unwrap();
    let loose = dir.path().join("loose.md");
    fs::write(&loose, "---\ntags: [prompt]\n---\nLoose body\n").unwrap();

    let other = tempfile::tempdir().unwrap();
    let resolved = resolve_prompts(
        &BTreeSet::from([loose.clone()]),
        &[library(other.path(), "Other")],
    );

    assert_eq!(resolved.len(), 1, "an unlabelled path must still resolve");
    assert_eq!(
        resolved[0].display_name, "loose.md",
        "with no owning library the file name is the only label available"
    );
    assert_eq!(
        resolved[0].library_name, "",
        "no library may be claimed for a path outside every root"
    );
    assert_eq!(resolved[0].content, "Loose body");
    assert_eq!(resolved[0].path, loose);
}

/// A skill that the tag index already lists is not appended a second time, while
/// a skill it does not list still is.
#[test]
fn test_resolve_prompts_appends_only_the_skills_missing_from_the_tag_index() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let batch = root.join("Skills").join("Batch");
    fs::create_dir_all(&batch).unwrap();
    fs::write(batch.join("indexed.md"), "Indexed skill").unwrap();
    fs::write(batch.join("unindexed.md"), "Unindexed skill").unwrap();
    fs::write(
        root.join("note.md"),
        "---\ntags: [prompt]\n---\nNote body\n",
    )
    .unwrap();

    let prompt_paths = BTreeSet::from([root.join("note.md"), batch.join("indexed.md")]);
    let resolved = resolve_prompts(&prompt_paths, &[library(root, "Vault")]);

    let paths: Vec<PathBuf> = resolved.iter().map(|p| p.path.clone()).collect();
    assert_eq!(
        paths.len(),
        3,
        "two indexed prompts plus the one unindexed skill, got {paths:?}"
    );
    assert_eq!(
        paths
            .iter()
            .filter(|p| **p == batch.join("indexed.md"))
            .count(),
        1,
        "a skill already in the tag index must not be appended twice"
    );
    assert!(
        paths.contains(&batch.join("unindexed.md")),
        "a skill the tag index never saw must still be offered, got {paths:?}"
    );
}

/// A `tags` value that is neither a sequence nor a string (a number, a mapping,
/// an explicit null) is not a prompt tag — the predicate must not panic on it.
#[test]
fn test_has_prompt_tag_rejects_a_tags_value_that_is_neither_list_nor_string() {
    for yaml in ["tags: 42", "tags: {name: prompt}", "tags:", "tags: true"] {
        let value = front_matter(yaml);
        assert!(
            !has_prompt_tag(&value),
            "`{yaml}` must not be treated as a prompt tag, got {value:?}"
        );
    }
}

/// Non-string entries inside the tag list are ignored rather than compared, and
/// only a real `prompt` string flips the predicate.
#[test]
fn test_has_prompt_tag_ignores_non_string_entries_in_the_tag_list() {
    assert!(
        !has_prompt_tag(&front_matter("tags: [42, note]")),
        "a list without a `prompt` string is not a prompt"
    );
    assert!(
        has_prompt_tag(&front_matter("tags: [42, PROMPT, null]")),
        "the matching string may sit anywhere in a mixed-type list"
    );
}
