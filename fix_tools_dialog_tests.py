import os
import re

path = 'src/desktop/src/ui/tools_dialog_tests.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace(
    'let mut tm = app.orchestrator.tool_manager.write().unwrap();',
    'let tm = app.orchestrator.tool_manager.load();'
)
content = content.replace(
    'app.orchestrator.tool_manager.write().unwrap().set_group_enabled(&mut config, &id, false);',
    '''app.orchestrator.tool_manager.rcu(|mgr| {
            let mut new_mgr = (**mgr).clone();
            new_mgr.set_group_enabled(&mut config, &id, false);
            new_mgr
        });'''
)
content = content.replace(
    'app.orchestrator.tool_manager.write().unwrap().clear_error(&id);',
    '''app.orchestrator.tool_manager.rcu(|mgr| {
            let mut new_mgr = (**mgr).clone();
            new_mgr.clear_error(&id);
            new_mgr
        });'''
)
content = content.replace(
    'app.orchestrator.tool_manager.write().unwrap().record_error(&id, crate::agent::tools::registry::ToolGroupError::now(crate::agent::tools::registry::ToolErrorKind::Execution, "Test error".to_string()));',
    '''app.orchestrator.tool_manager.rcu(|mgr| {
            let mut new_mgr = (**mgr).clone();
            new_mgr.record_error(&id, crate::agent::tools::registry::ToolGroupError::now(crate::agent::tools::registry::ToolErrorKind::Execution, "Test error".to_string()));
            new_mgr
        });'''
)
with open(path, 'w', encoding='utf-8', newline='\n') as f:
    f.write(content)
