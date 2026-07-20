# Quickstart Validation Guide: CSV Database Tools

## Prerequisites
Ensure Rust is installed and you are working within `src/desktop`.
Storage directory `%APPDATA%\fastmd\db\` (or equivalent) will be created automatically if it doesn't exist.

## Validation Scenario

1. **Create Database**:
   Invoke `create_csv` with `db_name = "employees"` and `headers = ["id", "name", "salary"]`.
   - *Expected outcome*: Success message and `employees.csv` created in the db directory with the correct header line.

2. **Add Valid Rows**:
   Invoke `add_rows` with `db_name = "employees"` and `rows = [{"id":"1", "name":"Alice", "salary":"50000"}, {"id":"2", "name":"Bob", "salary":"60000"}]`.
   - *Expected outcome*: 2 rows added successfully.

3. **Trigger Schema Validation Error**:
   Invoke `add_rows` with `db_name = "employees"` and `rows = [{"id":"3", "name":"Charlie"}]`. (Missing 'salary' column).
   - *Expected outcome*: Explicit error rejecting the insertion due to strict schema mismatch.

4. **Execute Query**:
   Invoke `query` with `db_name = "employees"` and `predicate = "salary > 55000"`.
   - *Expected outcome*: Returns only the row for Bob.

5. **Execute Aggregation**:
   Invoke `query` with `db_name = "employees"`, `aggregate_col = "salary"`, `aggregate_func = "average"`.
   - *Expected outcome*: Returns `{ "aggregate_result": 55000.0 }`.
