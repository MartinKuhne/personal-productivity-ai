# Interface Contracts: CSV Database Tools

## LLM Tool Definitions

### `create_csv`
- **Parameters**:
  - `db_name`: string (Name of the database)
  - `headers`: array of string (List of column names)
- **Returns**: Success confirmation.

### `add_rows`
- **Parameters**:
  - `db_name`: string
  - `rows`: array of object (Array of key-value pairs mapping header names to row values)
- **Returns**: Number of rows added or strict schema mismatch error.

### `delete_rows`
- **Parameters**:
  - `db_name`: string
  - `predicate`: string (`evalexpr` expression identifying rows to delete)
- **Returns**: Number of rows deleted.

### `list_csv`
- **Parameters**: None
- **Returns**: Array of string (Available database names).

### `query`
- **Parameters**:
  - `db_name`: string
  - `predicate`: string (optional, filter expression)
  - `aggregate_col`: string (optional, column to aggregate)
  - `aggregate_func`: string (optional, "sum" or "average")
- **Returns**: JSON object containing matched rows and/or aggregated numerical result.
