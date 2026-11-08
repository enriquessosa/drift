# Drift

Drift is an unprivileged, read-only Linux CLI for collecting observations and
comparing saved historical snapshots. Milestone 1 implements historical drift
only. It does not evaluate Nix, assign risk severity, remediate findings, run a
daemon, or scan other hosts.

## NixOS setup and verification

From this directory, on a host with registry/network and Nix daemon access:

```sh
nix develop path:.
cargo fmt --check
cargo clippy --offline --locked --all-targets -- -D warnings
cargo test --offline --locked
cargo build --offline --locked
cargo run -- --help
cargo run -- --config drift.example.toml scan
cargo run -- --config drift.example.toml snapshot create
cargo run -- --config drift.example.toml snapshot create
cargo run -- snapshot list
cargo run -- diff --latest
cargo run -- diff --from 1 --to 2
```

The flake supports x86_64-linux and aarch64-linux. `path:.` includes new source
files before they are tracked. For Git-based evaluation (`nix develop .`), add
Cargo.toml, Cargo.lock, flake.nix, flake.lock, and source files to Git first.
No commit is necessary. Cargo.lock and flake.lock are present and should be
retained. On a fresh host, run `cargo fetch --locked` with network access before
the offline checks above; dependency installation does not require root.
Generated binaries and SQLite files are ignored. Default runtime state is outside
the repository, so it does not interfere with flake evaluation.

For an isolated end-to-end smoke test without changing services:

```sh
state_dir=$(mktemp -d)
XDG_STATE_HOME="$state_dir" cargo run -- snapshot create
XDG_STATE_HOME="$state_dir" cargo run -- snapshot create
XDG_STATE_HOME="$state_dir" cargo run -- snapshot list
XDG_STATE_HOME="$state_dir" cargo run -- diff --latest
```

This leaves two disposable snapshots in the printed/assigned temporary directory.
A diff can legitimately contain service or listener changes between scans.

## Commands and configuration

- `drift scan`: collect and print collector status/counts without opening a DB.
- `drift snapshot create`: collect, atomically save, and print the snapshot ID.
- `drift snapshot list`: newest IDs first, with UTC Unix timestamps in seconds.
- `drift diff --latest`: compare the two newest inserted snapshots; error if fewer
  than two exist.
- `drift diff --from ID --to ID`: compare any two stored snapshots.
- `drift --help`: command usage; each subcommand also accepts `--help`.

Pass `--config /absolute/or/relative/config.toml` explicitly. No configuration
file is loaded implicitly. See `drift.example.toml`. All collectors default to
enabled; the default monitored-path list is empty. Set `systemd`, `network`, or
`files` to false to disable it. Disabled evidence remains visible. Unknown keys,
relative monitored paths, parent traversal (`..`), and relative database paths
are errors. Monitored paths are lexically normalized and deduplicated without
resolving symlinks.

The optional `database` is an absolute SQLite filename. Otherwise the default is
`$XDG_STATE_HOME/drift/snapshots.sqlite3` when XDG_STATE_HOME is absolute, falling
back to `$HOME/.local/state/drift/snapshots.sqlite3`. If neither base is usable,
Drift reports an error. Newly created directories use 0700 and the database uses
0600 (subject to a more restrictive umask). An existing immediate database parent
must be owned by the current user and private (no group or other permissions).
Existing database permissions are never changed, including on failure. Existing
Drift databases with group/other permissions are rejected. Symlink ancestors,
database symlinks, hardlinks, special files, and non-sticky ancestors writable by
other users are rejected. Use a dedicated private directory for overrides. Old
snapshots are never removed automatically.

Database identity uses SQLite application_id `0x44524654` (DRFT), schema version
1, an exact expected schema, and a quick integrity check. An existing file is
opened by SQLite for immutable, read-only validation before Drift permits a
read/write connection. Validation necessarily reads the file through SQLite;
filesystem access-time updates may occur. It does not chmod, migrate, recover
journals, create SQLite sidecars, or write database contents. Existing journal,
WAL, or SHM sidecars are rejected before opening for writes, including sidecars
beside a new database name. Recover a trusted interrupted database separately;
Drift does not automatically recover it. New files are exclusively created and
initialized transactionally. A failed new initialization can leave a private
empty file which will be rejected on the next attempt.

The database from the earlier Milestone 1 implementation lacks the identity
marker. It is preserved but rejected as unmarked, even if its schema resembles
Drift. No automatic adoption or migration is implemented: take a backup and
verify its provenance before planning an explicitly authorized offline migration.
Do not set the marker on an arbitrary database to bypass validation.

The trust boundary is the current user and the owners of the state directory's
ancestors (including a sticky temporary directory's owner). Held file/directory
descriptors, no-follow file opens, exclusive creation, and inode checks reduce
path replacement risks. SQLite resolves its own database pathname and manages
locking/journals after validation; these checks are not isolation from a hostile
process running as the same user, a hostile ancestor owner, or a filesystem that
changes underneath SQLite. Use a local filesystem and a trusted private directory.
Application identity is an ownership convention, not authentication against a
maliciously crafted SQLite file.

## Architecture and comparison semantics

A single Cargo package separates Clap parsing, validated TOML configuration,
collectors, a Serde observation model, SQLite persistence, a pure comparison
engine, and human-readable reporting. Observation and database versions start at
1. Unknown versions and unreadable/corrupt data produce errors. Snapshot IDs are
SQLite autoincrement integers; a transaction commits each complete JSON snapshot
payload and timestamp together. Snapshots retain collector source, scope, status,
and errors. Host compatibility uses `/etc/machine-id`; no hostname or environment
variables are persisted. A missing or invalid machine ID prevents collection.
Cloned hosts with identical machine IDs cannot be distinguished. CLI orchestration
accepts an injectable collector in the library, so tests use synthetic host
identity without a production option to spoof `/etc/machine-id`.

Resources and properties use sorted maps. Stable identities are systemd unit
names, TCP family/address/port tuples, and normalized configured file paths.
Timestamps, resource ordering, and collection errors do not affect property
equality. Diff output includes previous/current properties and collector outcomes.
With complete, matching collector scope and source, resources classify as added,
removed, changed, or unchanged. If either collector is missing, disabled, partial,
or failed, or scope/source differs, that collector's resource comparisons are
indeterminate. TCP additionally requires a known, equal network namespace scope.
Namespace scope combines boot ID with namespace device/inode identity; snapshots
from different boots are conservatively indeterminate for TCP. Old observations
without namespace scope deserialize safely and compare as indeterminate. New
scope and auxiliary process fields are backward-compatible additions to observation
version 1. This conservative rule also avoids treating missing optional
properties in partial evidence as changes. Complete empty results remain distinct
from unavailable evidence. A change carries no inferred severity or causation.

## Collection, safety, and limitations

- **Systemd:** fixed `systemctl list-units` and `list-unit-files` invocations,
  without a shell, collect service load/active/substate and unit-file state.
  Unloaded installed services have unit-file state alone. Output parsing uses the
  C locale and rejects unexpected rows. A missing executable, inaccessible system
  manager, or failed command becomes failed/partial evidence. Only the system
  manager is covered, not per-user managers. Commands depend on the host PATH and
  may wait on the system manager; no execution timeout is currently implemented.
- **TCP:** `/proc/net/tcp` and `/proc/net/tcp6` collect listening endpoints in the
  current network namespace. Kernel native-endian address words are normalized to
  IP strings. Duplicate sockets at one endpoint collapse to one resource. Process
  association is optional: visible `/proc/PID/fd` socket links are joined to TCP
  inodes, with process start time, namespace and endpoint/inode rechecks to reduce
  PID/socket reuse races. Only PID and short `comm` name are persisted. `/proc/PID/stat`
  start time is used transiently; command arguments, environments, executable paths,
  and monitored file contents are not collected. Empty associations mean unknown;
  a nonempty list need not identify every owner. Access failures in process attribution
  do not remove listeners or make listener coverage incomplete. Process annotations
  are excluded from comparable properties, so PID/name changes do not create drift.
  Missing tables are incomplete evidence (including disabled IPv6). Missing or
  changing namespace identity prevents confirmed TCP comparisons. All collection
  remains sequential and best effort; it cannot establish an atomic process/socket
  ownership view.
- **Files:** only explicitly configured paths are inspected with
  `symlink_metadata`. Existence, type, mode, numeric UID and GID are recorded; no
  contents, hashes, sizes, timestamps, symlink targets, or recursive directory
  listings are collected. Final symlinks, including dangling links, are recorded
  as symlinks. Symlink ancestors are rejected. Not-found is distinct from other
  access errors. Metadata checks are best effort and do not prevent concurrent
  filesystem races, but never open monitored files for content access.

Observations are sequential, not an atomic view of the running system. Collector
errors are printed as well as persisted; scans with incomplete evidence still
succeed so useful partial snapshots can be saved. Read the displayed statuses.
Snapshots contain local machine identity, paths, ownership, services and bind
addresses, so keep them private. Normal commands need neither root nor service
configuration changes. Only Drift's database/directories are written, aside from
build tools and explicitly created configuration files.

## Verification status

Local offline compilation, Clippy with warnings denied, and tests passed after
hardening. Tests include successful systemd output fixtures, injected
permission-denied metadata errors (independent of host/root permission behavior),
unchanged rejection of unrelated/legacy databases and sidecars, schema/version
checks, namespace compatibility, process attribution fixtures, PID-insensitive
diffing, serialization, and CLI orchestration with synthetic host identity.
Formatting passes. All 22 tests passed (one unit test and 21 integration tests).
On the NixOS host, `nix develop --offline path:. --command rustc --version`
succeeded. Real default and example-config scans succeeded; inspect the collector
statuses rather than treating exit success as complete evidence. A temporary
loopback listener was detected and correctly associated with its owning process.
Snapshot creation, listing, latest comparison, and explicit-ID comparison all
succeeded using disposable state outside the repository. Host verification logs
are in `/tmp/drift-host-verification.log` (mode 0600; temporary, not a release artifact).

These checks verify the current host and development shell; they do not establish
compatibility with every NixOS release, permission setup, or network namespace.
Collection remains sequential and best effort.

## Git repository

Earlier sessions exposed an empty `.git` directory, initially behind a read-only
sandbox mount. After unrestricted inspection confirmed the underlying directory
was also empty and unmounted, a fresh repository was initialized on `main` with
explicit approval. No existing Git history was replaced. No remote is configured.
Cargo.lock and flake.lock are tracked for reproducibility; build outputs, local
databases and sidecars, runtime state, credentials, and editor temporary files
are excluded by `.gitignore`.
