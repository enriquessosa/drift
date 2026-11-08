# Drift — Agent Instructions

## Mission
Drift is a read-only, CLI-first Linux system auditor written in Rust. It compares observed runtime state with historical snapshots and, in a later milestone, supported NixOS declared-state expectations. Findings must distinguish security, reliability, and informational observations without overstating causation or risk.

## Architectural boundaries
- Maintain a single Cargo package with cohesive modules unless a split is justified.
- Separate collection, normalized observations, persistence, comparison, and presentation.
- Keep the comparison engine deterministic, independent of SQLite and terminal rendering.
- Version persisted schemas and observation representations.
- Preserve evidence provenance and explicit collection completeness. A failed or partial collector is **not** an empty successful result.
- Use stable resource identities; exclude timestamps and incidental ordering from equality comparisons.
- Do not evaluate arbitrary Nix source as a shortcut to declared-state auditing.

## Safety and privacy
- Run unprivileged by default. Never require root for normal commands.
- Audit only the local machine; do not add remote scanning, exploitation, or automated remediation.
- Do not modify system configuration or service state. The only expected writes are Drift-owned snapshots, reports, and explicitly requested configuration artifacts.
- Never read monitored file contents; collect only explicitly configured file metadata. Avoid collecting secrets, environment variables, and process command lines.
- Avoid following symlinks unexpectedly; represent access failures and unknowns explicitly.
- Create private local state with restrictive permissions. Do not log sensitive data unnecessarily.
- No arbitrary shell commands from user configuration.

## Development conventions
- Prefer clear Rust types, explicit error handling, and small modules over premature frameworks.
- Use Clap for CLI parsing, Serde for serialization, TOML for configuration, and SQLite for persistence.
- Provide a Nix flake development shell compatible with NixOS.
- Store runtime data under XDG state directories, not inside the repository. Ensure Nix flake evaluation is not disrupted by generated files.
- Preserve existing files and user changes; avoid unrelated rewrites.
- Document behavior and limitations, especially permission restrictions and incomplete evidence.

## Testing and verification
- Add deterministic unit tests for identities, diffing, missing/incomplete evidence, and stable ordering.
- Use temporary files and synthetic fixtures in integration tests. Never change host service configuration in tests.
- Attempt `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`, and `cargo build` after changes.
- If sandbox networking, registry access, or Nix daemon access fails, report it accurately; do not claim unrun checks passed or weaken tests to hide failures.

## Milestone discipline
- Implement only the milestone explicitly requested.
- Milestone 1: local collectors, snapshot persistence, deterministic historical diff, CLI, and tests.
- Later milestones: NixOS declared-state adapter; evidence-based security/reliability rules; enhanced reporting and release hardening.
- No web UI, background daemon, remote monitoring, automatic remediation, or elevated helper in Milestone 1.

## Completion report
Summarize modified files, architecture, implemented commands, checks actually run, limitations, and exact host verification commands.
