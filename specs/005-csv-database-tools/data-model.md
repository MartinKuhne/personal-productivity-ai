# Data Model: CSV Database Tools

## Entities

### `CsvDatabase`
Represents a CSV file residing in the storage directory.
- **Fields**:
  - `name`: String (The name of the database/file without extension)
  - `path`: PathBuf (Absolute path to the file)
  - `headers`: Vec<String> (Parsed headers of the CSV)

### `QueryRequest`
Represents an invocation of the `query` tool.
- **Fields**:
  - `db_name`: String
  - `predicate`: Option<String> (The `evalexpr` compatible expression, e.g., "age > 30")
  - `aggregate_column`: Option<String> (Column name to aggregate)
  - `aggregate_function`: Option<String> ("sum" or "average")

### `QueryResponse`
Represents the result of a query.
- **Fields**:
  - `rows`: Vec<Vec<String>> (Filtered data rows)
  - `aggregate_result`: Option<f64> (Result of the sum or average)

## State Transitions & Validation
- **Insertion**: `add_rows` validates that the provided rows have a length equal to `headers.len()` and that the keys map exactly to the headers.
- **Type Inference**: During query evaluation, numeric strings are parsed to `f64` or `i64` using Rust's `str::parse` before passing into the `evalexpr::Context`. Strings failing this parse are kept as strings.
