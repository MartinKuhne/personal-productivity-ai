#!/usr/bin/env python3
"""
analyze-deps.py — Automated dependency analysis tool for Rust workspaces.

Audits dependency counts, transitive subtrees, exclusive crates per subsystem,
and duplicate crate versions.
"""

import argparse
import json
import os
import re
import subprocess
import sys


def run_command(cmd, cwd=None):
    """Run a shell command and return stdout as string."""
    try:
        proc = subprocess.run(
            cmd,
            shell=True,
            capture_output=True,
            encoding="utf-8",
            errors="replace",
            cwd=cwd,
        )
        return proc.stdout, proc.stderr, proc.returncode
    except Exception as e:
        return "", str(e), 1


def get_crate_list(package="fastmd", flags="--edges no-dev", cwd=None):
    """Get the set of unique crates in the dependency tree."""
    cmd = f"cargo tree -p {package} {flags} --prefix none"
    stdout, _, code = run_command(cmd, cwd=cwd)
    if code != 0:
        return set()
    crates = set()
    for line in stdout.splitlines():
        m = re.match(r"^\s*([a-zA-Z0-9_-]+)\s+v([0-9.]+)", line.strip())
        if m:
            crates.add(m.group(1))
    return crates


def get_direct_dependencies(package="fastmd", cwd=None):
    """Get direct dependencies for a package."""
    cmd = f"cargo tree -p {package} --depth 1 --edges no-dev --prefix none"
    stdout, _, code = run_command(cmd, cwd=cwd)
    if code != 0:
        return set()
    deps = set()
    lines = stdout.splitlines()
    for line in lines[1:]:
        m = re.match(r"^\s*([a-zA-Z0-9_-]+)\s+v([0-9.]+)", line.strip())
        if m:
            deps.add(m.group(1))
    return deps


def find_duplicates(package="fastmd", cwd=None):
    """Find duplicate crate versions in the dependency tree."""
    cmd = f"cargo tree -p {package} --edges no-dev -d"
    stdout, _, _ = run_command(cmd, cwd=cwd)
    duplicates = {}
    current_crate = None
    for line in stdout.splitlines():
        m = re.match(r"^([a-zA-Z0-9_-]+)\s+v([0-9.]+)", line.strip())
        if m:
            current_crate = m.group(1)
            version = m.group(2)
            if current_crate not in duplicates:
                duplicates[current_crate] = set()
            duplicates[current_crate].add(version)
    return {k: sorted(list(v)) for k, v in duplicates.items() if len(v) > 1}


def analyze(package="fastmd", cwd=None):
    """Perform complete dependency analysis."""
    total_crates = get_crate_list(package=package, cwd=cwd)
    direct_deps = get_direct_dependencies(package=package, cwd=cwd)

    dep_trees = {}
    for dep in sorted(direct_deps):
        dep_trees[dep] = get_crate_list(package=dep, cwd=cwd)

    # Compute exclusive crates
    exclusive_map = {}
    for dep in direct_deps:
        other_crates = set()
        for other, tree in dep_trees.items():
            if other != dep:
                other_crates.update(tree)
        exclusive = dep_trees[dep] - other_crates
        exclusive_map[dep] = exclusive

    duplicates = find_duplicates(package=package, cwd=cwd)

    return {
        "package": package,
        "total_unique_crates": len(total_crates),
        "direct_dependency_count": len(direct_deps),
        "dependencies": [
            {
                "name": dep,
                "transitive_count": len(dep_trees[dep]),
                "exclusive_count": len(exclusive_map[dep]),
                "exclusive_sample": sorted(list(exclusive_map[dep]))[:10],
            }
            for dep in sorted(
                direct_deps, key=lambda d: len(exclusive_map[d]), reverse=True
            )
        ],
        "duplicates": duplicates,
    }


def main():
    parser = argparse.ArgumentParser(description="Analyze Rust dependencies")
    parser.add_argument(
        "--package", "-p", default="fastmd", help="Root package to analyze"
    )
    parser.add_argument(
        "--json", action="store_true", help="Output results as JSON"
    )
    args = parser.parse_args()

    repo_root = os.path.abspath(
        os.path.join(os.path.dirname(__file__), "..", "..", "..", "..")
    )
    data = analyze(package=args.package, cwd=repo_root)

    if args.json:
        print(json.dumps(data, indent=2))
        return

    print(f"=== Dependency Analysis for '{data['package']}' ===")
    print(f"Total Unique Crates (non-dev): {data['total_unique_crates']}")
    print(f"Direct Dependencies:          {data['direct_dependency_count']}\n")

    print(
        f"{'Direct Dependency':<25} | {'Transitive':<10} | {'Exclusive':<10} | {'Sample Exclusive Crates'}"
    )
    print("-" * 80)
    for d in data["dependencies"]:
        if d["exclusive_count"] > 0 or d["transitive_count"] >= 20:
            sample = ", ".join(d["exclusive_sample"][:5])
            if d["exclusive_count"] > 5:
                sample += "..."
            print(
                f"{d['name']:<25} | {d['transitive_count']:<10} | {d['exclusive_count']:<10} | {sample}"
            )

    if data["duplicates"]:
        print(
            f"\n=== Duplicate Crate Versions Detected ({len(data['duplicates'])}) ==="
        )
        for crate, versions in sorted(data["duplicates"].items()):
            print(f"  - {crate}: {', '.join(versions)}")


if __name__ == "__main__":
    main()
