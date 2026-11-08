# Drift

Drift is an unprivileged, read-only Rust CLI for Linux/NixOS system auditing.
It compares historical snapshots and performs limited NixOS declared-state checks
when supported semantics and compatible generation evidence are available.
Findings are generated on demand and receive conservative severity indicating
investigation priority, not proof of exploitation or compromise. Not every
change becomes a finding; raw changes and indeterminate comparisons remain distinct.

Drift is MIT-licensed; see [LICENSE](LICENSE) and
[third-party notices](THIRD_PARTY_NOTICES.md). The copyright-holder placeholder
must be confirmed before publication.

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

- `drift scan`: collect observations and audit supported NixOS declarations without opening a DB.
- `drift scan --no-declared`: disable declared-state auditing for troubleshooting.
- `drift scan --json`: emit the observation and declared audit in a JSON envelope.
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

## NixOS declared-state coverage (Milestones 2 and 2.1)

Declared-state auditing is separate from historical snapshot diffing. `scan`
adds a typed audit to its existing observation summary. Snapshot creation, SQLite
schema/version, version-1 observations, and historical comparisons are unchanged;
declared expectations/results are not silently persisted into old snapshots.
`scan --json` emits `{"observation": ..., "declared": ...}`. The declared report
has its own version 1, capability/reason, generation provenance, declarations,
comparison outcomes, diagnostics, and skipped-entry count. Milestone 2.1 adds
stable `reason_code`/`diagnostic_codes` fields and a derived `coverage` summary.
These are additive version-1 JSON fields; older documents still deserialize
with unknown reason codes. Historical snapshots and SQLite are unaffected.
Human output retains
the ordinary collector summary and adds evidence and reasons for each result.

The adapter explicitly requires `ID=nixos` in os-release, an accessible
`/run/booted-system` symlink resolving directly to a Nix store system generation,
`nixos-version` metadata, and an `etc/systemd/system` artifact tree in the store.
This layout was checked on this NixOS 26.05 host; other versions are supported
only when they satisfy those layout assumptions, not by an unverified version
compatibility promise. Containers identified by `/run/systemd/container` are
unsupported. Missing references, inaccessible metadata and unsupported layouts
are reported explicitly; ordinary Linux scanning still works on non-NixOS hosts.

The booted generation is always the reference. `/run/current-system` records the
activated generation and is checked separately, along with the canonical live
`/etc/systemd/system` tree. The selected profile at
`/nix/var/nix/profiles/system` is never used as a substitute. If activated and
booted generations differ (for example after switching without rebooting), the
adapter retains booted provenance but cannot confirm runtime mismatches. References
are checked again after collection. These are trusted local system references,
not cryptographic attestation of the machine or isolation from concurrent changes.

Only top-level system `.service` artifacts are inventoried. A supported service
has an explicit Type of `simple`, `exec`, `notify`, `notify-reload`, `dbus`, or
`idle`, without interpreted override/activation complications. Milestone 2.1 also
recognizes omitted Type as `simple` when exactly one nonempty ExecStart and no
BusName or credential directives exist, or as `dbus` with one static, unambiguous
BusName and that same start/credential restriction. Repeated/reset directives,
credential-implied defaults and dynamic/escaped bus names remain unsupported.
Templates and instances, aliases, masks, other implicit/unknown Types, oneshot/forking services,
conditions/asserts, ExecCondition, RemainAfterExit, StopWhenUnneeded, socket/timer/path
companions, continued directives, and generation or runtime drop-ins are
unsupported. Runtime `TriggeredBy` also excludes activation-dependent comparisons.
Targets and non-service entries are counted as skipped. This intentionally leaves
many real NixOS units unsupported, including generated units with override drop-ins.
No systemd merge precedence, target graph, or arbitrary NixOS option semantics
are approximated. Unit artifacts are read transiently (bounded to 1 MiB) solely
to classify supported metadata. ExecStart commands, environments and other raw
unit content are not included in reports or snapshots; no commands from artifacts
are executed. Configured monitored files still receive metadata-only inspection.

For supported declarations, fresh machine-readable `systemctl show` properties
supplement the existing systemd observations. Disabled or incomplete systemd
collection suppresses those supplemental queries. Complete, agreeing observation
scope, matching generation/live-tree scope, a loaded unit, no pending daemon
reload, and resolved artifact evidence are required. A match establishes only
that the loaded immutable fragment and service Type agree. A different loaded
fragment or Type is a narrowly described mismatch, without severity or an
inference about causation. Missing units/properties, access failures, changing
runtime evidence, or generation ambiguity are indeterminate. An inactive, enabled,
static, indirect or disabled service is never an activity mismatch by itself.

Optional operator assertions express activity intent explicitly, pinned to the
exact booted store path. For example, in a TOML configuration:

```toml
[[nixos_expectations]]
unit = "example.service"
generation = "/nix/store/00000000000000000000000000000000-nixos-system-example-26.05"
active = true
```

Replace that illustrative generation with the actual resolved booted path after
reviewing the service semantics. `active = false` means not active (including
failed); this is not a health check. A supported, fully evidenced service can
match or mismatch that explicit assertion. Transitional states, wrong-generation
assertions and assertions without a declaration are indeterminate. Unsupported
services remain unsupported even with an assertion. Duplicate units, templates,
relative generation paths and unsafe unit names are rejected. The adapter runs
no Nix evaluation, rebuild, activation, remediation, or privileged operation.

Examples (the exact inventory depends on the host):

```sh
cargo run --offline --locked -- scan
cargo run --offline --locked -- scan --json
cargo run --offline --locked -- scan --no-declared
readlink -f /run/booted-system
readlink -f /run/current-system
systemctl show example.service --no-pager -p Id -p LoadState -p ActiveState -p Type -p FragmentPath -p DropInPaths -p NeedDaemonReload
```

```text
Declared state: Supported [layout_detected]: limited booted-generation system service artifact audit; enablement does not imply activity
  Match example.service [artifact_match]: unit artifact and Type agree; activity is informational, with no continuous activity expectation
  Unsupported setup.service [unsupported_type]: oneshot, forking, or unknown service Type is outside supported continuous classes
```

These are illustrative results, not assertions that those services exist locally.
The reference distinction follows [NixOS stage-2 initialization](https://github.com/NixOS/nixpkgs/blob/master/nixos/modules/system/boot/stage-2-init.sh)
and the installed generation's activation script. Runtime metadata uses the
machine-readable [systemctl show interface](https://www.freedesktop.org/software/systemd/man/latest/systemctl.html#show%20PATTERN%E2%80%A6%7CJOB%E2%80%A6).

### Coverage diagnostics

`scan` and `scan --json` group declarations and comparisons separately by status
and stable reason code. **Unsupported** means the semantics/layout are outside
this adapter's contract. **Indeterminate** means missing, inaccessible, conflicting,
or incompatible evidence prevents a supported comparison. Neither is healthy or
a confirmed mismatch. Ordinary observation collection remains available.

`declared.coverage` contains `declarations`, `comparisons`, and `expectation_only`
counts (`total`, `by_status`, `by_reason`), `scope`, `limitations`, and skipped
non-service entries. The existing `declared.generation` supplies generation
identifiers/provenance without duplicating paths in the summary. Each declaration
and its result has one primary code; totals are not additive across these two
views. Assertions lacking a declaration go only into `expectation_only`.
Only nonzero buckets are emitted. Counts are recomputed from results during
serialization/rendering, including after a reference changes; incoming JSON
counts are never trusted. Summaries contain categories/counts, not unit names,
commands, credential values or private file paths. Existing detailed evidence
is retained separately.

For example, a host with a generation mismatch can report:

```text
  scope: Incompatible; limitations: scope_incompatible
  declarations: 166; status counts: {"supported": 11, "unsupported": 155}
    supported_service: 8
    implicit_simple_service: 1
    implicit_dbus_service: 2
    ...
  comparisons: 166; status counts: {"indeterminate": 11, "unsupported": 155}
    ...
    scope_incompatible: 11
```

This abbreviated example reflects the measured host; it is not a universal
NixOS inventory. See [coverage analysis and reason-code catalog](docs/nixos-coverage.md)
for before/after counts, chosen cases, rejected categories, code meanings, JSON
compatibility, and reproducible verification commands.

## Security and reliability findings (Milestone 3)

Historical findings are an opt-in analysis layered on the existing raw diff:

```sh
drift diff --latest --findings
drift diff --from 10 --to 11 --findings --json
drift scan --findings --json
```

`diff --findings` preserves raw comparison output and appends separate findings.
Its JSON form has independent `comparison` and versioned `findings` objects.
`scan --findings` evaluates only the current scan's confirmed
declared/runtime mismatches; those results are not persisted and are not
attributed to historical snapshots. Findings are never written to SQLite.
Without `--findings`, existing scan, snapshot, diff, and JSON behavior remains
unchanged.

Initial rules report a newly observed TCP listener (informational), rwx
permission broadening on a configured regular file (low security), a systemd
service transition into `failed` (low reliability), and narrowly confirmed
NixOS declared/runtime mismatches (low informational or reliability). Every
finding carries stable rule/resource IDs, normalized evidence, provenance,
confidence, limitations and investigation guidance. Incomplete, unsupported or
scope-incompatible comparisons remain separate and cannot become findings. A
finding is a prompt to investigate, not proof of compromise or system health.

TCP bind-scope widening is intentionally not inferred: current process
annotations do not provide a reliable durable service identity. Address/port
changes remain raw listener removals/additions rather than being paired by PID
or process name. Exact triggers, severity rationale, JSON fields and remaining
limitations are documented in [the findings rules reference](docs/findings-rules.md).

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
Formatting passed for Milestone 1 with 22 tests (one unit test and 21 integration tests).
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

Milestone 2 adds 19 synthetic NixOS tests, for 41 tests total. Required formatting,
Clippy, tests and build are rerun in the Nix development shell before committing.
A real read-only NixOS scan resolved the booted and activated generation paths
independently, confirmed the live unit tree belonged to the activated generation,
and inventoried 166 booted service declarations: 158 unsupported and 8
indeterminate. There were no confirmed mismatches. This is expected on this host:
booted and activated generations differ. Matched-scope matches and mismatches are
verified with fixtures, not by changing or rebooting the host. JSON output and
`--no-declared` are checked alongside unchanged historical snapshot operations.
Milestone 2 verification logs are kept privately under `/tmp/drift-milestone2-*`.

Milestone 2.1 brings the suite to 50 tests, with coverage grouping, privacy, legacy JSON,
missing evidence, inferred-type success and rejection fixtures. The real host's
166-declaration comparison moves from 158 unsupported/8 indeterminate to
155 unsupported/11 indeterminate, with no matches or mismatches asserted while
generations differ. Detailed results and limitations are in the coverage analysis.
Private verification artifacts use `/tmp/drift-coverage-*` and are not committed.

Milestone 3 adds deterministic historical and same-scan declared/runtime
findings. The required `cargo fmt --check`, offline locked Clippy, tests, and
build passed in the Nix development shell; 60 tests passed (11 unit, 21
foundation, 28 NixOS). The real unprivileged scan returned Complete for systemd,
network and files (202, 16 and 0 resources respectively). Scan JSON,
`--findings`, `--no-declared`, and two snapshot/list/latest-diff smoke checks
passed using a private temporary database outside production state. The NixOS
adapter reported supported capability, but booted/activated/live references did
not agree: 155 declarations remained unsupported and 11 comparisons
indeterminate, with no match or mismatch claimed. These results do not verify a
matched-generation host comparison. Fixture tests cover that case without
changing or rebooting the host.

## Git repository

Earlier sessions exposed an empty `.git` directory, initially behind a read-only
sandbox mount. After unrestricted inspection confirmed the underlying directory
was also empty and unmounted, a fresh repository was initialized on `main` with
explicit approval. Later, with an independently verified repository backup,
local history was rewritten to remove committed milestone prompt documents while
preserving the milestone implementation commits. No remote is configured.
Cargo.lock and flake.lock are tracked for reproducibility; build outputs, local
databases and sidecars, runtime state, credentials, and editor temporary files
are excluded by `.gitignore`.

## Initial release preparation (0.1.0)

See [installation, CLI, exit codes and troubleshooting](docs/release.md) and
[release notes](CHANGELOG.md). Build with `nix build path:.` and run
`./result/bin/drift --help`. Synthetic demonstrations are available with
`cargo run --offline --locked --example demo` and the `-- --incomplete` variant.
