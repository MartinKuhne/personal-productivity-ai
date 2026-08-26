# Feature Specification: System Library Skills (VFS-120..123)

## Summary
The system library provides a `Skills` folder structured into `Note`, `Folder`, and `Batch` subdirectories. Files placed in these folders are automatically discovered and surfaced in the application's user interface:
- Files in `Skills/Note` appear as context menu actions when right-clicking any note in the directory tree or tab bar, executing the skill file's content as an agent prompt on that note.
- Files in `Skills/Folder` appear as context menu actions when right-clicking any folder in the directory tree, executing the skill file's content as an agent prompt on that folder.
- Files in `Skills/Batch` appear as prompt options in the Batch prompt-processing dialog.

## Requirements (EARS)

### System Skills Directory Structure
- [VFS-120] The system library SHALL support a `Skills` folder with `Note`, `Folder` and `Batch` subdirectories.
  - Storage location: `%APPDATA%/fastmd/system/Skills` on Windows (with user profile fallback).
  - When `%APPDATA%/fastmd/system/Skills` or any of its subdirectories (`Note`, `Folder`, `Batch`) do not exist, the system SHALL create them.

### Note Skills Context Menu
- [VFS-121] When one or more files are present in the `Skills/Note` folder, the system SHALL offer them as an option in the context menu when the user right-clicks on a note, either in the directory tree or on an open tab. The system SHALL then execute an agent prompt with the contents of the file as the user prompt.

### Folder Skills Context Menu
- [VFS-122] When one or more files are present in the `Skills/Folder` folder, the system SHALL offer them as an option in the context menu when the user right-clicks on a folder in the directory tree. The system SHALL then execute an agent prompt with the contents of the file as the user prompt.

### Batch Skills Integration
- [VFS-123] When one or more files are present in the `Skills/Batch` folder, the system SHALL offer them as an option in the batch dialog.

## User Stories

### User Story 1: Note Skills on Context Menu (P1)
As a user right-clicking on a document in the tree or active tab,
I want to see my custom note skills (e.g. "Proofread", "Summarize") in the context menu,
So that I can trigger one-click agent tasks on the selected document.

### User Story 2: Folder Skills on Context Menu (P2)
As a user right-clicking on a folder in the tree,
I want to see my custom folder skills (e.g. "Index Folder", "Audit Notes") in the context menu,
So that I can trigger one-click agent tasks on the selected folder.

### User Story 3: Batch Skills in Batch Dialog (P3)
As a user configuring a batch prompt run in the Batch Dialog,
I want to select from prompts stored in `Skills/Batch`,
So that I can execute reusable batch routines across files in a directory.

## Success Criteria
1. System library initialization ensures `Skills/`, `Skills/Note/`, `Skills/Folder/`, and `Skills/Batch/` directories are created.
2. Placing files into `Skills/Note` renders menu items in file context menus in the tree and tab bar. Clicking an item submits an agent prompt with the file's content.
3. Placing files into `Skills/Folder` renders menu items in directory context menus in the tree. Clicking an item submits an agent prompt with the file's content.
4. Placing files into `Skills/Batch` includes those prompt files in the batch dialog's prompt selector.
5. All unit tests and integration tests pass quality gates.
