# Research: Default System Library & Conversation Logging

## 1. System Library Storage & Naming
- Windows `%APPDATA%/fastmd/system` is standard application data storage for persistent system files.
- `AppConfig::get_system_library_path()` resolves `std::env::var("APPDATA")` -> `PathBuf::from(appdata).join("fastmd").join("system")`.
- If `APPDATA` is missing, fallback to `USERPROFILE/.fastmd/system` or relative `system`.
- Display name defaults to `"System"` (VFS-101), overridden by `system_library_name` in configuration (VFS-102).

## 2. Conversation Logging File Lifecycle
- `Conversations` subfolder: `%APPDATA%/fastmd/system/Conversations`.
- Log file naming: `YYYY-MM-DD HH-MM-SS.md` using session initiation local time.
- File headings: `## Prompt (nnn)` and `## Response (nnn)` where `nnn` is 1-based index (1, 2, 3...).
- Write tools: Mutating tools (`Safety::Mutating` such as `create_note`, `patch_note`, `insert_into_note`, `move_note`, etc.) executed during the turn are appended at the end of the `## Response (nnn)` section.

## 3. Integration Seams
- On config arrival / app init, ensure the system library exists on disk and is present in `content_libraries`.
- When an agent turn runs, the conversation logger writes prompt, response, and mutating tool execution details to disk.
