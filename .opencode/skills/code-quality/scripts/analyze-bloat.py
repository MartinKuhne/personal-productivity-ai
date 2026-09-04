#!/usr/bin/env python3
"""
analyze-bloat.py — Binary size and profile audit script for Rust workspaces.

Inspects release profile configurations, executable sizes, and .rlib
intermediate archive sizes to detect binary bloat.
"""

import argparse
import glob
import json
import os
import re
import sys


def read_file_safe(path):
    if os.path.exists(path):
        with open(path, "r", encoding="utf-8", errors="replace") as f:
            return f.read()
    return ""


def audit_profile_settings(repo_root):
    """Audit Cargo.toml and .cargo/config.toml for size-optimization flags."""
    cargo_toml = read_file_safe(os.path.join(repo_root, "Cargo.toml"))
    config_toml = read_file_safe(os.path.join(repo_root, ".cargo", "config.toml"))

    combined = cargo_toml + "\n" + config_toml

    settings = {
        "strip": False,
        "lto": None,
        "codegen-units": None,
        "opt-level": None,
        "panic": None,
    }

    # Match strip
    m = re.search(r"^\s*strip\s*=\s*(true|false|\"[^\"]+\")", combined, re.M)
    if m:
        settings["strip"] = m.group(1) == "true" or m.group(1).strip('"') in (
            "symbols",
            "debuginfo",
        )

    # Match lto
    m = re.search(r"^\s*lto\s*=\s*(true|false|\"[^\"]+\")", combined, re.M)
    if m:
        settings["lto"] = m.group(1).strip('"')

    # Match codegen-units
    m = re.search(r"^\s*codegen-units\s*=\s*(\d+)", combined, re.M)
    if m:
        settings["codegen-units"] = int(m.group(1))

    # Match opt-level
    m = re.search(r"^\s*opt-level\s*=\s*(\d+|\"[^\"]+\")", combined, re.M)
    if m:
        settings["opt-level"] = m.group(1).strip('"')

    # Match panic
    m = re.search(r"^\s*panic\s*=\s*\"([^\"]+)\"", combined, re.M)
    if m:
        settings["panic"] = m.group(1)

    return settings


def inspect_binaries(repo_root):
    """Check sizes of release executables."""
    results = []
    release_dir = os.path.join(repo_root, "target", "release")
    if not os.path.exists(release_dir):
        return results

    for file in os.listdir(release_dir):
        if file.endswith(".exe") or (
            os.access(os.path.join(release_dir, file), os.X_OK)
            and "." not in file
        ):
            full_path = os.path.join(release_dir, file)
            if os.path.isfile(full_path):
                size = os.path.getsize(full_path)
                results.append(
                    {
                        "name": file,
                        "size_bytes": size,
                        "size_mb": round(size / (1024 * 1024), 2),
                    }
                )
    results.sort(key=lambda x: x["size_bytes"], reverse=True)
    return results


def inspect_rlibs(repo_root, top_n=20):
    """Analyze top .rlib files by size in target/release/deps."""
    deps_dir = os.path.join(repo_root, "target", "release", "deps")
    results = []
    if not os.path.exists(deps_dir):
        return results

    for rlib_path in glob.glob(os.path.join(deps_dir, "*.rlib")):
        filename = os.path.basename(rlib_path)
        size = os.path.getsize(rlib_path)
        # Clean crate name from libfoo-hash.rlib
        m = re.match(r"^lib([a-zA-Z0-9_]+)-[0-9a-f]+\.rlib$", filename)
        crate_name = m.group(1) if m else filename
        results.append(
            {
                "crate": crate_name,
                "file": filename,
                "size_bytes": size,
                "size_mb": round(size / (1024 * 1024), 2),
            }
        )

    results.sort(key=lambda x: x["size_bytes"], reverse=True)
    return results[:top_n]


def check_embedded_assets(repo_root):
    """Check for known static embedded assets (e.g. embedded fonts in Typst)."""
    flags = []
    md2pdf_cargo = os.path.join(repo_root, "src", "md2pdf", "Cargo.toml")
    content = read_file_safe(md2pdf_cargo)
    if "typst-kit-embed-fonts" in content:
        flags.append(
            {
                "location": "src/md2pdf/Cargo.toml",
                "feature": "typst-kit-embed-fonts",
                "impact": "~8.4 MB of static fonts bundled directly into executable",
                "remedy": "Switch to system font resolution via `typst-kit-fonts`",
            }
        )
    return flags


def analyze(repo_root):
    return {
        "profile_settings": audit_profile_settings(repo_root),
        "binaries": inspect_binaries(repo_root),
        "top_rlibs": inspect_rlibs(repo_root),
        "embedded_asset_flags": check_embedded_assets(repo_root),
    }


def main():
    parser = argparse.ArgumentParser(description="Analyze Rust binary size & bloat")
    parser.add_argument("--json", action="store_true", help="Output as JSON")
    args = parser.parse_args()

    repo_root = os.path.abspath(
        os.path.join(os.path.dirname(__file__), "..", "..", "..", "..")
    )
    data = analyze(repo_root)

    if args.json:
        print(json.dumps(data, indent=2))
        return

    print("=== Release Profile Optimization Audit ===")
    p = data["profile_settings"]
    print(f"  strip = {p['strip']} (Recommended: true)")
    print(f"  lto = {p['lto']} (Recommended: 'thin' or 'fat')")
    print(f"  codegen-units = {p['codegen-units']} (Recommended: 1)")
    print(f"  opt-level = {p['opt-level']} (Recommended: 'z' or 3)")
    print(f"  panic = {p['panic']} (Recommended: 'abort')")

    if data["binaries"]:
        print("\n=== Release Binaries ===")
        for b in data["binaries"]:
            print(f"  {b['name']:<25} : {b['size_mb']:>6.2f} MB ({b['size_bytes']:,} bytes)")

    if data["embedded_asset_flags"]:
        print("\n=== Embedded Static Asset Flags ===")
        for f in data["embedded_asset_flags"]:
            print(f"  Location: {f['location']}")
            print(f"  Feature:  {f['feature']}")
            print(f"  Impact:   {f['impact']}")
            print(f"  Remedy:   {f['remedy']}\n")

    if data["top_rlibs"]:
        print("=== Top 15 Largest Compiled Libraries (.rlib) ===")
        print(f"{'Crate Name':<30} | {'Archive Size (MB)':<18} | {'Filename'}")
        print("-" * 75)
        for r in data["top_rlibs"][:15]:
            print(f"{r['crate']:<30} | {r['size_mb']:>14.2f} MB | {r['file']}")


if __name__ == "__main__":
    main()
