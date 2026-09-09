#!/usr/bin/env python3
"""
audit-perf.py — Automated runner for Workflow W8 (Performance, Dependencies, Binary Size, and Compile Times).

Emits findings in the standard Code Quality Report markdown table format.
"""

import argparse
import json
import os
import re
import sys

import importlib.util

# Import sibling modules
script_dir = os.path.dirname(os.path.abspath(__file__))

def load_sibling(module_name, filename):
    filepath = os.path.join(script_dir, filename)
    spec = importlib.util.spec_from_file_location(module_name, filepath)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod

analyze_deps = load_sibling("analyze_deps", "analyze-deps.py")
analyze_bloat = load_sibling("analyze_bloat", "analyze-bloat.py")


def run_w8_audit(package="fastmd"):
    repo_root = os.path.abspath(
        os.path.join(script_dir, "..", "..", "..", "..")
    )

    findings = []
    finding_id = 1

    # 1. Dependency Analysis
    deps_data = analyze_deps.analyze(package=package, cwd=repo_root)

    # Check for heavyweight dependencies (>50 exclusive crates)
    for dep in deps_data["dependencies"]:
        if dep["exclusive_count"] >= 50:
            findings.append(
                {
                    "id": f"W8-{finding_id:02d}",
                    "severity": "HIGH",
                    "location": f"Cargo.toml (dep: {dep['name']})",
                    "summary": f"Heavyweight subsystem adds {dep['exclusive_count']} exclusive crates",
                    "evidence": f"Transitive: {dep['transitive_count']} crates, Exclusive: {dep['exclusive_count']} crates",
                    "recommendation": f"Evaluate feature-gating or decoupling '{dep['name']}' into an on-demand CLI sidecar",
                }
            )
            finding_id += 1

    # Check for duplicate crate versions
    if deps_data["duplicates"]:
        for crate, versions in deps_data["duplicates"].items():
            findings.append(
                {
                    "id": f"W8-{finding_id:02d}",
                    "severity": "MEDIUM",
                    "location": "Cargo.lock",
                    "summary": f"Duplicate versions of crate '{crate}' in build graph",
                    "evidence": f"Versions detected: {', '.join(versions)}",
                    "recommendation": f"Align dependency version bounds to unify '{crate}' onto a single version",
                }
            )
            finding_id += 1

    # 2. Binary Bloat & Profile Analysis
    bloat_data = analyze_bloat.analyze(repo_root)
    profile = bloat_data["profile_settings"]

    if not profile["strip"]:
        findings.append(
            {
                "id": f"W8-{finding_id:02d}",
                "severity": "HIGH",
                "location": ".cargo/config.toml (or Cargo.toml)",
                "summary": "Release profile does not strip symbols (strip = true missing)",
                "evidence": "Debug symbols and DWARF/PDB tables remain embedded in release executable",
                "recommendation": "Add `strip = true` to `[profile.release]` to cut binary size in half",
            }
        )
        finding_id += 1

    for asset_flag in bloat_data["embedded_asset_flags"]:
        findings.append(
            {
                "id": f"W8-{finding_id:02d}",
                "severity": "MEDIUM",
                "location": asset_flag["location"],
                "summary": f"Static embedded asset feature enabled: {asset_flag['feature']}",
                "evidence": asset_flag["impact"],
                "recommendation": asset_flag["remedy"],
            }
        )
        finding_id += 1

    # 3. Linker & Compile-Time Configuration
    config_path = os.path.join(repo_root, ".cargo", "config.toml")
    config_content = analyze_bloat.read_file_safe(config_path)

    has_fast_linker = bool(
        re.search(
            r"linker\s*=\s*\"(lld-link|lld|mold)", config_content, re.IGNORECASE
        )
        or re.search(r"-fuse-ld=(lld|mold)", config_content)
    )

    if not has_fast_linker:
        findings.append(
            {
                "id": f"W8-{finding_id:02d}",
                "severity": "MEDIUM",
                "location": ".cargo/config.toml",
                "summary": "Default slow system linker in use (lld-link / mold not configured)",
                "evidence": "No custom linker specified in target configuration",
                "recommendation": "Configure `linker = 'lld-link.exe'` for MSVC or `mold` for Linux to accelerate link stage",
            }
        )
        finding_id += 1

    return {
        "package": package,
        "total_crates": deps_data["total_unique_crates"],
        "direct_deps": deps_data["direct_dependency_count"],
        "duplicates_count": len(deps_data["duplicates"]),
        "findings": findings,
    }


def main():
    parser = argparse.ArgumentParser(description="Run W8 Performance Audit")
    parser.add_argument(
        "--package", "-p", default="fastmd", help="Package to audit"
    )
    parser.add_argument("--json", action="store_true", help="Output as JSON")
    args = parser.parse_args()

    data = run_w8_audit(package=args.package)

    if args.json:
        print(json.dumps(data, indent=2))
        return

    print(f"## Code Quality Report - perf - {data['package']}\n")
    print("### Findings")
    print("| ID | Severity | Location | Summary | Evidence | Recommendation |")
    print("|---|---|---|---|---|---|")
    for f in data["findings"]:
        print(
            f"| {f['id']} | {f['severity']} | {f['location']} | {f['summary']} | {f['evidence']} | {f['recommendation']} |"
        )

    print("\n### Metrics")
    print(f"- Total unique non-dev crates: {data['total_crates']}")
    print(f"- Direct dependencies: {data['direct_deps']}")
    print(f"- Duplicate crate groups: {data['duplicates_count']}")
    severities = {"CRITICAL": 0, "HIGH": 0, "MEDIUM": 0, "LOW": 0}
    for f in data["findings"]:
        sev = f["severity"].upper()
        if sev in severities:
            severities[sev] += 1
    print(
        f"- Findings by severity: CRITICAL {severities['CRITICAL']}, HIGH {severities['HIGH']}, MEDIUM {severities['MEDIUM']}, LOW {severities['LOW']}"
    )


if __name__ == "__main__":
    main()
