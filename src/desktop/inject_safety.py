#!/usr/bin/env python3
"""Add `fn safety` to read-only tool impls in tools/registry.rs.

Walks each `impl Tool for XxxTool` block and inserts `fn safety` right
after `fn is_enabled`. The `is_enabled` body is preserved verbatim
(including any complex multi-line bodies) by matching the body up to
the first `}` that closes the function.
"""
import re
from pathlib import Path

REGISTRY = Path(r"C:\Users\mkuhn\src\ppai\src\desktop\src\tools\registry.rs")

# Tool names whose Tool impls are read-only and should opt into parallel dispatch.
READ_ONLY = {
    "grep", "read_tags", "list_files_by_tag", "list_files", "read_file",
    "read_file_lines", "web_fetch", "read_yaml_header", "web_search",
    "search_calendar", "get_calendar", "get_calendar_item",
    "search_email", "get_email_by_id", "search_contact", "get_contact",
    "list_csv", "query",
}

# Match the name() function so we know the tool's name and can anchor.
NAME_RE = re.compile(
    r'fn name\(&self\) -> &\x27static str \{\s*"(?P<name>[a-z_]+)"\s*\}'
)

# Match an `is_enabled` body that doesn't itself contain a `}` (e.g. a
# single boolean expression, optionally with `&&`/`||` and method calls).
IS_ENABLED_BODY_RE = re.compile(
    r'fn is_enabled\(&self, [^)]*\) -> bool \{\s*(?P<body>(?:[^{}]|\{[^{}]*\})*?)\}',
    re.DOTALL,
)


src = REGISTRY.read_text(encoding="utf-8")
out = []
last = 0
added = 0
seen = set()

for name_m in NAME_RE.finditer(src):
    name = name_m.group("name")
    if name not in READ_ONLY or name in seen:
        continue
    # Find the next is_enabled after this name() within the same impl block.
    search_from = name_m.end()
    is_m = IS_ENABLED_BODY_RE.search(src, search_from)
    if is_m is None:
        continue
    # Confirm the is_enabled is still inside the same impl block (i.e. the
    # next `fn execute` or `}` of the impl block hasn't appeared).
    impl_end = src.find("fn execute", search_from)
    if impl_end != -1 and is_m.start() > impl_end:
        continue
    # Insert safety after is_enabled.
    insert_pos = is_m.end()
    out.append(src[last:insert_pos])
    out.append(
        "\n    fn safety(&self) -> crate::tools::Safety {\n"
        "        crate::tools::Safety::ReadOnly\n"
        "    }"
    )
    last = insert_pos
    seen.add(name)
    added += 1

out.append(src[last:])
REGISTRY.write_text("".join(out), encoding="utf-8")
print(f"Injected safety() into {added} of {len(READ_ONLY)} read-only tool impls")
print("Missing:", sorted(READ_ONLY - seen))
