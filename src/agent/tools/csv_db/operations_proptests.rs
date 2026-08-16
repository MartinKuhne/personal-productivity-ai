//! Property-based tests for the CSV row-writer in
//! `agent::tools::csv_db::operations::write_rows_to_writer`.
//!
//! `write_rows_to_writer` is the atomic-write path for CSV
//! rows: it serialises the new rows through the `csv` crate
//! and flushes them to a `Write` impl. The caller is
//! expected to commit the resulting bytes atomically
//! (e.g. via a single `write_all` to a file).
//!
//! The proptest surface is the helper directly, not the
//! higher-level `add_rows` (which requires a real
//! filesystem and an existing CSV). The helper is the
//! pure function that does the row-formatting + flush; a
//! panic here would corrupt every CSV add-row operation.
//!
//! # Properties under test
//!
//! All four properties are sourced from the A5 corner-case
//! row in `doc/planning/fuzzing.md` §2.2 "Phase 3".
//!
//! 1. **`write_rows_to_writer` never panics on any
//!    input.** Any `Vec<HashMap<String, String>>` and any
//!    header list is serialised without unwinding.
//! 2. **The output is valid CSV.** Every row has the
//!    same number of cells as the header list, in the
//!    header's order. A missing key in a row produces an
//!    empty cell, not a panic or a shifted cell.
//! 3. **Header order is preserved.** A row with a HashMap
//!    whose keys are not in the header order still
//!    produces cells in the header order. The csv writer
//!    must use the header list as the column order, not
//!    the HashMap's iteration order (which is
//!    non-deterministic).
//! 4. **Empty row list is a no-op.** An empty `rows`
//!    argument produces an empty output (no header row is
//!    written by the helper — that's the caller's job).
//!
//! `cases = 1024` per property.

use crate::tools::csv_db::operations::write_rows_to_writer;
use proptest::prelude::*;
use std::collections::HashMap;
use std::io::{self, Write};

/// One proptest case count for every property in this sidecar.
const CASES: u32 = 1024;

/// A `Write` that captures every byte in a `Vec<u8>`.
#[derive(Default)]
struct CapturedWrite {
    bytes: Vec<u8>,
}
impl Write for CapturedWrite {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Strategy: a list of 1-4 column headers, each `A-Z{1,5}`.
fn any_headers() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec("[A-Z][a-z]{1,4}", 1..4)
}

/// Strategy: a list of 0-8 rows, each row a `HashMap` whose
/// keys are a subset of the supplied headers.
fn any_rows_for(headers: Vec<String>) -> impl Strategy<Value = Vec<HashMap<String, String>>> {
    // The row's key set is a non-empty subset of the
    // headers (possibly with extras that the writer must
    // ignore — the writer uses the headers as the column
    // order, not the row's keys).
    let header_set: Vec<String> = headers.clone();
    let row = prop::collection::hash_map(
        prop::sample::select(header_set.clone()),
        prop::string::string_regex(r"[A-Za-z0-9 ]{0,16}").unwrap(),
        1..4,
    );
    prop::collection::vec(row, 0..8)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(CASES))]

    /// `write_rows_to_writer` never panics on any input.
    /// Any combination of rows and headers is serialised
    /// without unwinding. The function returns `Result`;
    /// the `?` operator inside the writer may surface a
    /// `String` error from the `csv` crate but never a
    /// panic.
    #[test]
    fn write_rows_never_panics_on_any_input(
        (headers, rows) in (any_headers(), any_headers())
            .prop_flat_map(|(h, _)| {
                let rows_strat = any_rows_for(h.clone());
                (Just(h), rows_strat)
            })
    ) {
        let mut captured = CapturedWrite::default();
        let _ = write_rows_to_writer(&rows, &headers, &mut captured);
    }

    /// The output is valid CSV. Every non-empty line in
    /// the output has the same number of cells as the
    /// header list. A missing key in a row produces an
    /// empty cell, not a shifted cell.
    #[test]
    fn write_rows_output_has_consistent_cell_count(
        (headers, rows) in (any_headers(), any_headers())
            .prop_flat_map(|(h, _)| {
                let rows_strat = any_rows_for(h.clone());
                (Just(h), rows_strat)
            })
    ) {
        let mut captured = CapturedWrite::default();
        let _ = write_rows_to_writer(&rows, &headers, &mut captured);
        let body = String::from_utf8_lossy(&captured.bytes);
        let n_cols = headers.len();
        for (i, line) in body.lines().enumerate() {
            // The csv crate quotes fields containing
            // commas/quotes/newlines. Strip quotes for the
            // cell-count check: a `"a,b"` field is one
            // cell, not two.
            let cells = parse_csv_line(line);
            prop_assert_eq!(
                cells.len(),
                n_cols,
                "row {} has {} cells, expected {} (line={:?})",
                i,
                cells.len(),
                n_cols,
                line
            );
        }
    }

    /// Header order is preserved. A row with a HashMap
    /// whose keys are not in the header order still
    /// produces cells in the header order.
    ///
    /// This test uses a fixed 3-element header list (not
    /// a proptest strategy) because the assertion checks
    /// specific cell values, and a strategy that
    /// generated the header names would either collide
    /// (HashMap dedupes) or hide the bug. The interesting
    /// case is the HashMap iteration order, which is
    /// already non-deterministic — the test relies on
    /// that to make the assertion meaningful.
    #[test]
    fn write_rows_header_order_is_canonical(_unused in 0..1u8) {
        let headers = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let h0 = headers[0].clone();
        let h1 = headers[1].clone();
        let h2 = headers[2].clone();
        // Insert in non-canonical order to make sure the
        // writer uses the headers (not the HashMap order).
        let mut row = HashMap::new();
        row.insert(h2.clone(), "value-2".to_string());
        row.insert(h0.clone(), "value-0".to_string());
        row.insert(h1.clone(), "value-1".to_string());
        let rows = vec![row];
        let mut captured = CapturedWrite::default();
        write_rows_to_writer(&rows, &headers, &mut captured).expect("write");
        let body = String::from_utf8_lossy(&captured.bytes);
        let lines: Vec<&str> = body.lines().collect();
        prop_assert!(!lines.is_empty(), "at least one row expected");
        let cells = parse_csv_line(lines[0]);
        prop_assert_eq!(cells.len(), 3);
        prop_assert_eq!(cells[0].clone(), "value-0");
        prop_assert_eq!(cells[1].clone(), "value-1");
        prop_assert_eq!(cells[2].clone(), "value-2");
    }

    /// An empty `rows` argument is a no-op. The helper
    /// does not write a header row (that's the caller's
    /// responsibility); the output is empty.
    #[test]
    fn write_rows_empty_rows_is_noop(
        headers in any_headers()
    ) {
        let mut captured = CapturedWrite::default();
        let _ = write_rows_to_writer(&[], &headers, &mut captured);
        prop_assert!(
            captured.bytes.is_empty(),
            "empty rows must produce empty output, got: {:?}",
            String::from_utf8_lossy(&captured.bytes)
        );
    }

    /// A row missing a key produces an empty cell, not
    /// a panic. The writer uses `row.get(header).cloned().unwrap_or_default()`
    /// so a missing key is treated as an empty string.
    #[test]
    fn write_rows_missing_key_produces_empty_cell(
        // Use unique headers; a duplicate header would
        // cause the row's keys to dedupe in the HashMap
        // and the test's value-cell assertion would be
        // muddled.
        headers in prop::collection::vec("[A-Z][a-z]{1,4}", 2..3)
            .prop_map(|mut v| { v.dedup(); v })
            .prop_filter("must have 2 unique headers", |v| v.len() == 2),
    ) {
        // Row with only the first header, missing the
        // second.
        let h0 = headers[0].clone();
        let h1 = headers[1].clone();
        let mut row = HashMap::new();
        row.insert(h0.clone(), "v0".to_string());
        // Intentionally do NOT insert h1.
        let rows = vec![row];
        let mut captured = CapturedWrite::default();
        write_rows_to_writer(&rows, &headers, &mut captured).expect("write");
        let body = String::from_utf8_lossy(&captured.bytes);
        let cells = parse_csv_line(body.trim());
        prop_assert_eq!(cells.len(), 2);
        prop_assert_eq!(cells[0].clone(), "v0");
        let h1_dbg = h1.clone();
        prop_assert_eq!(cells[1].clone(), "", "missing key {:?} must produce empty cell", h1_dbg);
    }
}

/// Parse a single CSV line into a `Vec<String>`, respecting
/// double-quoted fields that may contain commas. This is a
/// minimal CSV parser sufficient for the cell-count check
/// (we don't need full RFC 4180 here; just enough to split
/// fields on unquoted commas).
fn parse_csv_line(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    for c in line.chars() {
        if c == '"' {
            in_quotes = !in_quotes;
            continue;
        }
        if c == ',' && !in_quotes {
            cells.push(std::mem::take(&mut current));
            continue;
        }
        current.push(c);
    }
    cells.push(current);
    cells
}
