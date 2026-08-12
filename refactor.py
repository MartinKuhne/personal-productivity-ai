import os
import re

def replace_in_file(path, replacements):
    if not os.path.exists(path):
        return
    with open(path, 'r', encoding='utf-8') as file:
        content = file.read()
    
    nc = content
    for old, new in replacements:
        nc = nc.replace(old, new)
        
    if nc != content:
        with open(path, 'w', encoding='utf-8', newline='\n') as file:
            file.write(nc)

# 1. agent/manager.rs
replace_in_file('src/desktop/src/agent/manager.rs', [
    ('AgentSessionManager', 'AgentSession'),
    ('manager_tests.rs', 'session_tests.rs'),
])
replace_in_file('src/desktop/src/agent/manager_tests.rs', [
    ('AgentSessionManager', 'AgentSession'),
])
replace_in_file('src/desktop/src/agent/mod.rs', [
    ('mod manager;', 'mod session;'),
    ('pub use manager::AgentSessionManager;', 'pub use session::AgentSession;'),
    ('AgentSessionManager::', 'AgentSession::'),
])
replace_in_file('src/desktop/src/app/orchestrator.rs', [
    ('AgentSessionManager', 'AgentSession'),
])
replace_in_file('src/desktop/src/ui/app/render.rs', [
    ('AgentSessionManager', 'AgentSession'),
])

os.rename('src/desktop/src/agent/manager.rs', 'src/desktop/src/agent/session.rs')
os.rename('src/desktop/src/agent/manager_tests.rs', 'src/desktop/src/agent/session_tests.rs')

# 2. agent/tools/manager/mod.rs
replace_in_file('src/desktop/src/agent/tools/manager/mod.rs', [
    ('ToolManager', 'ToolRegistry'),
])
replace_in_file('src/desktop/src/agent/tools/mod.rs', [
    ('pub mod manager;', 'pub mod registry;'),
    ('pub use manager::ToolManager;', 'pub use registry::ToolRegistry;'),
    ('manager::ToolManager', 'registry::ToolRegistry'),
])
replace_in_file('src/desktop/src/agent/session.rs', [
    ('ToolManager', 'ToolRegistry'),
])
replace_in_file('src/desktop/src/agent/session_tests.rs', [
    ('ToolManager', 'ToolRegistry'),
])

os.rename('src/desktop/src/agent/tools/manager', 'src/desktop/src/agent/tools/registry')

# 3. integrations/discord/context.rs
replace_in_file('src/desktop/src/integrations/discord/context.rs', [
    ('ContextManager', 'DiscordContext'),
])
replace_in_file('src/desktop/src/integrations/discord/mod.rs', [
    ('ContextManager', 'DiscordContext'),
])

# 4. integrations/mcp/manager.rs
replace_in_file('src/desktop/src/integrations/mcp/manager.rs', [
    ('McpClientManager', 'McpClients'),
    ('ManagerState', 'InnerState'),
])
replace_in_file('src/desktop/src/integrations/mcp/manager_tests.rs', [
    ('McpClientManager', 'McpClients'),
])
replace_in_file('src/desktop/src/integrations/mcp/mod.rs', [
    ('mod manager;', 'mod clients;'),
    ('pub use manager::McpClientManager;', 'pub use clients::McpClients;'),
    ('pub use manager::McpClients;', 'pub use clients::McpClients;'),
    ('McpClientManager', 'McpClients'),
])
replace_in_file('src/desktop/src/app/orchestrator.rs', [
    ('McpClientManager', 'McpClients'),
])
replace_in_file('src/desktop/src/app/batch/executor.rs', [
    ('McpClientManager', 'McpClients'),
])
replace_in_file('src/desktop/src/ui/modals.rs', [
    ('McpClientManager', 'McpClients'),
])
replace_in_file('src/desktop/src/ui/panels/top.rs', [
    ('McpClientManager', 'McpClients'),
])

os.rename('src/desktop/src/integrations/mcp/manager.rs', 'src/desktop/src/integrations/mcp/clients.rs')
if os.path.exists('src/desktop/src/integrations/mcp/manager_tests.rs'):
    os.rename('src/desktop/src/integrations/mcp/manager_tests.rs', 'src/desktop/src/integrations/mcp/clients_tests.rs')

print("Refactoring complete.")
