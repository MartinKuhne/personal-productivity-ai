# Quickstart: Batch Prompt Processing

**Feature**: Batch Prompt Processing  
**Date**: 2026-07-20  
**Status**: Draft

---

## Prerequisites

1. **Build the application**:
   ```bash
   cd src/desktop
   cargo build --release
   ```

2. **Prepare test content**:
   - Create a content library with markdown files
   - Add a prompt file with `tags: [prompt]` in front matter
   
   Example prompt (`test_prompt.md`):
   ```markdown
   ---
   tags: [prompt]
   ---
   
   Summarize the following document in 3 bullet points:
   
   {{content}}
   ```

3. **Configure LLM API key** in `config.toml` (required for agent execution):
   ```toml
   [models.my-model]
   model = "gpt-4o-mini"
   api_url = "https://api.openai.com/v1"
   api_key = "sk-..."
   use_case = ["chat"]
   ```

---

## Test Scenarios

### Scenario 1: Open Batch Dialog
**Goal**: Verify "Batch ..." button opens dialog with all controls

**Steps**:
1. Launch app: `cargo run --release`
2. Wait for indexing to finish (top bar shows "Indexing finished (N files)")
3. Click "Batch ..." button in top navigation bar
4. **Expected**: Dialog opens with:
   - Directory dropdown (populated with content libraries)
   - Wildcard pattern text field (default "*.md")
   - Prompt dropdown (shows prompt files)
   - Batch mode radio: File / Directory
   - Concurrency dropdown: 1-8 (default 4)
   - Cancel and Process buttons

### Scenario 2: File Mode Batch Processing
**Goal**: Process multiple files with a prompt

**Steps**:
1. Open Batch dialog
2. Select directory: "My Notes" (content library)
3. Pattern: `*.md`
4. Select prompt: "Summarize Document"
5. Mode: **File**
6. Concurrency: **3**
7. Click **Process**
8. **Expected**:
   - Process button disables immediately
   - Background log shows "Batch session started: File mode, N jobs, concurrency 3"
   - For each file: "Starting batch job N: path/to/file.md" → "Completed batch job N: path/to/file.md"
   - Up to 3 jobs run concurrently
   - When all done: "Batch session completed: N/N jobs"
   - Process button re-enables

### Scenario 3: Directory Mode Batch Processing
**Goal**: Process subdirectories with a prompt

**Steps**:
1. Open Batch dialog
2. Select directory: "Projects" (has subdirs project1/, project2/, etc.)
3. Mode: **Directory** (pattern field hides)
4. Select prompt: "Analyze Project"
5. Concurrency: **2**
6. Click **Process**
7. **Expected**:
   - Pattern field hidden within 100ms
   - Jobs created per subdirectory
   - Background log shows directory paths
   - Max 2 concurrent

### Scenario 4: Cancel Before Processing
**Goal**: Cancel dialog without side effects

**Steps**:
1. Open Batch dialog
2. Configure any valid settings
3. Click **Cancel**
4. **Expected**: Dialog closes immediately, no log entries, no files modified

### Scenario 5: Cancel During Processing
**Goal**: Graceful cancellation mid-batch

**Steps**:
1. Start a batch with 10+ files, concurrency 2
2. Wait for 2-3 jobs to complete
3. Click **Cancel**
5. **Expected**:
   - No new jobs start (running_jobs stops increasing)
   - Currently running jobs finish (Running → Completed)
   - Dialog closes after in-flight jobs complete
   - Log shows "Batch session cancelled" with counts

### Scenario 6: Mode Switching Hides/Shows Pattern
**Goal**: Verify UI reacts to mode change

**Steps**:
1. Open Batch dialog
2. Select **Directory** mode
3. **Expected**: Pattern field hidden within 100ms
4. Select **File** mode
5. **Expected**: Pattern field shown within 100ms

### Scenario 7: Empty Results Handling
**Goal**: Graceful handling of no matches

**Steps**:
1. File mode: Select directory, pattern `*.nonexistent`
2. Click Process
3. **Expected**: Session completes with 0 jobs, log shows "Batch session completed: 0/0 jobs"
4. Directory mode: Select directory with no subdirectories
5. Click Process
6. **Expected**: Same behavior

### Scenario 8: Concurrency Limit Respected
**Goal**: Verify max concurrent jobs

**Steps**:
1. Create 10 test files
2. Set concurrency = 1
3. Start batch, observe log timestamps → sequential
4. Set concurrency = 4
5. Start batch, observe → max 4 overlapping

---

## Validation Commands

```bash
# Run unit tests for new batch module
cd src/desktop
cargo test batch_

# Run integration tests (if any)
cargo test --test batch_integration

# Run all tests to ensure no regressions
cargo test
```

---

## Expected Log Output (Background Log Window)

```
[2026-07-20 14:30:01.123] [Batch] Batch session started: File mode, 5 jobs, concurrency 3
[2026-07-20 14:30:01.124] [Batch] Starting batch job 0: notes/meeting.md
[2026-07-20 14:30:01.125] [Batch] Starting batch job 1: notes/todo.md
[2026-07-20 14:30:01.126] [Batch] Starting batch job 2: notes/ideas.md
[2026-07-20 14:30:15.432] [Batch] Completed batch job 0: notes/meeting.md
[2026-07-20 14:30:15.433] [Batch] Starting batch job 3: notes/archive.md
[2026-07-20 14:30:28.901] [Batch] Completed batch job 1: notes/todo.md
[2026-07-20 14:30:42.105] [Batch] Completed batch job 2: notes/ideas.md
[2026-07-20 14:30:42.106] [Batch] Starting batch job 4: notes/draft.md
[2026-07-20 14:30:55.789] [Batch] Completed batch job 3: notes/archive.md
[2026-07-20 14:31:08.234] [Batch] Completed batch job 4: notes/draft.md
[2026-07-20 14:31:08.235] [Batch] Batch session completed: 5/5 jobs
```

---

## Error Cases to Verify

| Error Condition | Expected Behavior |
|-----------------|-------------------|
| No API key configured | AgentFailed message in log, job marked Failed |
| Prompt file deleted mid-batch | Job fails, session continues |
| LLM API rate limit | Job fails with error, session continues |
| Network failure | Job fails, session continues |
| Invalid glob pattern | Dialog validation error, Process disabled |

---

## Performance Benchmarks

| Metric | Target |
|--------|--------|
| Dialog open time | < 1 second |
| Configuration time | < 30 seconds |
| Process button disable latency | < 100ms |
| Mode switch UI update | < 100ms |
| Cancel response (new jobs) | < 500ms |