import os
import re

def replace_in_file(filepath):
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()

    original = content
    content = content.replace("crate::config::AppConfig", "crate::agent::config::AgentConfig")
    content = content.replace("crate::config::AgentConfig", "crate::agent::config::AgentConfig")
    content = content.replace("use crate::config::AppConfig;", "use crate::agent::config::AgentConfig;")
    content = content.replace("use crate::config::{AppConfig, LlmConfig};", "use crate::agent::config::AgentConfig;\nuse crate::config::LlmConfig;")
    content = content.replace("AppConfig::default()", "AgentConfig::default()")
    content = content.replace("AppConfig", "AgentConfig")
    
    # Fix the specific pattern AppConfig { ... ..AppConfig::default() }
    # This is trickier since AgentConfig's fields are private. 
    # Actually, if we just replace it with AgentConfig::default() for now in tests, or use the builder.
    # Let's use a regex to replace `AgentConfig { \n field: value, \n ..AgentConfig::default() }` 
    # with `AgentConfigBuilder::new().with_field(value).build()`
    # But it might be too complex for a simple script. 
    # For now, let's just do the basic string replacements. 
    
    # Fix AppConfig -> AgentConfig
    if content != original:
        with open(filepath, 'w', encoding='utf-8') as f:
            f.write(content)
        print(f"Updated {filepath}")

def main():
    agent_dir = r"C:\Users\mkuhn\src\ppai\src\desktop\src\agent"
    integrations_dir = r"C:\Users\mkuhn\src\ppai\src\desktop\src\integrations"
    for d in [agent_dir, integrations_dir]:
        for root, dirs, files in os.walk(d):
            for file in files:
                if file.endswith(".rs") and "test" in file:
                    replace_in_file(os.path.join(root, file))
    
    # Also fix some specific files that we know failed
    extra_files = [
        r"C:\Users\mkuhn\src\ppai\src\desktop\src\integrations\dav\card_tests.rs",
        r"C:\Users\mkuhn\src\ppai\src\desktop\src\agent\tools\descriptor.rs"
    ]
    for ef in extra_files:
        if os.path.exists(ef):
            replace_in_file(ef)

if __name__ == "__main__":
    main()
