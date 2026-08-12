import os

path = 'src/desktop/src/agent/agent_impl.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace(
    'let tools_json = ctx\n        .tool_manager\n        .load()\n        .get_tools_schema(&ctx.config, &ctx.prompt);',
    '''ctx.tool_manager.rcu(|mgr| {
        let mut new_mgr = (**mgr).clone();
        new_mgr.update_and_refresh(&ctx.config);
        new_mgr
    });
    let tools_json = ctx.tool_manager.load().get_schema(&ctx.config, &ctx.prompt);'''
)
with open(path, 'w', encoding='utf-8', newline='\n') as f:
    f.write(content)

path = 'src/desktop/src/agent/tools/registry/mod.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

content = content.replace(
    'pub fn get_tools_schema(&mut self, config: &AppConfig, prompt: &str) -> serde_json::Value {',
    '''pub fn update_and_refresh(&mut self, config: &AppConfig) {
        self.mcp_manager.update_config(config);
        self.refresh_mcp_tools(config);
        self.refresh_state(config);
    }
    
    pub fn get_tools_schema(&mut self, config: &AppConfig, prompt: &str) -> serde_json::Value {'''
)
with open(path, 'w', encoding='utf-8', newline='\n') as f:
    f.write(content)


path = 'src/desktop/src/ui/tools_dialog.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()
content = content.replace(
    '''    let groups = app
        .orchestrator
        .tool_manager
        .load()
        .groups_snapshot(app.config());''',
    '''    app.orchestrator.tool_manager.rcu(|mgr| {
        let mut new_mgr = (**mgr).clone();
        new_mgr.refresh_state(app.config());
        new_mgr
    });
    let groups = app.orchestrator.tool_manager.load().groups();'''
)
with open(path, 'w', encoding='utf-8', newline='\n') as f:
    f.write(content)

path = 'src/desktop/src/ui/tools_dialog_tests.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()
content = content.replace(
    '''        let tm = app.orchestrator.tool_manager.load();
        tm.groups_snapshot(app.config());''',
    '''        app.orchestrator.tool_manager.rcu(|mgr| {
            let mut new_mgr = (**mgr).clone();
            new_mgr.refresh_state(app.config());
            new_mgr
        });'''
)
with open(path, 'w', encoding='utf-8', newline='\n') as f:
    f.write(content)
