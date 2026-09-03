//! Scanner for recognizing bare URLs and wikilinks in markdown text runs.
//!
//! Unit tests live in the sibling `link_scanner_tests.rs` sidecar.

use crate::markdown::{InlineElem, TextStyle};

/// Scans a text run for bare URLs and wikilinks, segmenting it into `InlineElem`s.
///
/// If `style.code` is true, the text is returned as a single `InlineElem::Text`
/// without link parsing.
pub fn scan_text_for_links(text: &str, style: &TextStyle) -> Vec<InlineElem> {
    if style.code || text.is_empty() {
        return vec![InlineElem::Text(text.to_string(), style.clone())];
    }

    let mut result = Vec::new();
    let mut cursor = 0;
    let bytes = text.as_bytes();
    let len = bytes.len();

    while cursor < len {
        // Find next candidate link: either wikilink `[[` or URL scheme / `www.`
        let next_wikilink = find_subsequence(&bytes[cursor..], b"[[");
        let next_url = find_url_candidate(&text[cursor..]);

        match (next_wikilink, next_url) {
            (None, None) => {
                // No more links found; push remainder
                let rem = &text[cursor..];
                push_text(&mut result, rem, style);
                break;
            }
            (Some(wiki_rel), None) => {
                let wiki_start = cursor + wiki_rel;
                if let Some((end, target, label)) = parse_wikilink(&text[wiki_start..]) {
                    if wiki_start > cursor {
                        push_text(&mut result, &text[cursor..wiki_start], style);
                    }
                    result.push(InlineElem::Link(format!("wikilink:{target}"), label));
                    cursor = wiki_start + end;
                } else {
                    // Not a valid wikilink, advance past `[[`
                    let advance = wiki_start + 2;
                    push_text(&mut result, &text[cursor..advance], style);
                    cursor = advance;
                }
            }
            (None, Some((url_rel, url_len, dest_url))) => {
                let url_start = cursor + url_rel;
                if url_start > cursor {
                    push_text(&mut result, &text[cursor..url_start], style);
                }
                let display = text[url_start..url_start + url_len].to_string();
                result.push(InlineElem::Link(dest_url, display));
                cursor = url_start + url_len;
            }
            (Some(wiki_rel), Some((url_rel, url_len, dest_url))) => {
                if wiki_rel < url_rel {
                    let wiki_start = cursor + wiki_rel;
                    if let Some((end, target, label)) = parse_wikilink(&text[wiki_start..]) {
                        if wiki_start > cursor {
                            push_text(&mut result, &text[cursor..wiki_start], style);
                        }
                        result.push(InlineElem::Link(format!("wikilink:{target}"), label));
                        cursor = wiki_start + end;
                    } else {
                        let advance = wiki_start + 2;
                        push_text(&mut result, &text[cursor..advance], style);
                        cursor = advance;
                    }
                } else {
                    let url_start = cursor + url_rel;
                    if url_start > cursor {
                        push_text(&mut result, &text[cursor..url_start], style);
                    }
                    let display = text[url_start..url_start + url_len].to_string();
                    result.push(InlineElem::Link(dest_url, display));
                    cursor = url_start + url_len;
                }
            }
        }
    }

    result
}

/// Appends text to the result vector, coalescing with the previous `Text` element if styles match.
fn push_text(result: &mut Vec<InlineElem>, text: &str, style: &TextStyle) {
    if text.is_empty() {
        return;
    }
    if let Some(InlineElem::Text(existing, s)) = result.last_mut()
        && s == style
    {
        existing.push_str(text);
        return;
    }
    result.push(InlineElem::Text(text.to_string(), style.clone()));
}

/// Finds the relative byte offset of a byte subsequence within a slice.
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Attempts to parse a wikilink starting at the beginning of `s` (which begins with `[[`).
/// Returns `Some((total_bytes_consumed, target, display_label))`.
fn parse_wikilink(s: &str) -> Option<(usize, String, String)> {
    if !s.starts_with("[[") {
        return None;
    }
    let closing = s.find("]]")?;
    // Wikilinks cannot span newlines
    if s[..closing].contains('\n') || s[..closing].contains('\r') {
        return None;
    }
    let inner = &s[2..closing];
    let (target_part, label_part) = if let Some(pipe_idx) = inner.find('|') {
        let t = inner[..pipe_idx].trim();
        let l = inner[pipe_idx + 1..].trim();
        (t, if l.is_empty() { t } else { l })
    } else {
        let t = inner.trim();
        (t, t)
    };

    if target_part.is_empty() {
        return None;
    }

    Some((closing + 2, target_part.to_string(), label_part.to_string()))
}

/// Finds the first candidate bare URL in `s`.
/// Returns `Some((start_byte_offset, byte_length, destination_url))`.
fn find_url_candidate(s: &str) -> Option<(usize, usize, String)> {
    let mut earliest: Option<(usize, usize, String)> = None;

    // Search schemes: "https://", "http://", "ftp://"
    for scheme in &["https://", "http://", "ftp://"] {
        let mut search_from = 0;
        while let Some(pos) = s[search_from..].find(scheme) {
            let abs_pos = search_from + pos;
            if is_valid_url_boundary_before(s, abs_pos) {
                let (len, dest) = extract_url_from_start(&s[abs_pos..], scheme);
                if len > scheme.len() {
                    match earliest {
                        None => earliest = Some((abs_pos, len, dest)),
                        Some((earliest_pos, _, _)) if abs_pos < earliest_pos => {
                            earliest = Some((abs_pos, len, dest));
                        }
                        _ => {}
                    }
                    break;
                }
            }
            search_from = abs_pos + scheme.len();
        }
    }

    // Search "www."
    let mut search_from = 0;
    while let Some(pos) = s[search_from..].find("www.") {
        let abs_pos = search_from + pos;
        if is_valid_url_boundary_before(s, abs_pos) {
            let (len, _) = extract_url_from_start(&s[abs_pos..], "www.");
            // Must have something resembling a domain after "www." (e.g. www.x.y)
            let candidate = &s[abs_pos..abs_pos + len];
            if candidate.len() > 4 && candidate[4..].contains('.') {
                let dest = format!("https://{candidate}");
                match earliest {
                    None => earliest = Some((abs_pos, len, dest)),
                    Some((earliest_pos, _, _)) if abs_pos < earliest_pos => {
                        earliest = Some((abs_pos, len, dest));
                    }
                    _ => {}
                }
                break;
            }
        }
        search_from = abs_pos + 4;
    }

    earliest
}

/// Verifies that the character preceding the URL position is a valid boundary
/// (e.g. whitespace, start of string, or open parenthesis/bracket).
fn is_valid_url_boundary_before(s: &str, pos: usize) -> bool {
    if pos == 0 {
        return true;
    }
    let prev_char = s[..pos].chars().next_back().unwrap_or(' ');
    prev_char.is_whitespace()
        || matches!(
            prev_char,
            '(' | '[' | '{' | '<' | '"' | '\'' | '`' | ':' | ';' | ',' | '.'
        )
}

/// Extracts a URL starting at the beginning of `s`, trimming invalid characters
/// and trailing punctuation according to CommonMark / GFM autolink heuristics.
fn extract_url_from_start(s: &str, _scheme: &str) -> (usize, String) {
    // Collect bytes until whitespace or forbidden delimiters
    let mut end = 0;
    for (idx, ch) in s.char_indices() {
        if ch.is_whitespace() || matches!(ch, '<' | '>' | '"' | '`') {
            break;
        }
        end = idx + ch.len_utf8();
    }

    let mut candidate = &s[..end];

    // Trim trailing punctuation (.,;:!?) unless it's balanced or part of query
    while !candidate.is_empty() {
        let last_char = candidate.chars().next_back().unwrap();
        if matches!(last_char, '.' | ',' | ';' | ':' | '!' | '?' | '\'') {
            candidate = &candidate[..candidate.len() - last_char.len_utf8()];
        } else if last_char == ')' {
            // Trim ')' if it has no matching '(' in the candidate
            let open_count = candidate.chars().filter(|&c| c == '(').count();
            let close_count = candidate.chars().filter(|&c| c == ')').count();
            if close_count > open_count {
                candidate = &candidate[..candidate.len() - 1];
            } else {
                break;
            }
        } else if last_char == ']' {
            let open_count = candidate.chars().filter(|&c| c == '[').count();
            let close_count = candidate.chars().filter(|&c| c == ']').count();
            if close_count > open_count {
                candidate = &candidate[..candidate.len() - 1];
            } else {
                break;
            }
        } else {
            break;
        }
    }

    (candidate.len(), candidate.to_string())
}

#[cfg(test)]
#[path = "link_scanner_tests.rs"]
mod link_scanner_tests;
