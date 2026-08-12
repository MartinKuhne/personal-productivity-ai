import os
import re

path = 'src/desktop/src/agent/tool_executor.rs'
with open(path, 'r', encoding='utf-8') as f:
    content = f.read()

# Replace test_tool_executor_new
content = re.sub(
    r'let executor = ToolExecutor::new\(\s*config,\s*bus,\s*browser_session,\s*pdf_backing,\s*cache,\s*tm,\s*uuid_gen,\s*\);',
    r'let executor = ToolExecutorBuilder::new(std::sync::Arc::new(config), bus, cache, tm)\n        .with_browser_session(browser_session)\n        .with_pdf_backing(pdf_backing)\n        .with_uuid_gen(uuid_gen)\n        .build();',
    content
)

with open(path, 'w', encoding='utf-8', newline='\n') as f:
    f.write(content)
