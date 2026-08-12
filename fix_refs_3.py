import os

def replace_in_dir(directory, replacements):
    for root, dirs, files in os.walk(directory):
        for name in files:
            if not name.endswith('.rs'):
                continue
            path = os.path.join(root, name)
            with open(path, 'r', encoding='utf-8') as file:
                content = file.read()
            
            nc = content
            for old, new in replacements:
                nc = nc.replace(old, new)
                
            if nc != content:
                with open(path, 'w', encoding='utf-8', newline='\n') as file:
                    file.write(nc)

replace_in_dir('src/desktop', [
    ('ToolManager', 'ToolRegistry'),
    ('AgentSessionManager', 'AgentSession'),
])

print("Global replace complete.")
