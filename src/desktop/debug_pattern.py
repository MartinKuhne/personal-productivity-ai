import re
src = open(r"C:\Users\mkuhn\src\ppai\src\desktop\src\tools\registry.rs", encoding='utf-8').read()

# Same pattern as main script
PATTERN_CONFIG = re.compile(
    r'fn name\(&self\) -> &[A-Za-z\']+static str \{\s*"(?P<name>[a-z_]+)"\s*\}'
    r'(?P<between>.*?)'
    r'fn is_enabled\(&self, config: &AppConfig, _: &str\) -> bool \{\s*'
    r'(?P<cond>.*?)\}',
    re.DOTALL,
)

print("Matches with main-script pattern:")
for m in PATTERN_CONFIG.finditer(src):
    print(f"  {m.group('name')} @ {m.start()}")
