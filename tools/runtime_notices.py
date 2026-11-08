#!/usr/bin/env python3
"""Copy source notices from exact locked Nix runtime source archives.
Realize these source paths through locked nixpkgs before running. This prepares
notices; it does not discharge corresponding-source obligations for exports.
"""
import hashlib
import json
import pathlib
import shutil
import tarfile
import tempfile
import tomllib
import urllib.request

root = pathlib.Path(__file__).resolve().parent.parent
sources = {
    "rust-1.99.0": ("/nix/store/v0whnr5j6mm22yhz4as1xdlrw5mrk56f-rustc-1.99.0-src.tar.gz", "2035e4077b834a42ff8afd07f277ae3f06340098b86b1d2843aa234b4cfcae67"),
    "glibc-2.44": ("/nix/store/jq00nlpdcxgcj5xwx25jibn61a611234-glibc-2.44.tar.xz", "37f600f2bef3c5e8300147059568b2a2e40a7ad6ccc65ce942556d49429cc667"),
    "gcc-16.2.0": ("/nix/store/chw5kxp89wq3wkc37f6fr8wiw4yc7paj-gcc-16.2.0.tar.xz", "e6738e29597f733270731aa90600f37ffdc045079dfc27ec7e8192cc81085c3e"),
    "libidn2-2.3.8": ("/nix/store/9dcs318zy7jbxz8kfdjnxd1c6s1xd2vy-libidn2-2.3.8.tar.gz", "f557911bf6171621e1f72ff35f5b1825bb35b52ed45325dcdee931e5d3c0787a"),
    "libunistring-1.4.2": ("/nix/store/l6b9nvw7fhknbn8f4g7zgglg5y2cf5ki-libunistring-1.4.2.tar.gz", "e82664b170064e62331962126b259d452d53b227bb4a93ab20040d846fec01d8"),
}
records = []
for component, (archive, expected_archive_sha256) in sources.items():
    path = pathlib.Path(archive)
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    archive_sha256 = digest.hexdigest()
    if archive_sha256 != expected_archive_sha256:
        raise ValueError(f"{archive} hash mismatch: {archive_sha256}")
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
    records.append({"component": component, "nix_source": archive, "archive_sha256": archive_sha256, "files": sorted(files, key=lambda f: f["path"])})

# The Nix rustc package omits its generated binary documentation. Obtain the
# matching official upstream release artifact, verify its checksum and exact
# version in the published Rust 1.99.0 channel manifest, then retain the file
# byte-for-byte.
rust_manifest_url = "https://static.rust-lang.org/dist/channel-rust-1.99.0.toml"
rust_manifest_sha256 = "ce6dddc886364f8d786514771212cebe9b731ba82d6b859951c6b0ccc516b6a2"
rust_archive_url = "https://static.rust-lang.org/dist/2026-10-01/rustc-1.99.0-x86_64-unknown-linux-gnu.tar.xz"
rust_archive_sha256 = "77171ba2a0345fdf2abc4fedda55d6de078dae7a68527c28be8c77dcc9604bd5"
rust_doc_path = "rustc-1.99.0-x86_64-unknown-linux-gnu/rustc/share/doc/rust/COPYRIGHT-library.html"
with urllib.request.urlopen(rust_manifest_url, timeout=60) as response:
    manifest_bytes = response.read()
assert hashlib.sha256(manifest_bytes).hexdigest() == rust_manifest_sha256
manifest = tomllib.loads(manifest_bytes.decode("utf-8"))
release_package = manifest["pkg"]["rustc"]["target"]["x86_64-unknown-linux-gnu"]
assert release_package["xz_url"] == rust_archive_url
assert release_package["xz_hash"] == rust_archive_sha256
with tempfile.TemporaryDirectory(prefix="drift-rust-notices-") as temp_dir:
    archive_path = pathlib.Path(temp_dir) / "rustc.tar.xz"
    with (
        urllib.request.urlopen(rust_archive_url, timeout=60) as response,
        archive_path.open("wb") as output,
    ):
        shutil.copyfileobj(response, output)
    digest = hashlib.sha256(archive_path.read_bytes()).hexdigest()
    assert digest == rust_archive_sha256, f"unexpected Rust release archive hash: {digest}"
    with tarfile.open(archive_path, "r:xz") as source:
        member = source.getmember(rust_doc_path)
        assert member.isfile()
        stream = source.extractfile(member)
        assert stream is not None
        raw = stream.read()
rust_doc = root / "licenses/runtime/rust-1.99.0/COPYRIGHT-library.html"
rust_doc.parent.mkdir(parents=True, exist_ok=True)
rust_doc.write_bytes(raw)
records.append({
    "component": "rust-1.99.0-binary-library-attribution",
    "compiler": "rustc 1.99.0 (b940084d7eb6a299eb4bfeb8e34901bc051e7ac4)",
    "channel_manifest_url": rust_manifest_url,
    "channel_manifest_sha256": rust_manifest_sha256,
    "source_url": rust_archive_url,
    "source_archive_sha256": rust_archive_sha256,
    "source_path": rust_doc_path,
    "content_sha256": hashlib.sha256(raw).hexdigest(),
    "path": str(rust_doc.relative_to(root)),
})
(root / "licenses" / "runtime-inventory.json").write_text(json.dumps(records, indent=2, sort_keys=True) + "\n")
print("Prepared exact-source runtime notices:", {r["component"]: len(r.get("files", [r])) for r in records})
