---
name: audit-licenses
description: Audit Cargo workspace dependencies for software license compliance. Identifies non-permissive licenses (AGPL, GPL, LGPL, MPL), font licenses, and non-standard permissive licenses, and traces reverse dependency paths back to workspace crates. Use when auditing licenses, checking compliance before release, or verifying third-party library licenses.
---

# Audit Licenses Skill

This skill audits all direct and transitive third-party dependencies across a Rust/Cargo workspace to ensure compliance with open source licensing policies (e.g., MIT, Apache-2.0, BSD).

## 1. When to Use This Skill

Activate this skill when:
- The user asks to audit or check licenses of libraries/dependencies used in the project.
- Preparing a release or distribution build to ensure compliance with the project's public license (e.g., MIT).
- Evaluating a new dependency before adding it to `Cargo.toml`.
- Investigating potential copyleft infection (AGPL, GPL, LGPL) in compiled binaries.
- Running CI quality gate checks against non-permissive licenses.

---

## 2. Bundled Script

This skill provides a reusable Python script:
`scripts/audit_licenses.py` (located in this skill folder: `.opencode/skills/audit-licenses/scripts/audit_licenses.py`).

### Key Capabilities
- **No third-party Python packages required**: Pure standard library (`json`, `subprocess`, `argparse`, `re`, `pathlib`).
- **Full transitive dependency graph inspection**: Queries `cargo metadata --format-version 1`.
- **Reverse dependency path tracing**: Shows the exact chain from workspace members to the flagged crate (e.g., `fastmd -> fastmd-agent -> evalexpr`).
- **Distinguishes runtime vs dev dependencies**: Flags whether a dependency is linked into the production binary or only used in tests/benchmarks.
- **SPDX boolean expression parsing**: Accurately resolves `OR` (multi-licensing alternatives) and `AND` (compound requirements).
- **CI / Quality Gate mode**: Exits with non-zero code via `--fail-on-copyleft`.
- **Cross-platform**: Safely handles UTF-8 / console encoding on Windows, macOS, and Linux.

---

## 3. Usage & Common Commands

Run the script from the workspace root or pass `--workspace-dir`:

### Quick Summary of Flagged / Non-Permissive Dependencies
```bash
python .opencode/skills/audit-licenses/scripts/audit_licenses.py --format summary
```

### Full Markdown Report (Filtered to Non-Permissive / Flagged Only)
```bash
python .opencode/skills/audit-licenses/scripts/audit_licenses.py --non-permissive-only
```

### Exclude Dev / Test Dependencies (Audit Runtime Only)
```bash
python .opencode/skills/audit-licenses/scripts/audit_licenses.py --non-permissive-only --no-dev-deps
```

### Export Complete Audit to JSON
```bash
python .opencode/skills/audit-licenses/scripts/audit_licenses.py --format json --output license_audit.json
```

### CI Quality Gate (Fails if Any Runtime Copyleft Dependency is Found)
```bash
python .opencode/skills/audit-licenses/scripts/audit_licenses.py --fail-on-copyleft
```

---

## 4. License Classification & Risk Hierarchy

| Category | Typical Licenses | Compatibility with MIT/Apache Distribution | Risk Level |
| :--- | :--- | :--- | :--- |
| **🚨 Strong / Network Copyleft** | `AGPL-3.0`, `GPL-2.0`, `GPL-3.0`, `SSPL` | **Incompatible**. Compels the entire application and network services to be distributed under copyleft terms. | **Critical** |
| **⚠️ Copyleft** | `LGPL-2.1`, `LGPL-3.0` | **Problematic in Rust**. Rust compiles and links crates statically. Distributing static binaries requires relinking mechanisms and GPLv3-aligned source access. | **High** |
| **⚠️ Weak Copyleft** | `MPL-2.0`, `CDDL-1.0`, `EPL-2.0` | **File-level copyleft**. Permissible in a "Larger Work" provided modifications to the MPL files themselves remain MPL-licensed. | **Low - Moderate** |
| **ℹ️ Font / Asset Licenses** | `OFL-1.1`, `Ubuntu-font-1.0` | **Conditional**. Allows bundling in applications, but prohibits selling fonts standalone or renaming derivatives without permission. | **Low** |
| **✅ Other Permissive** | `Unicode-3.0`, `Zlib`, `BSL-1.0`, `CDLA-Permissive-2.0`, `ISC`, `CC0-1.0`, `0BSD`, `Unlicense` | **Fully compatible**. Standard attribution or public domain dedication terms. | **None** |
| **✅ Standard Permissive** | `MIT`, `Apache-2.0`, `BSD-2-Clause`, `BSD-3-Clause` | **Fully compatible**. Industry standard permissive terms. | **None** |

> [!NOTE]
> **Multi-licensing (`OR`)**: If a crate is dual-licensed under `Apache-2.0 OR GPL-2.0-only` (e.g., `self_cell`) or `MIT OR LGPL-2.1` (e.g., `r-efi`), the downstream distributor can choose the permissive option (`Apache-2.0` or `MIT`).

---

## 5. Standard Audit Workflow for Agents

When requested to perform a license audit:

1. **Execute the Audit Script**:
   Run `python .opencode/skills/audit-licenses/scripts/audit_licenses.py --format json --output <temp_file>`.
2. **Examine Flagged Dependencies**:
   - Check if any packages fall under **Strong Copyleft** or **Copyleft**.
   - Check whether each flagged package is **Runtime** or **Dev-only**.
3. **Trace the Dependency Lineage**:
   - For every flagged crate, trace which workspace crate and intermediate dependency brought it in.
   - Look up the source file in the workspace where the crate is imported (`use <crate>::...`).
4. **Formulate Remediation Recommendations**:
   - **Strong Copyleft (AGPL/GPL)**: Must be replaced before public release under MIT/Apache. Propose permissive alternatives or custom in-tree implementations.
   - **LGPL in Rust**: Evaluate whether dynamic linking, alternative crates, or direct reimplementation (e.g., via `reqwest` / `quick-xml`) is possible.
   - **Dev-dependencies**: Document that they do not ship in compiled binaries, but verify they are not accidentally leaked into runtime feature flags.
5. **Present Clear Report**:
   - Present findings grouped by risk category with clickable links to the relevant `Cargo.toml` and source code locations.
