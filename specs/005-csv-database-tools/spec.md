# Feature Specification: csv-database-tools

**Feature Branch**: `[005-csv-database-tools]`

**Created**: 2026-07-19

**Status**: Draft

**Input**: User description: "### CSV Database Tools\n\n* [REQ-650] Tool Availability: The CSV database tools (`add_rows`, `delete_rows`, `create_csv`, `list_csv`, `query`) shall only be offered to the LLM if the user's query contains any of the tool names, \"table\", \"csv\", or \"database\".\n* [REQ-651] Query Evaluation: The `query` tool shall use the `evalexpr` crate to parse and execute query predicates as dynamic expressions against CSV rows.\n* [REQ-652] Aggregate Functions: The query system shall allow `sum` and `average` as aggregate functions over a specified column.\n* [REQ-653] The system shall store all csv databases in a user specified location. Default to %APPDATA%\\fastmd\\db\\ if not configured."

## Clarifications

### Session 2026-07-19

- Q: Schema Validation on Insertion (add_rows) → A: Strict validation (Error if new rows do not exactly match existing headers)
- Q: Data Type Inference for Queries → A: Best-effort inference (Automatically parse numeric fields into integers/floats for evalexpr)

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Context-Aware Tool Presentation (Priority: P1)

As an LLM attempting to fulfill user requests, I need the CSV database tools to be provided to me only when relevant to the user's prompt (mentioning table, csv, database, or tool names), so that my context window is not cluttered with unrelated tools.

**Why this priority**: Essential for LLM performance and token efficiency. 

**Independent Test**: Can be tested by simulating user prompts with and without the keywords and asserting the presence of the tools in the LLM's context.

**Acceptance Scenarios**:

1. **Given** a user query "Please parse this csv file", **When** the prompt is processed, **Then** the `add_rows`, `delete_rows`, `create_csv`, `list_csv`, and `query` tools are made available.
2. **Given** a user query "Tell me a joke", **When** the prompt is processed, **Then** none of the CSV database tools are made available.

---

### User Story 2 - Dynamic Querying (Priority: P1)

As a user or LLM analyzing data, I need to execute queries with dynamic expressions (like filtering rows where a column value > 10) against CSV files, so I can extract specific datasets.

**Why this priority**: Core functionality of the database tools.

**Independent Test**: Can be tested by invoking the `query` tool with a mock CSV and a valid expression and verifying the correct rows are returned.

**Acceptance Scenarios**:

1. **Given** a CSV with numerical columns, **When** the `query` tool is called with an expression like `age > 30`, **Then** it returns only the rows satisfying the condition.

---

### User Story 3 - Data Aggregation (Priority: P2)

As a user or LLM, I need to easily compute the sum and average of specific numeric columns across the dataset, so I can gain summary insights without manually calculating them.

**Why this priority**: Aggregations are highly valuable for data analysis, but filtering is a prerequisite.

**Independent Test**: Test by running a query with `sum` or `average` aggregation on a known dataset and checking if the numerical result is accurate.

**Acceptance Scenarios**:

1. **Given** a CSV with a `price` column, **When** a query is executed specifying `sum` aggregation on `price`, **Then** the total sum of the `price` column is returned.
2. **Given** a CSV with a `score` column, **When** a query is executed specifying `average` aggregation on `score`, **Then** the arithmetic mean of the `score` column is returned.

---

### User Story 4 - Configurable Storage (Priority: P3)

As a system administrator or user, I want the system to store all CSV databases in a designated directory (defaulting to `%APPDATA%\fastmd\db\`), so I can manage, back up, or isolate database files appropriately.

**Why this priority**: Storage management is crucial but the default provides a working baseline.

**Independent Test**: Test by creating a CSV database without explicit configuration and checking the default path, then repeating with a configured path.

**Acceptance Scenarios**:

1. **Given** no custom configuration, **When** a CSV is created via the tools, **Then** it is stored in `%APPDATA%\fastmd\db\`.
2. **Given** a custom location is configured, **When** a CSV is created, **Then** it is stored in the configured location.

### Edge Cases

- What happens when a query expression is malformed or invalid? (Should return a clear error message)
- How does the system handle querying a column that does not exist in the CSV?
- What happens if an aggregate function is applied to a non-numeric column?
- What happens if the configured or default storage location is inaccessible or read-only?
- **Schema mismatch**: The `add_rows` tool receives data with columns that don't perfectly align with the target CSV (results in strict validation error).
- **Type inference ambiguity**: A query predicate expects a string operation on a value that best-effort inference converted to a number.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: System MUST selectively expose the CSV tools (`add_rows`, `delete_rows`, `create_csv`, `list_csv`, `query`) based on keyword matching in the user query.
- **FR-002**: The required keywords for tool activation MUST be: `add_rows`, `delete_rows`, `create_csv`, `list_csv`, `query`, `table`, `csv`, `database`.
- **FR-003**: System MUST execute query predicates dynamically against rows of the target CSV, utilizing the `evalexpr` crate.
- **FR-004**: System MUST support `sum` and `average` aggregate functions applied to a specified column within a query.
- **FR-005**: System MUST store CSV database files in a default location of `%APPDATA%\fastmd\db\`.
- **FR-006**: System MUST allow users to override the default storage location via configuration.
- **FR-007**: The `add_rows` tool MUST perform strict schema validation, returning an error if the inserted rows do not exactly match the headers of the target CSV file.
- **FR-008**: System MUST employ best-effort inference during query evaluation, automatically parsing numeric strings into integers or floats before passing them to `evalexpr`.

### Key Entities 

- **CSV Database**: A comma-separated values file acting as a table, stored in the designated storage location.
- **Query**: A request containing an optional predicate and an optional aggregate function (sum/average) targeting a specific column.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Keyword-based tool filtering correctly excludes CSV tools on 100% of non-relevant queries, improving token efficiency.
- **SC-002**: Queries using dynamic expressions evaluate accurately against datasets up to 10,000 rows in under 1 second.
- **SC-003**: `sum` and `average` aggregations return mathematically correct results on numeric columns, matching equivalent spreadsheet calculations.
- **SC-004**: 100% of generated CSV files are successfully written to and read from the configured or default storage location.

## Assumptions

- Users have stable file system access with adequate permissions for the storage location.
- CSV files managed by these tools have headers defining the column names.
- The user's query context includes the raw text of their prompt for keyword scanning.
- Values evaluated by queries and aggregations can be safely parsed as appropriate numerical or string types.
