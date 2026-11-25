//! Tests for the datamarking envelope helpers and security header (`agent::datamark`).

use super::*;

#[test]
fn test_wrap_tool_result_basic() {
    let out = wrap_tool_result("web_fetch", "Hello world");
    assert!(out.starts_with(EXTERNAL_DATA_START));
    assert!(out.ends_with(EXTERNAL_DATA_END));
    assert!(out.contains("provenance=tool:web_fetch"));
    assert!(out.contains("trust=untrusted"));
    assert!(out.contains("Hello world"));
}

#[test]
fn test_wrap_tool_result_preserves_multiline_content() {
    let content = "line1\nline2\nline3\n";
    let out = wrap_tool_result("read_note", content);
    assert!(out.contains("line1\nline2\nline3"));
}

#[test]
fn test_wrap_tool_result_adds_trailing_newline() {
    // Content without trailing newline must still have the
    // closing marker on its own line.
    let out = wrap_tool_result("search_notes", "no-newline");
    let end_with_newline = out.ends_with(&format!("\n{EXTERNAL_DATA_END}"));
    assert!(
        end_with_newline,
        "closing marker should sit on its own line; got tail: {:?}",
        &out[out.len().saturating_sub(64)..]
    );
}

#[test]
fn test_wrap_tool_result_does_not_double_newline() {
    // Content with trailing newline: don't add another.
    let out = wrap_tool_result("read_note", "trailing\n");
    // Exactly one newline before the closing marker.
    let needle = format!("\n{EXTERNAL_DATA_END}");
    let count = out.matches(&needle).count();
    assert_eq!(
        count, 1,
        "expected exactly one newline before END, got {count} in: {out:?}"
    );
}

#[test]
fn test_wrap_user_md_includes_library() {
    let out = wrap_user_md("Notes", "remember the milk");
    assert!(out.contains("provenance=user_md"));
    assert!(out.contains("library=Notes"));
    assert!(out.contains("remember the milk"));
    assert!(out.starts_with(EXTERNAL_DATA_START));
    assert!(out.ends_with(EXTERNAL_DATA_END));
}

#[test]
fn test_wrap_envelope_contains_markers_around_content() {
    // The content must sit *between* the markers, never overlap.
    let content = "sensitive data";
    let out = wrap_tool_result("read_note", content);
    let start_idx = out.find(EXTERNAL_DATA_START).expect("start marker");
    let end_idx = out.find(EXTERNAL_DATA_END).expect("end marker");
    let content_idx = out.find(content).expect("content");
    assert!(start_idx < content_idx);
    assert!(content_idx < end_idx);
}

#[test]
fn test_provenance_header_line_is_key_value() {
    // The header line format is part of the contract — tests
    // elsewhere grep for `provenance=tool:` and `provenance=user_md`.
    let tool = Provenance::Tool("web_fetch".to_string()).header_line();
    assert_eq!(tool, "provenance=tool:web_fetch");
    let user_md = Provenance::UserMd {
        library: "Notes".to_string(),
    }
    .header_line();
    assert_eq!(user_md, "provenance=user_md library=Notes");
}

#[test]
fn test_trust_value_is_unstable_only_for_untrusted() {
    // The single Trust variant is `Untrusted`. If we ever add
    // a new variant, this test should be updated to assert the
    // new mapping too.
    assert_eq!(Trust::Untrusted.as_str(), "untrusted");
}

#[test]
fn test_security_header_mentions_markers() {
    // The LLM is trained to follow the convention. If a future
    // edit renames the markers, this test fails before the
    // LLM-facing contract drifts.
    assert!(SECURITY_HEADER.contains(EXTERNAL_DATA_START));
    assert!(SECURITY_HEADER.contains(EXTERNAL_DATA_END));
}

#[test]
fn test_security_header_mentions_mutating_tool_prefixes() {
    // V8 mitigation relies on the LLM recognising the prefix list
    // and refusing. If a future edit drops a prefix, a destructive
    // tool with that prefix becomes harder to catch.
    for prefix in [
        "delete_", "send_", "replace_", "write_", "add_", "update_", "create_",
    ] {
        assert!(
            SECURITY_HEADER.contains(prefix),
            "security header should list the mutating tool prefix `{prefix}`"
        );
    }
}
