//! User-visible description strings for the CSV database tool family.

pub const CREATE_CSV_DESCRIPTION: &str = "Create a CSV database with specified column headers.";

pub const LIST_CSV_DESCRIPTION: &str = "List all CSV databases.";

pub const ADD_ROWS_DESCRIPTION: &str = "Add rows to a CSV database.";

pub const DELETE_ROWS_DESCRIPTION: &str = "Delete rows from a CSV database using an expression.";

pub const QUERY_DESCRIPTION: &str =
    "Query a CSV database using an expression or aggregate function.";

pub const FIELD_CREATE_CSV_INPUT_DB_NAME: &str = "Specify a unique name for the new CSV database.";

pub const FIELD_CREATE_CSV_INPUT_HEADERS: &str =
    "Provide column headers for the new CSV database in sequential order.";

pub const FIELD_ADD_ROWS_INPUT_DB_NAME: &str = "Specify the target CSV database name.";

pub const FIELD_ADD_ROWS_INPUT_ROWS: &str = "Provide rows as JSON objects mapping header names to string values. The system stores missing keys as empty strings.";

pub const FIELD_DELETE_ROWS_INPUT_DB_NAME: &str = "Specify the target CSV database name.";

pub const FIELD_DELETE_ROWS_INPUT_PREDICATE: &str = "Specify an expression to evaluate each row. The tool deletes rows where the expression returns true.";

pub const FIELD_QUERY_REQUEST_DB_NAME: &str = "Specify the target CSV database name.";

pub const FIELD_QUERY_REQUEST_PREDICATE: &str = "Specify an optional expression to filter rows. If you omit the expression, the tool evaluates all rows.";

pub const FIELD_QUERY_REQUEST_AGGREGATE_COL: &str =
    "Specify the column to aggregate. Provide this value when you set `aggregate_func`.";

pub const FIELD_QUERY_REQUEST_AGGREGATE_FUNC: &str =
    "Specify the aggregate function: `sum`, `average`, or `count`.";
