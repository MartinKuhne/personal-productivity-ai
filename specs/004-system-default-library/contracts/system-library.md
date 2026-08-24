# Contracts: System Library & Conversation Logging

## VFS Contract
- Root folder: `%APPDATA%/fastmd/system`
- Name: Configured `system_library_name` or `"System"`
- Kind: `"text"`
- Readonly: `false`

## Log File Contract
- Path: `<system_library_root>/Conversations/YYYY-MM-DD HH-MM-SS.md`
- Multi-turn format:
```markdown
## Prompt (1)
<User prompt>

## Response (1)
<Assistant response>

<Write tool call details (if any)>

## Prompt (2)
<User prompt 2>

## Response (2)
<Assistant response 2>
```
