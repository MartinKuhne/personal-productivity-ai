//! User-visible description strings for the CSV database tool family.

pub const CREATE_CSV_DESCRIPTION: &str = "Create a new CSV file database with specified headers.";

pub const LIST_CSV_DESCRIPTION: &str = "List all CSV file databases.";

pub const ADD_ROWS_DESCRIPTION: &str = "Add rows to a CSV file database.";

pub const DELETE_ROWS_DESCRIPTION: &str =
    "Delete rows from a CSV file database based on a predicate.";

pub const QUERY_DESCRIPTION: &str =
    "Query a CSV file database using an evalexpr predicate, supporting sum and average aggregates.";

pub const FIELD_CREATE_CSV_INPUT_DB_NAME: &str =
    "The name of the new CSV database. Must be unique among all CSV databases.";

pub const FIELD_CREATE_CSV_INPUT_HEADERS: &str =
    "Column headers for the new CSV database, in the order they will appear in each row.";

pub const FIELD_ADD_ROWS_INPUT_DB_NAME: &str = "The name of the CSV database to add rows to.";

pub const FIELD_ADD_ROWS_INPUT_ROWS: &str = "Each row is a JSON object mapping header names to their string values; missing keys are stored as empty strings.";

pub const FIELD_DELETE_ROWS_INPUT_DB_NAME: &str =
    "The name of the CSV database to delete rows from.";

pub const FIELD_DELETE_ROWS_INPUT_PREDICATE: &str = "An `evalexpr` expression evaluated against each row; rows for which the expression is truthy are deleted.";

pub const FIELD_QUERY_REQUEST_DB_NAME: &str = "The name of the CSV database to query.";

pub const FIELD_QUERY_REQUEST_PREDICATE: &str =
    "An optional `evalexpr` expression to filter rows. If omitted, every row is included.";

pub const FIELD_QUERY_REQUEST_AGGREGATE_COL: &str =
    "The column to aggregate over. Required when `aggregate_func` is set.";

pub const FIELD_QUERY_REQUEST_AGGREGATE_FUNC: &str =
    "The aggregate function to apply. One of `sum`, `average`, or `count` (case-insensitive).";
