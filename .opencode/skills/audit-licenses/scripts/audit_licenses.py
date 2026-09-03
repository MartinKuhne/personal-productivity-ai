#!/usr/bin/env python3
"""Cargo workspace dependency license auditing script.

Audits all dependencies of a Rust Cargo workspace using `cargo metadata`,
identifying non-permissive (copyleft, weak-copyleft, restricted) licenses,
font licenses, and non-standard permissive licenses. Traces reverse
dependency paths to locate the exact parent crate or workspace member.
"""

from __future__ import annotations

import argparse
from collections import deque
import json
from pathlib import Path
import re
import subprocess
import sys
from typing import Any, Dict, List, Optional, Set, Tuple

# Default recognized permissive licenses
DEFAULT_PERMISSIVE = {
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "BSD-3-Clause-Clear",
    "0BSD",
    "ISC",
    "CC0-1.0",
    "Unlicense",
    "Zlib",
    "BSL-1.0",
    "Unicode-3.0",
    "Unicode-DFS-2016",
    "CDLA-Permissive-2.0",
}

COPYLEFT_PATTERNS = [
    (re.compile(r"AGPL", re.IGNORECASE), "AGPL (Strong / Network Copyleft)"),
    (re.compile(r"(?<!L)GPL", re.IGNORECASE), "GPL (Strong Copyleft)"),
    (re.compile(r"LGPL", re.IGNORECASE), "LGPL (Copyleft)"),
    (re.compile(r"MPL", re.IGNORECASE), "MPL (Weak Copyleft)"),
    (re.compile(r"SSPL", re.IGNORECASE), "SSPL (Server Side Copyleft)"),
    (re.compile(r"EUPL", re.IGNORECASE), "EUPL (European Union Public License)"),
    (re.compile(r"CDDL", re.IGNORECASE), "CDDL (Weak Copyleft)"),
    (re.compile(r"EPL", re.IGNORECASE), "EPL (Eclipse Public License)"),
]

FONT_PATTERNS = [
    (re.compile(r"OFL", re.IGNORECASE), "SIL Open Font License"),
    (re.compile(r"Ubuntu-font", re.IGNORECASE), "Ubuntu Font Licence"),
]


def run_cargo_metadata(workspace_dir: Path) -> dict:
    cmd = ["cargo", "metadata", "--format-version", "1"]
    result = subprocess.run(
        cmd,
        cwd=workspace_dir,
        capture_output=True,
        encoding="utf-8",
    )
    if result.returncode != 0:
        print(f"Error running cargo metadata: {result.stderr}", file=sys.stderr)
        sys.exit(result.returncode)
    return json.loads(result.stdout)


def evaluate_spdx(expression: str, allowed_licenses: Set[str]) -> Tuple[bool, str]:
    """Evaluates whether an SPDX expression satisfies the allowed licenses.
    
    Returns (is_acceptable, explanation).
    Handles 'OR' (satisfiable if at least one alternative is allowed)
    and 'AND' (all parts must be allowed).
    """
    if not expression:
        return False, "Missing license"

    clean_expr = expression.replace("/", " OR ")
    
    # Simple recursive token-based evaluator
    tokens = re.findall(r"\(|\)|\bOR\b|\bAND\b|[^\s()]+", clean_expr)
    
    def parse_expression(index: int) -> Tuple[bool, int]:
        sub_results = []
        operators = []
        
        while index < len(tokens):
            tok = tokens[index]
            if tok == "(":
                sub_res, index = parse_expression(index + 1)
                sub_results.append(sub_res)
            elif tok == ")":
                break
            elif tok == "OR":
                operators.append("OR")
            elif tok == "AND":
                operators.append("AND")
            else:
                base_tok = tok.split(" WITH ")[0].strip()
                sub_results.append(base_tok in allowed_licenses)
            index += 1
            
        if not sub_results:
            return False, index
            
        current = sub_results[0]
        for i, op in enumerate(operators):
            nxt = sub_results[i + 1] if i + 1 < len(sub_results) else False
            if op == "OR":
                current = current or nxt
            elif op == "AND":
                current = current and nxt
        return current, index

    is_permissive, _ = parse_expression(0)
    return is_permissive, clean_expr


def find_dependency_paths(
    target_id: str,
    workspace_members: Set[str],
    parent_map: Dict[str, List[Tuple[str, List[dict]]]],
    max_paths: int = 2,
) -> List[List[str]]:
    """Finds shortest dependency paths from any workspace member to target_id."""
    queue = deque([(target_id, [target_id])])
    visited = {target_id}
    paths = []
    
    while queue:
        curr, path = queue.popleft()
        if curr in workspace_members:
            paths.append(path)
            if len(paths) >= max_paths:
                break
            continue
        for parent_id, _ in parent_map.get(curr, []):
            if parent_id not in visited:
                visited.add(parent_id)
                queue.append((parent_id, [parent_id] + path))
    return paths


def format_path(
    path: List[str],
    packages: Dict[str, dict],
    resolve_nodes: Dict[str, dict],
) -> str:
    parts = []
    for i in range(len(path) - 1):
        p1 = packages[path[i]]
        p2 = packages[path[i + 1]]
        node = resolve_nodes.get(p1["id"], {})
        kinds_str = "normal"
        for d in node.get("deps", []):
            if d["pkg"] == p2["id"]:
                kinds = d.get("dep_kinds", [])
                if kinds:
                    kinds_str = ", ".join(
                        f"{k.get('kind') or 'normal'}"
                        + (f"({k.get('target')})" if k.get("target") else "")
                        for k in kinds
                    )
        parts.append(f"{p1['name']} v{p1['version']} --[{kinds_str}]-->")
    parts.append(f"{packages[path[-1]]['name']} v{packages[path[-1]]['version']}")
    return " ".join(parts)


def classify_license(license_str: str) -> str:
    if not license_str:
        return "Unknown / Unspecified"
    
    # Check for AGPL
    if "AGPL" in license_str:
        return "Strong Copyleft (AGPL)"
    
    # Check for LGPL
    if "LGPL" in license_str:
        if any(perm in license_str for perm in ["MIT", "Apache-2.0", "BSD"]):
            return "Multi-licensed (Includes Permissive Option)"
        return "Copyleft (LGPL)"
        
    # Check for GPL
    if "GPL" in license_str:
        if any(perm in license_str for perm in ["MIT", "Apache-2.0", "BSD"]):
            return "Multi-licensed (Includes Permissive Option)"
        return "Strong Copyleft (GPL)"
        
    # Check for MPL
    if "MPL" in license_str:
        if any(perm in license_str for perm in ["MIT", "Apache-2.0", "BSD"]):
            return "Multi-licensed (Includes Permissive Option)"
        return "Weak Copyleft (MPL)"
        
    # Check for Font
    if "OFL" in license_str or "Ubuntu-font" in license_str:
        return "Font / Asset License"
        
    # Check if standard permissive
    standard_tokens = {"MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "BSD-3-Clause-Clear"}
    tokens = re.findall(r"[A-Za-z0-9\.\-]+", license_str)
    if all(t in standard_tokens or t in {"AND", "OR", "WITH", "LLVM-exception"} for t in tokens):
        return "Standard Permissive (MIT / Apache / BSD)"
        
    return "Other Permissive"


def audit_workspace(
    workspace_dir: Path,
    allowed_licenses: Set[str],
    include_dev: bool = True,
) -> Dict[str, Any]:
    metadata = run_cargo_metadata(workspace_dir)
    packages = {p["id"]: p for p in metadata["packages"]}
    workspace_members = set(metadata["workspace_members"])
    resolve_nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}

    parent_map: Dict[str, List[Tuple[str, List[dict]]]] = {}
    for node_id, node in resolve_nodes.items():
        for dep in node.get("deps", []):
            dep_id = dep["pkg"]
            kinds = dep.get("dep_kinds", [])
            parent_map.setdefault(dep_id, []).append((node_id, kinds))

    audited_packages = []
    
    for p in metadata["packages"]:
        if p["id"] in workspace_members:
            continue
            
        lic = p.get("license") or ""
        paths = find_dependency_paths(p["id"], workspace_members, parent_map)
        formatted_paths = [format_path(pt, packages, resolve_nodes) for pt in paths]
        
        is_dev_only = bool(paths and all("dev" in fp for fp in formatted_paths))
        if not include_dev and is_dev_only:
            continue

        is_acceptable, clean_lic = evaluate_spdx(lic, allowed_licenses)
        classification = classify_license(lic)

        audited_packages.append({
            "name": p["name"],
            "version": p["version"],
            "license": lic,
            "classification": classification,
            "is_acceptable": is_acceptable,
            "is_dev_only": is_dev_only,
            "paths": formatted_paths,
            "repository": p.get("repository"),
            "description": p.get("description"),
        })

    return {
        "workspace_root": metadata["workspace_root"],
        "workspace_members": [
            {"name": packages[wm]["name"], "version": packages[wm]["version"]}
            for wm in workspace_members
        ],
        "total_packages": len(audited_packages),
        "packages": audited_packages,
    }


def generate_markdown_report(data: dict, non_permissive_only: bool = False) -> str:
    lines = []
    lines.append("# Cargo Dependency License Audit Report\n")
    lines.append(f"**Workspace Root:** `{data['workspace_root']}`  ")
    members_str = ", ".join(f"`{m['name']} v{m['version']}`" for m in data["workspace_members"])
    lines.append(f"**Workspace Members:** {members_str}  ")
    lines.append(f"**Total Audited Dependencies:** {data['total_packages']}\n")

    groups: Dict[str, List[dict]] = {}
    for p in data["packages"]:
        groups.setdefault(p["classification"], []).append(p)

    order = [
        "Strong Copyleft (AGPL)",
        "Strong Copyleft (GPL)",
        "Copyleft (LGPL)",
        "Weak Copyleft (MPL)",
        "Font / Asset License",
        "Multi-licensed (Includes Permissive Option)",
        "Other Permissive",
        "Standard Permissive (MIT / Apache / BSD)",
        "Unknown / Unspecified",
    ]

    for category in order:
        pkgs = groups.get(category, [])
        if not pkgs:
            continue
        if non_permissive_only and category in (
            "Standard Permissive (MIT / Apache / BSD)",
            "Other Permissive",
            "Multi-licensed (Includes Permissive Option)",
        ):
            continue

        icon = "🚨" if "Strong Copyleft" in category else "⚠️" if "Copyleft" in category else "ℹ️" if "Font" in category else "✅"
        lines.append(f"## {icon} {category} ({len(pkgs)} packages)\n")
        lines.append("| Package | Version | License | Scope | Introduced By / Path |")
        lines.append("| :--- | :--- | :--- | :--- | :--- |")

        for p in sorted(pkgs, key=lambda x: x["name"]):
            scope = "Dev Dependency" if p["is_dev_only"] else "Runtime"
            first_path = p["paths"][0] if p["paths"] else "Unknown"
            lines.append(f"| **`{p['name']}`** | `{p['version']}` | `{p['license']}` | {scope} | `{first_path}` |")
        lines.append("")

    return "\n".join(lines)


def main():
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="replace")
    if hasattr(sys.stderr, "reconfigure"):
        sys.stderr.reconfigure(encoding="utf-8", errors="replace")

    parser = argparse.ArgumentParser(description="Audit Cargo workspace dependency licenses.")
    parser.add_argument(
        "--workspace-dir",
        type=Path,
        default=Path.cwd(),
        help="Path to Cargo workspace root (defaults to current directory)",
    )
    parser.add_argument(
        "--format",
        choices=["markdown", "json", "summary"],
        default="markdown",
        help="Output format (default: markdown)",
    )
    parser.add_argument(
        "--non-permissive-only",
        action="store_true",
        help="Display only non-permissive (copyleft, font, unapproved) dependencies",
    )
    parser.add_argument(
        "--no-dev-deps",
        action="store_true",
        help="Exclude development dependencies (test/benchmarks)",
    )
    parser.add_argument(
        "--fail-on-copyleft",
        action="store_true",
        help="Exit with return code 1 if any non-permissive or copyleft license is found in runtime deps",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="Path to write report output",
    )

    args = parser.parse_args()

    allowed = set(DEFAULT_PERMISSIVE)
    data = audit_workspace(
        workspace_dir=args.workspace_dir,
        allowed_licenses=allowed,
        include_dev=not args.no_dev_deps,
    )

    if args.format == "json":
        output = json.dumps(data, indent=2)
    elif args.format == "summary":
        non_perm = [
            p for p in data["packages"]
            if p["classification"] in (
                "Strong Copyleft (AGPL)",
                "Strong Copyleft (GPL)",
                "Copyleft (LGPL)",
                "Weak Copyleft (MPL)",
                "Font / Asset License",
            )
        ]
        output = (
            f"Total dependencies: {data['total_packages']}\n"
            f"Non-permissive / flagged dependencies: {len(non_perm)}\n"
        )
        for p in non_perm:
            scope = "[DEV]" if p["is_dev_only"] else "[RUNTIME]"
            output += f" - {p['name']} v{p['version']} ({p['license']}) {scope}\n"
    else:
        output = generate_markdown_report(data, non_permissive_only=args.non_permissive_only)

    if args.output:
        args.output.write_text(output, encoding="utf-8")
        print(f"Report written to {args.output}")
    else:
        print(output)

    if args.fail_on_copyleft:
        copyleft_runtime = [
            p for p in data["packages"]
            if not p["is_dev_only"]
            and p["classification"] in (
                "Strong Copyleft (AGPL)",
                "Strong Copyleft (GPL)",
                "Copyleft (LGPL)",
                "Weak Copyleft (MPL)",
            )
        ]
        if copyleft_runtime:
            print(
                f"\n[CI FAILURE] Found {len(copyleft_runtime)} copyleft runtime dependencies:",
                file=sys.stderr,
            )
            for p in copyleft_runtime:
                print(f"  - {p['name']} v{p['version']} ({p['license']})", file=sys.stderr)
            sys.exit(1)


if __name__ == "__main__":
    main()
