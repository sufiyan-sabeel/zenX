#!/usr/bin/env python3
"""Verify crate metadata and the crates.io publication order.

Checks, for every workspace member:
  * the package name, version and shared metadata are what we expect;
  * every internal dependency carries both a path and a version, and that
    version matches the workspace version (a path-only dependency cannot be
    published);
  * the publication order in scripts/publish.sh is a valid topological order.

Exits non-zero on the first inconsistency. Run from the repository root.
"""

from __future__ import annotations

import pathlib
import re
import sys
import tomllib

ROOT = pathlib.Path(__file__).resolve().parent.parent
MODULES = [
    "core", "storage", "security", "trace", "skills", "doctor",
    "mobile", "bench", "contract", "docs", "selfupdate",
]
BIN_PACKAGE = "zenx"
BIN_DIR = "crates/zenx-cli"

problems: list[str] = []
notes: list[str] = []


def fail(message: str) -> None:
    problems.append(message)


def load(path: pathlib.Path) -> dict:
    return tomllib.loads(path.read_text())


workspace = load(ROOT / "Cargo.toml")
shared = workspace.get("workspace", {}).get("package", {})
ws_version = shared.get("version")
ws_license = shared.get("license")
ws_repository = shared.get("repository")
members = workspace["workspace"]["members"]
workspace_deps = workspace.get("workspace", {}).get("dependencies", {})

if not ws_version:
    fail("workspace.package.version is missing")
if not ws_license:
    fail("workspace.package.license is missing")

# ---------------------------------------------------------------- members
packages: dict[str, dict] = {}
for member in members:
    manifest_path = ROOT / member / "Cargo.toml"
    if not manifest_path.is_file():
        fail(f"{member}: listed as a workspace member but has no Cargo.toml")
        continue
    manifest = load(manifest_path)
    package = manifest.get("package", {})
    name = package.get("name")
    if not name:
        fail(f"{member}: package.name is missing")
        continue
    packages[name] = {"manifest": manifest, "dir": member}

    for field, expected in (
        ("version", ws_version),
        ("license", ws_license),
        ("repository", ws_repository),
    ):
        declared = package.get(field)
        if isinstance(declared, dict) and declared.get("workspace"):
            continue  # inherited; verified against the workspace value below
        if declared != expected:
            fail(f"{name}: package.{field} is {declared!r}, expected {expected!r}")
    if not package.get("description"):
        fail(f"{name}: package.description is missing (required by crates.io)")
    for field in ("edition", "rust-version"):
        if field not in package:
            fail(f"{name}: package.{field} is missing")

expected_names = {f"zenx-{module}" for module in MODULES} | {BIN_PACKAGE}
missing = expected_names - set(packages)
if missing:
    fail(f"missing packages: {sorted(missing)}")
unexpected = set(packages) - expected_names
if unexpected:
    fail(f"unexpected packages: {sorted(unexpected)}")

# ------------------------------------------------------- internal deps
graph: dict[str, set[str]] = {}
for name, info in packages.items():
    deps = dict(info["manifest"].get("dependencies", {}))
    deps.update(info["manifest"].get("dev-dependencies", {}))
    internal = set()
    for dep_name, spec in deps.items():
        if not dep_name.startswith("zenx"):
            continue
        internal.add(dep_name)
        if isinstance(spec, dict) and spec.get("workspace"):
            spec = workspace_deps.get(dep_name, {})
        if not isinstance(spec, dict):
            fail(f"{name}: internal dependency {dep_name} is not a table")
            continue
        if "path" not in spec:
            fail(f"{name}: internal dependency {dep_name} has no path")
        if "version" not in spec:
            fail(
                f"{name}: internal dependency {dep_name} has no version; "
                "a path-only dependency cannot be published"
            )
        elif spec["version"] != ws_version:
            fail(
                f"{name}: internal dependency {dep_name} is pinned to "
                f"{spec['version']}, expected {ws_version}"
            )
        if dep_name not in packages:
            fail(f"{name}: depends on {dep_name}, which is not a workspace member")
    graph[name] = internal

# workspace.dependencies must mirror the same rules
for dep_name, spec in workspace_deps.items():
    if not dep_name.startswith("zenx"):
        continue
    if not isinstance(spec, dict) or "path" not in spec or "version" not in spec:
        fail(f"workspace.dependencies.{dep_name} needs both path and version")

# --------------------------------------------------------------- binary
bin_info = packages.get(BIN_PACKAGE)
if bin_info:
    manifest = bin_info["manifest"]
    if bin_info["dir"] != BIN_DIR:
        fail(f"the {BIN_PACKAGE} package lives in {bin_info['dir']}, expected {BIN_DIR}")
    bins = manifest.get("bin", [])
    names = [entry.get("name") for entry in bins]
    if names != [BIN_PACKAGE]:
        fail(f"{BIN_PACKAGE}: [[bin]] names are {names}, expected ['{BIN_PACKAGE}']")
    for field in ("keywords", "categories", "documentation"):
        if field not in manifest.get("package", {}):
            fail(f"{BIN_PACKAGE}: package.{field} is missing")

# ------------------------------------------------------- publish order
publish_script = (ROOT / "scripts" / "publish.sh").read_text()
match = re.search(r"ORDER=\((.*?)\)", publish_script, re.S)
if not match:
    fail("scripts/publish.sh has no ORDER array")
else:
    order = [line.strip() for line in match.group(1).splitlines() if line.strip()]
    if sorted(order) != sorted(packages):
        fail(
            "publish order does not list every package exactly once: "
            f"missing {sorted(set(packages) - set(order))}, "
            f"unknown {sorted(set(order) - set(packages))}"
        )
    seen: set[str] = set()
    for position, package in enumerate(order, start=1):
        unmet = graph.get(package, set()) - seen
        if unmet:
            fail(
                f"publish order is wrong: {package} is #{position} but its "
                f"dependencies {sorted(unmet)} are not published before it"
            )
        seen.add(package)
    notes.append(f"publish order verified across {len(order)} packages")

for note in notes:
    print(f"  ok  {note}")

if problems:
    print()
    for problem in problems:
        print(f"  FAIL  {problem}")
    print(f"\n{len(problems)} problem(s) found")
    sys.exit(1)

print(f"metadata verified for {len(packages)} packages at version {ws_version}")
