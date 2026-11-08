# NixOS coverage analysis — Milestone 2.1

## Method and baseline

The verified starting point was commit
`3d55ac108b3a6bb3f7a069f5437ba0a7693a2e47` on
`feat/nixos-declared-state`. An unprivileged `drift scan --json` was captured before
any changes. Phase A added codes and grouping with unchanged classification
semantics, tested those diagnostics, and reran the live scan before Phase B.
The inventory stayed at 166 service declarations and 231 other skipped entries.
Private raw captures are under `/tmp/drift-coverage-*`; no host unit inventory,
command text, credential values or private paths are committed here.

The NixOS 26.05 host runs systemd 260.5. Booted and activated generations differ,
and the live unit tree belongs to the activated generation. Both references were
resolved independently and rechecked; the selected profile is not substituted.
Thus supported declarations cannot produce confirmed runtime comparisons here.
There were zero matches and zero mismatches before and after. These numbers
measure this host/point in time, not a benchmark or a target count.

| View | Before | After |
| --- | ---: | ---: |
| Declarations supported | 8 | 11 |
| Declarations unsupported | 158 | 155 |
| Declarations indeterminate | 0 | 0 |
| Comparisons unsupported | 158 | 155 |
| Comparisons indeterminate | 8 | 11 |
| Comparisons match / mismatch | 0 / 0 | 0 / 0 |
| Total declarations / declaration comparisons | 166 / 166 | 166 / 166 |
| Assertions without declarations | 0 | 0 |

The following is the complete comparison reason breakdown. Before counts use
the Phase A codes, which preserve the original Milestone 2 classifications.

| Primary comparison reason | Before | After |
| --- | ---: | ---: |
| `conditional_semantics` | 29 | 29 |
| `generation_drop_ins` | 27 | 27 |
| `socket_activation` | 22 | 22 |
| `unsupported_identity` | 22 | 22 |
| `unsupported_type` | 18 | 18 |
| `artifact_naming` | 15 | 15 |
| `implicit_type` | 10 | 7 |
| `scope_incompatible` | 8 | 11 |
| `masked_unit` | 6 | 6 |
| `timer_activation` | 6 | 6 |
| `path_activation` | 3 | 3 |

Declaration reasons are the same except `scope_incompatible` is replaced by
`supported_service` (8 before/after), plus `implicit_simple_service` (0 → 1) and
`implicit_dbus_service` (0 → 2). A declaration's supported classification means
its expectation is understood, not that runtime agrees or the service is healthy.
A primary reason describes the first decisive exclusion; one unit can have other
unsupported features, but is counted once in each view, never once per feature.

## Selection and narrowly supported semantics

The largest group, conditional/retained semantics (29), contains conditions,
assertions and/or true RemainAfterExit/StopWhenUnneeded settings. Its measured
boolean settings offered no simple explicit-false subset to admit. Evaluating
conditions or inferring persistence/activation would require more evidence.
Generation drop-ins (27) require systemd precedence, resets and merged semantics;
no safe small merge implementation was justified. Templates, companion activation,
masks, aliases/noncanonical artifacts, oneshots and forking remain excluded.
No category was admitted just to improve counts.

Of ten implicit-Type declarations, seven lack ExecStart, two have ExecStart and
BusName, and one has ExecStart without BusName. None of those three candidates
uses credential directives. This motivated only the following two explicit cases:

- **Default simple:** Type absent, exactly one nonempty ExecStart, BusName absent,
  no credential directives, and all existing provenance/activation checks satisfied.
- **Default dbus:** Type absent, exactly one nonempty ExecStart, exactly one static
  well-known BusName, no credential directives, and the same existing checks.
  Accepted names use dot-separated ASCII identifier components with alphabetic or
  underscore starts. This deliberately excludes some valid names rather than guess
  about quoting, escaping, unique names or specifier expansion.

The interpretation is grounded in the
[installed-version systemd service specification (v260.5)](https://github.com/systemd/systemd/blob/v260.5/man/systemd.service.xml).
Credential-implied exec, empty/reset/repeated ExecStart or BusName directives,
unknown explicit Types, continuations, dynamic bus names, conditions, drop-ins
and activation companions remain unsupported. Commands are inspected only for
presence; they are never executed, copied into reports, or evaluated. Bus-name
values and credential contents are not included in diagnostics.

These cases derive only a service Type and artifact expectation. Inactivity,
enablement, static/indirect status and dependency membership still imply no
continuous activity expectation. Activity comparisons require the existing explicit
operator assertion pinned to the exact booted generation. Type/fragment mismatches
require compatible generation/live-tree scope, complete agreeing observations,
loaded state, no pending reload, and no uninterpreted runtime overrides/triggers.
Wrong or unavailable scope, partial observations and missing provenance remain
indeterminate; unsupported activation remains unsupported. No risk score is added.

## Summary and compatibility contract

`declared.coverage` is derived from the actual declarations and results every
time it is rendered or serialized. Its three independent groups are:

- `declarations`: supported/unsupported/indeterminate classification of artifacts.
- `comparisons`: match/mismatch/unsupported/indeterminate, one per declaration.
- `expectation_only`: results for operator assertions without declarations, kept
  outside the declaration denominator to avoid double counting.

Each group has `total`, `by_status`, and `by_reason`. Status maps are sorted by
name; reason maps and limitation sets have deterministic code ordering. Zero
buckets are omitted. `scope` is compatible/incompatible/unavailable, a compatibility
statement rather than a health assessment. Generation identity stays in the
existing `declared.generation`; summary fields contain no path/value payloads.
`limitations` is the deduplicated set of scope/collection/unknown-evidence codes.
Skipped non-service entries are not service comparisons.

Version 1 JSON gains `reason_code` on the audit, declarations and comparisons,
`diagnostic_codes` beside the existing text diagnostics, and `coverage`.
Existing status/reason fields, support-tag encoding and detailed evidence are
retained. Legacy Milestone 2 JSON without codes deserializes with `unknown` codes;
coverage scope is conservatively unavailable. Incoming coverage counts are ignored
and recomputed, so stale or supplied counts cannot change classifications. This
is additive report evolution, not a SQLite or historical observation migration.
Milestone 1 snapshot schemas and historical diff equality are unchanged.

## Stable reason-code catalog

Codes are assigned at decision sites, not inferred from English error text.
An OS error may add detail to an explanation, but does not become a code or label.

| Codes | Meaning |
| --- | --- |
| `supported_service` | Explicit supported service Type, no static exclusion found. |
| `implicit_simple_service`, `implicit_dbus_service` | The two narrowly evidenced omitted-Type cases above. |
| `masked_unit` | Artifact resolves to `/dev/null`; no activity expectation. |
| `artifact_outside_store` | Artifact not anchored in the supported store layout. |
| `unsupported_identity` | Template/instance or invalid/unsupported unit identity. |
| `artifact_naming` | Alias or canonical artifact filename differs; not interpreted. |
| `socket_activation`, `timer_activation`, `path_activation` | Corresponding activation companion exists. |
| `generation_drop_ins` | Generation overrides exist; no merging is attempted. |
| `artifact_unavailable` | Artifact or related activation/override metadata cannot be established. |
| `continued_directive`, `unsupported_syntax` | Parsing is outside the narrow unit-file grammar. |
| `conditional_semantics` | Conditions, assertions, retained state or activation-dependent directives. |
| `repeated_type` | Conflicting/repeated explicit Type directives. |
| `unsupported_type` | Explicit oneshot, forking, or unknown Type outside supported classes. |
| `implicit_type` | Default Type cannot be derived under the strict rules above. |
| `scope_incompatible` | Booted/activated/live-tree scopes disagree or a scope reference is missing. |
| `observation_incomplete` | Systemd collector missing, disabled, incomplete or incompatible. |
| `unit_observation_missing` | No ordinary runtime evidence for this declared unit. |
| `runtime_unavailable` | Supplemental metadata inaccessible, incomplete, malformed or skipped. |
| `runtime_unit_missing` | Supplemental unit properties absent, including possible aliases. |
| `runtime_activation`, `runtime_drop_ins` | Runtime triggers or overrides are not interpreted. |
| `unit_not_loaded` | Runtime load state is not loaded. |
| `daemon_reload_required` | Reload is pending or its state is unavailable. |
| `observation_changed` | Ordinary and supplemental runtime properties disagree. |
| `fragment_unavailable` | Resolved artifact/loaded-fragment evidence unavailable. |
| `fragment_mismatch`, `type_mismatch` | Confirmed narrow artifact/Type mismatch in compatible scope. |
| `runtime_type_unavailable`, `runtime_type_unsupported` | Missing or unknown runtime service Type. |
| `expectation_generation_mismatch` | Operator assertion refers to a different generation. |
| `transitional_activity` | Service transitioning; activity comparison cannot be confirmed. |
| `activity_match`, `activity_mismatch` | Explicit generation-pinned operator activity assertion agrees/disagrees. |
| `artifact_match` | Artifact and Type agree; activity remains informational. |
| `expectation_declaration_missing` | Operator assertion has no booted declaration to compare. |
| `generation_changed` | Reference recheck invalidates comparisons from this collection. |
| `layout_detected` | NixOS layout capability established, not a runtime health result. |
| `os_detection_unavailable`, `non_nixos` | OS metadata unavailable or NixOS identity not established. |
| `container_unsupported`, `container_detection_unavailable` | Container layout excluded or detection unavailable. |
| `store_unavailable`, `generation_unavailable` | Store/reference missing, inaccessible or unsupported. |
| `generation_metadata_unavailable`, `units_unavailable` | Generation metadata/unit tree missing or unreadable. |
| `disabled` | Explicit `scan --no-declared`. |
| `unknown` | Legacy report lacks a code; no new interpretation is invented. |

## Verification and limits

Exact checks, in the Nix development shell:

```sh
nix develop --offline path:.
cargo fmt --check
cargo clippy --offline --locked --all-targets -- -D warnings
cargo test --offline --locked
cargo build --offline --locked
./target/debug/drift scan
./target/debug/drift scan --json
./target/debug/drift scan --no-declared
```

Historical smoke checks use disposable Drift-owned state:

```sh
state_dir=$(mktemp -d)
XDG_STATE_HOME="$state_dir" ./target/debug/drift snapshot create
XDG_STATE_HOME="$state_dir" ./target/debug/drift snapshot create
XDG_STATE_HOME="$state_dir" ./target/debug/drift snapshot list
XDG_STATE_HOME="$state_dir" ./target/debug/drift diff --latest
XDG_STATE_HOME="$state_dir" ./target/debug/drift diff --from 1 --to 2
```

The fixture suite covers matched and mismatched activity/artifacts, incompatible
or missing generations, uncertain provenance, activation ambiguity, partial
collection, exact grouping, legacy JSON and deterministic serialization. Live
matched-generation results cannot be established on this host without changing
its generation; those paths are verified with fixtures only. No service state,
system configuration, boot selection, or generation is changed for verification.
The adapter remains a limited artifact auditor, not a NixOS evaluator, systemd
merge engine, condition evaluator or service health/risk classifier.
