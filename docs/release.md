# Installation and operation

Build the locked package with `nix build path:.`, then run `./result/bin/drift --help`.
Use `nix develop path:.` for Cargo development. Nix vendors dependencies from
Cargo.lock through fixed-output fetches; first-time dependency realization may
need network access. Compilation uses the vendored source. Both Linux architectures
in the flake are declared targets; only the current host is verified.

The initial candidate is 0.1.0. CI assumes Rust 1.95.0 on Ubuntu 24.04 and uses
checkout v6 pinned to a commit. Nix uses the Rust toolchain from locked nixpkgs. CI is prepared but
has not run on GitHub. Before publishing, review licensing, release artifacts,
architecture coverage, and the supported NixOS layout on target hosts.

# CLI and exit codes

Run `drift --help` or `<command> --help` for argument details.

| Command | Purpose |
| --- | --- |
| `scan [--json] [--findings] [--no-declared]` | Read live observations and optional declared audit |
| `snapshot create` | Save local observations |
| `snapshot list` | List stored snapshot IDs |
| `diff --latest` or `diff --from ID --to ID` | Compare historical observations |
| `diff ... --findings [--json]` | Add findings to a historical comparison |

Global `--config PATH` selects an explicit TOML file. Exit 0 means the command
completed, including partial evidence or findings; it does not mean secure or
healthy. Exit 1 means an operational/configuration/storage error. Exit 2 means
invalid CLI arguments. Help and version exit 0. Errors go to stderr; successful
JSON goes only to stdout. Live timestamps and runtime observations naturally
vary; ordering of equivalent normalized evidence is deterministic.

Scan JSON retains `observation` and `declared`, adding `findings` only when
requested. Historical findings JSON contains `from_snapshot_id`,
`to_snapshot_id`, `comparison`, and `findings`. For example the report includes
`{"schema_version":1,"findings":[],"counts":{"total":0,"by_category":{},"by_severity":{}}}`
plus comparison counts. Observation, database, declared-audit and findings
versions remain 1.

# Safe demonstrations

Run `cargo run --offline --locked --example demo` for synthetic new-listener,
permission-broadening and failed-service findings. Add `-- --incomplete` for
indeterminate evidence and zero findings. The example performs no collection,
file inspection, database writes or service operations.

# Troubleshooting

For a configuration error, check TOML syntax, unknown keys and absolute paths.
For database errors, use a private directory owned by your user; do not chmod
unrelated files or mark an arbitrary database as Drift-owned. Journal sidecars
require separate recovery of trusted state. Missing systemctl/system-manager
access yields incomplete evidence; TCP requires readable proc tables and known
namespace identity. Unsupported NixOS units and incompatible generations are
not confirmed mismatches. Review collection statuses even after exit 0.

Linux procfs and a valid local machine ID are required for collection. systemd
collection requires systemctl on PATH and access to the system manager. No root
is needed. State defaults to XDG state storage. Snapshot/report metadata can
include private paths, ownership and local endpoints; keep exports private.
Drift distinguishes historical change from current NixOS declaration agreement.
Neither evidence-based observations nor findings establish whether the machine
has been compromised. See README and findings-rules.md for configuration,
privacy boundaries and exact supported semantics.
