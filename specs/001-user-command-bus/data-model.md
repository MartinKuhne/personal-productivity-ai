# Data Model: User Command Bus

## Core Entities

### `UserCommand` (Enum)
Represents a discrete user interaction originating from the UI. It is the single source of truth for user intent.

**Traits**: `Clone`, `Send`, `'static`

**Expected Variants (grouped by UI surface)**:
- **Agent/Command Input**:
  - `RunAgent(String)`
  - `ShowModels`
  - `ShowDeprecatedModelMessage`
  - `CancelAgent`
- **Tabs**:
  - `CloseTab(usize)`
  - `CloseOtherTabs(usize)`
  - `CloseAllTabs`
- **TOC**:
  - `ScrollToHeader(String)`
- **Toolbar / Menu**:
  - `OpenBatchDialog`
  - `OpenToolsDialog`
  - `ToggleBackgroundLogs(bool)`
  - `ToggleAgentDebugWindow(bool)`
  - `SelectChatModel(String)`
  - `ChangeTableWidthStrategy(DeficitStrategy)`
  - `SelectTagFilter(Option<String>)`
- **Tools Dialog**:
  - `SetToolGroupEnabled { id: ToolGroupId, enabled: bool }`
  - `ClearToolGroupError(ToolGroupId)`
  - `StartMcpAuth(String)`
  - `ForgetMcpAuth(String)`
- **File Tree (Selection & Context Menu)**:
  - `SelectFile { path: PathBuf, multi: bool }`
  - `SelectDirectory { path: PathBuf, toggle_expand: bool }`
  - `OpenInEditor(PathBuf)`
  - `ShowInExplorer(PathBuf)`
  - `CopyPath(PathBuf)`
  - `SaveAsPdf(PathBuf)`
  - `Rename(PathBuf)`
  - `Move(PathBuf)`
  - `Delete(PathBuf)`
  - `CreateDirectory { parent: PathBuf }`
  - `CreateDocument { parent: PathBuf }`
  - `RunSkillPrompt { content: String, target_dir: Option<PathBuf>, target_file: Option<PathBuf> }`
  - `MergePrompt(Vec<PathBuf>)`
- **Dialog Confirmations**:
  - `ConfirmMove { file: PathBuf, destination: PathBuf }`
  - `ConfirmCreateDirectory { parent: PathBuf, name: String }`
  - `ConfirmCreateDocument { parent: PathBuf, name: String }`
  - `ConfirmRename { path: PathBuf, new_name: String }`
  - `StartBatch(BatchConfig)`
  - `CancelBatch`
- **Shortcuts**:
  - `ToggleAgentDebugWindowShortcut`

### `UserCommandProducer` (Struct)
A lightweight cloneable handle wrapping the `Bus<UserCommand>` transmitter. Passed into UI panels and contexts (e.g. `TreeNodeContext`) so they can emit commands without needing a mutable orchestrator borrow.
- **Fields**: `bus: Bus<UserCommand>`
- **Methods**: `publish(cmd: UserCommand)`

### `CommandExecutor` (Module / Implementation Block)
An execution boundary residing on the `AppOrchestrator` side. It drains the `Bus<UserCommand>` and mutates the application state.
- **Methods**: `apply_user_command(&mut self, cmd: UserCommand)`

## State Transitions

- **UI Interaction** -> `UserCommand` is minted and passed to `UserCommandProducer::publish`.
- **Frame Render Cycle** -> `AppOrchestrator::drain_user_command_bus` reads the command from the bus.
- **Command Execution** -> `apply_user_command` handles the variant, directly updating orchestrator fields (e.g., managing files, launching agents) without deferred slots like `submit_prompt`.
