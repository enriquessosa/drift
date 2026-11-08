# Findings rules (schema version 1)

Findings are a separate analysis of normalized observations. They do not change
snapshot data, the SQLite schema, collector behavior, or historical comparison
semantics. A finding is a prompt to investigate, not proof of compromise, a
vulnerability, service impact, or user intent.

## Output and provenance

Use `drift diff --latest --findings` to show historical raw changes followed by
findings. Add `--json` to get a structured document containing the raw
comparison and a separate `findings` report. The report includes schema version
1, stable rule/resource IDs, category, severity, normalized previous/current
evidence, snapshot ID/timestamp provenance, confidence, evidence limitations,
and non-destructive investigation guidance. Results sort by stable finding ID.
Counts by category and severity are separate from counts of raw
added/removed/changed/unchanged/indeterminate/unsupported comparisons.

`drift scan --findings` evaluates only confirmed declared/runtime mismatches
from that same live scan's NixOS audit. These audit results are intentionally
not persisted in historical snapshots, so they are not attributed to a past
snapshot pair. `scan --json` without `--findings` retains its existing shape;
with the flag it adds the findings report. Findings are never saved to SQLite.

## Severity meanings

| Severity | Meaning in this ruleset |
| --- | --- |
| `info` | A factual observation worth review; no adverse impact is inferred. |
| `low` | A bounded, evidence-backed discrepancy merits investigation, but the available evidence lacks context to claim material risk. |
| `medium`, `high` | Reserved. No initial rule assigns these severities. |

Category indicates the kind of observation, not a risk score. A rule's
confidence describes how directly the evidence supports that narrow statement;
it does not increase the severity. Missing, partial, failed, disabled, or
incompatible evidence stays indeterminate/unsupported and cannot create a
confirmed historical finding.

## Rules

### `network.tcp_listener_new`

- **Trigger:** A TCP listener resource is added between complete, comparable
  network observations with equal known network namespace scope.
- **Category/severity:** informational / `info`.
- **Evidence:** family, local bind address, port, snapshot IDs, and timestamps.
- **Limit:** This is an observed bind endpoint only. It does not establish
  firewall policy, external reachability, malicious intent, or service
  criticality. Optional PID/name process annotations are excluded from finding
  identity and evidence.

### TCP bind-scope widening

No widened-bind finding is emitted. The collector currently has no stable
service identity that safely pairs a loopback endpoint with a wildcard or
non-loopback endpoint across scans. PID is transient and process `comm` is not
unique or durable; changed ports and process associations cannot establish the
same service. Drift therefore shows the comparison as raw listener removal(s)
and addition(s), including the ordinary new-listener informational rule for a
confirmed added endpoint. This avoids inventing a service pairing.

### `file.permissions_broadened`

- **Trigger:** The same configured path is a present regular file in both
  complete comparable observations, and at least one rwx bit in the low nine
  permission bits (`0777`) becomes set.
- **Category/severity:** security / `low`.
- **Evidence:** prior/current metadata, including mode, UID and GID.
- **Limit:** Contents, purpose, sensitivity, ACLs, and actual effective access
  are not observed. The rule does not call a change a vulnerability. Symlinks,
  non-files, missing prior state, malformed modes, and unavailable/partial
  metadata do not produce this finding. Special mode bits are shown in the raw
  evidence but are excluded from the rwx broadening test.

### `systemd.service_failed`

- **Trigger:** The same service identity has a known prior `active` value other
  than `failed`, then `active=failed`, in complete comparable systemd
  collections.
- **Category/severity:** reliability / `low`.
- **Evidence:** normalized prior/current unit properties and snapshot
  provenance.
- **Limit:** Snapshots establish state only at collection times. Duration,
  criticality, cause, and impact are unknown. `inactive`, including the
  ordinary inactive state of oneshot services, is not failure.

### `nixos.declared_runtime_mismatch`

- **Trigger:** The live NixOS adapter reports a `Mismatch` for a supported
  declaration and an explicitly recognized mismatch reason (fragment, service
  Type, or a generation-pinned activity expectation), with booted, activated,
  and live unit-tree references all compatible. Indeterminate, unsupported,
  unknown-reason, or incompatible-generation results are excluded.
- **Category/severity:** fragment/Type differences are informational / `low`;
  activity expectation differences are reliability / `low`.
- **Evidence:** declared generation/type/fragment, the adapter's mismatch
  reason, and declared/runtime source provenance.
- **Limit:** The adapter implements a narrow systemd subset. A discrepancy
  does not establish its cause or harmfulness. Activity findings require the
  existing explicit operator expectation pinned to the exact booted generation;
  enablement alone is not activity intent.

## Determinism and safety

Rules consume the existing pure comparison result and typed NixOS audit. They
perform no collection, SQLite writes, Nix evaluation, service commands, or
remediation. In historical analysis, additions/removals are confirmed only when
the comparison engine already established complete matching source/coverage
and, for TCP, equal known namespace scope. Unavailable/partial observations
remain separate indeterminate comparisons. The JSON report excludes TCP
process annotations and includes no file contents or systemd command lines.
