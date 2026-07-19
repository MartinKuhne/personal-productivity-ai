# Quickstart Validation: Tab Context Menu

## Setup
1. Launch the application: `cargo run --bin fastmd`.
2. Open at least 3 files (so there are 3 tabs).
3. Modify at least one file so it has unsaved changes.

## Validation Scenarios

### Scenario 1: Basic Close
1. Right-click on a tab and select "Close".
2. **Expected**: The tab closes. If it had unsaved changes, you are prompted to save or discard.

### Scenario 2: Close Others
1. Right-click on the middle tab and select "Close Others".
2. **Expected**: All other tabs close except the selected one. Any tabs with unsaved changes among the closed ones will prompt for confirmation.

### Scenario 3: Close All
1. Open multiple tabs again and modify one.
2. Right-click on any tab and select "Close All".
3. **Expected**: All tabs attempt to close. The modified tab will prompt for confirmation.

### Scenario 4: File Operations
1. Right-click on a tab and select "Copy Path".
2. **Expected**: The file's path is in your clipboard (paste into Notepad to verify).
3. Right-click on the tab and select "Show in File Explorer".
4. **Expected**: The OS file explorer opens highlighting the file.
5. Right-click on the tab and select "Open in Editor".
6. **Expected**: The file opens in the default system editor for its extension.
7. Right-click on the tab and select "Format Markdown".
8. **Expected**: The background agent triggers to format the markdown document.
