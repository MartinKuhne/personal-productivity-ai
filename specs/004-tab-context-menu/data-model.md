# Data Model: Tab Context Menu

## Entities

### `TabContextAction` (Conceptual / UI Action Enum)
This entity represents the actions that can be triggered from the tab context menu.
- `Close`
- `CloseOthers`
- `CloseAll`
- `CopyPath`
- `ShowInExplorer`
- `OpenInEditor`
- `FormatMarkdown`

### Application State Extensions
The application state managing tabs needs to support:
- Iterative or bulk closing of tabs to implement `Close Others` and `Close All`.
- Triggering the existing unsaved changes dialog check for each closed tab.
- Accessing the underlying `PathBuf` for the file associated with a right-clicked tab, which is required to execute the file-specific actions (Copy Path, Show in File Explorer, Open in Editor, Format Markdown).
