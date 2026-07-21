# Contracts Index

**Feature**: Batch Prompt Processing

---

## Contract Files

1. **[batch_dialog.md](./batch_dialog.md)** - UI dialog interface, configuration, validation
2. **[batch_orchestration.md](./batch_dialog.md#contract-2-batch-orchestration)** - Background execution, concurrency, agent integration
3. **[prompt_discovery.md](./batch_dialog.md#contract-3-prompt-discovery)** - Finding and reading prompt files
4. **[background_log.md](./batch_dialog.md#contract-4-background-log-integration)** - Log category extension and message formats
5. **[app_state.md](./batch_dialog.md#contract-5-app-state-integration)** - FastMdApp state modifications
6. **[errors.md](./batch_dialog.md#error-handling-contract)** - Error types and handling

---

## Summary

All contracts are internal Rust APIs. No external interfaces (HTTP, CLI, etc.) are exposed by this feature. The batch processing dialog integrates with:

- **Existing**: `FastMdApp`, `BackgroundProcessManager`, `run_agent`, content libraries, file event bus
- **New**: `BatchDialogConfig`, `BatchConfig`, `BatchHandle`, `PromptInfo`, `LogCategory::Batch`

See individual contract files for detailed interfaces and behavior specifications.