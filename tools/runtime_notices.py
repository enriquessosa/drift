#!/usr/bin/env python3
"""Copy source notices from exact locked Nix runtime source archives.
Realize these source paths through locked nixpkgs before running. This prepares
notices; it does not discharge corresponding-source obligations for exports.
"""
import hashlib
import json
import pathlib
import tarfile

root = pathlib.Path(__file__).resolve().parent.parent
sources = {
    "rust-1.99.0": "/nix/store/v0whnr5j6mm22yhz4as1xdlrw5mrk56f-rustc-1.99.0-src.tar.gz",
    "glibc-2.44": "/nix/store/jq00nlpdcxgcj5xwx25jibn61a611234-glibc-2.44.tar.xz",
    "gcc-16.2.0": "/nix/store/chw5kxp89wq3wkc37f6fr8wiw4yc7paj-gcc-16.2.0.tar.xz",
}
records = []
for component, archive in sources.items():
    path = pathlib.Path(archive)
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    files = []
    with tarfile.open(path, "r|*") as source:
        for member in source:
            parts = pathlib.PurePosixPath(member.name).parts
            relative = pathlib.PurePosixPath(*parts[1:])
            name = relative.name.upper()
            notice = name.startswith(("LICENSE", "COPYRIGHT", "COPYING", "NOTICE"))
            selected = notice and (len(parts) == 2 or (component.startswith("rust") and "library" in relative.parts))
            if not member.isfile() or not selected:
                continue
            assert not relative.is_absolute() and ".." not in relative.parts
            stream = source.extractfile(member)
            assert stream is not None
            raw = stream.read()
            normalized = ("\n".join(line.rstrip() for line in raw.decode("utf-8").splitlines()).rstrip() + "\n").encode()
            destination = root / "licenses" / "runtime" / component / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(normalized)
            files.append({"source_path": str(relative), "path": str(destination.relative_to(root)), "upstream_sha256": hashlib.sha256(raw).hexdigest(), "sha256": hashlib.sha256(normalized).hexdigest()})
    records.append({"component": component, "nix_source": archive, "archive_sha256": digest.hexdigest(), "files": sorted(files, key=lambda f: f["path"])})
(root / "licenses" / "runtime-inventory.json").write_text(json.dumps(records, indent=2, sort_keys=True) + "\n")
print("Prepared exact-source runtime notices:", {r["component"]: len(r["files"]) for r in records})
