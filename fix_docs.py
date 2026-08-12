import os

def insert_docs(path, patterns):
    if not os.path.exists(path): return
    with open(path, 'r', encoding='utf-8') as f:
        lines = f.readlines()
        
    for var_name, doc in patterns.items():
        for i, line in enumerate(lines):
            if line.strip().startswith(f"const {var_name}:") or line.strip().startswith(f"pub const {var_name}:"):
                if i == 0 or not lines[i-1].strip().startswith("///"):
                    indent = line[:len(line) - len(line.lstrip())]
                    lines.insert(i, f"{indent}/// {doc}\n")
                break
                
    with open(path, 'w', encoding='utf-8', newline='\n') as f:
        f.writelines(lines)

insert_docs('src/desktop/src/integrations/mcp/session.rs', {
    'MAX_PAGES': 'Maximum number of pages to fetch when listing MCP resources.',
    'CLIENT_VERSION': 'The fastmd client version reported to MCP servers.',
})

insert_docs('src/desktop/src/integrations/weather/mod.rs', {
    'NWS_BASE': 'The base URL for the National Weather Service API.',
})

insert_docs('src/desktop/src/ui/tools_dialog.rs', {
    'ROW_HEIGHT': 'The row height for tool items in the dialog list.',
    'HEADER_HEIGHT': 'The height of the table header row.',
    'EXTRA_PADDING': 'Extra padding for the dialog scroll area.',
    'MIN_HEIGHT': 'The minimum height of the tools dialog.',
    'MIN_WIDTH': 'The minimum width of the tools dialog.',
    'DEFAULT_WIDTH': 'The default width of the tools dialog.',
})

insert_docs('src/desktop/src/ui/app/mod.rs', {
    'PERSISTED_UI_STATE_KEY': 'The key used to persist the eframe UI state.',
    'FONT_SCALE_MAX': 'The maximum font scaling factor allowed.',
})

print("Docstrings added.")
