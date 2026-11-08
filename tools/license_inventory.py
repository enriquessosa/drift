#!/usr/bin/env python3
"""Prepare Linux Cargo metadata inventory and upstream source notices.
Run after cargo fetch --locked. Review the generated changes before release.
"""
import hashlib
import json
import pathlib
import re
import subprocess

root = pathlib.Path(__file__).resolve().parent.parent
metadata = json.loads(subprocess.check_output([
    "cargo", "metadata", "--offline", "--locked", "--filter-platform",
    "x86_64-unknown-linux-gnu", "--format-version", "1",
], cwd=root))
packages = {p["id"]: p for p in metadata["packages"]}
project = metadata["resolve"]["root"]
nodes = {n["id"]: n for n in metadata["resolve"]["nodes"]}
direct = {d["pkg"] for d in nodes[project]["deps"]}
roles = {}

def visit(package, role, visited):
    if (package, role) in visited:
        return
    visited.add((package, role))
    if package != project:
        roles.setdefault(package, set()).add(role)
    for dependency in nodes[package]["deps"]:
        for kind in dependency["dep_kinds"]:
            edge = kind["kind"] or "normal"
            if package != project and edge == "dev":
                continue
            child_role = "dev" if role == "dev" or edge == "dev" else "build" if role == "build" or edge == "build" else "normal"
            visit(dependency["pkg"], child_role, visited)

visit(project, "normal", set())
output = root / "licenses" / "cargo"
output.mkdir(parents=True, exist_ok=True)
records = []
for identity, package in sorted(packages.items(), key=lambda item: (item[1]["name"], item[1]["version"])):
    if identity == project:
        continue
    source = pathlib.Path(package["manifest_path"]).parent
    destination = output / f'{package["name"]}-{package["version"]}'
    destination.mkdir(exist_ok=True)
    notices = []
    for original in sorted(source.iterdir()):
        if original.is_file() and original.name.upper().startswith(("LICENSE", "COPYING", "COPYRIGHT", "NOTICE")):
            raw = original.read_bytes()
            normalized = ("\n".join(line.rstrip() for line in raw.decode("utf-8").splitlines()).rstrip() + "\n").encode()
            (destination / original.name).write_bytes(normalized)
            notices.append({"path": str((destination / original.name).relative_to(root)), "upstream_sha256": hashlib.sha256(raw).hexdigest(), "sha256": hashlib.sha256(normalized).hexdigest()})
    if package["name"] == "libsqlite3-sys":
        # The bundled SQLite amalgamation carries its public-domain dedication
        # in the source header, rather than a separate license file.
        header = source / "sqlite3" / "sqlite3.c"
        content = header.read_text()
        introduction = content.split("*/", 1)[0] + "*/\n"
        disclaimer = next(block for block in re.findall(r"/\*.*?\*/", content, re.S) if "The author disclaims copyright" in block)
        text = introduction + "\n" + disclaimer + "\n"
        (destination / "SQLITE-HEADER.txt").write_text(text)
        notices.append({"path": str((destination / "SQLITE-HEADER.txt").relative_to(root)), "sha256": hashlib.sha256(text.encode()).hexdigest()})
    records.append({"name": package["name"], "version": package["version"], "license": package.get("license"), "source": package["source"], "direct": identity in direct, "roles": sorted(roles.get(identity, set())), "proc_macro": any("proc-macro" in t["kind"] for t in package["targets"]), "notices": notices})
(root / "licenses" / "inventory.json").write_text(json.dumps({"target": "x86_64-unknown-linux-gnu", "packages": records}, indent=2, sort_keys=True) + "\n")
print(f"Inventoried {len(records)} Cargo packages; copied upstream source notice files.")
