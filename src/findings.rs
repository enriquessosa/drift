//! Conservative, deterministic rules over already-normalized comparisons.
//! This module performs no collection, persistence, or terminal I/O.

use crate::{
    diff::{Comparison, Entry, Kind},
    model::{Resource, Snapshot},
    nixos::{
        coverage::ReasonCode,
        model::{Audit, Capability, Outcome, Support},
    },
};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Security,
    Reliability,
    Informational,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    DirectObservation,
    ConstrainedComparison,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Provenance {
    pub kind: String,
    pub source: String,
    pub previous_snapshot_id: Option<i64>,
    pub current_snapshot_id: Option<i64>,
    pub previous_timestamp: Option<u64>,
    pub current_timestamp: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Finding {
    /// Stable across runs for the same rule and resource identity.
    pub id: String,
    pub rule_id: String,
    pub category: Category,
    pub severity: Severity,
    pub resource_type: String,
    pub stable_identifier: String,
    pub summary: String,
    pub explanation: String,
    pub previous_evidence: Option<Value>,
    pub current_evidence: Option<Value>,
    pub provenance: Provenance,
    pub confidence: Confidence,
    pub evidence_limitations: Vec<String>,
    pub investigation_guidance: String,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct ComparisonCounts {
    pub added: usize,
    pub removed: usize,
    pub changed: usize,
    pub unchanged: usize,
    pub mismatched: usize,
    pub indeterminate: usize,
    pub unsupported: usize,
}

#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct FindingCounts {
    pub total: usize,
    pub by_category: BTreeMap<Category, usize>,
    pub by_severity: BTreeMap<Severity, usize>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Report {
    pub schema_version: u32,
    pub findings: Vec<Finding>,
    pub counts: FindingCounts,
    /// Raw historical comparison outcomes remain separate from findings.
    pub historical_comparisons: ComparisonCounts,
    /// Declared/runtime results are present only for a same-run scan audit.
    pub declared_comparisons: ComparisonCounts,
}

impl Default for Report {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            findings: Vec::new(),
            counts: FindingCounts::default(),
            historical_comparisons: ComparisonCounts::default(),
            declared_comparisons: ComparisonCounts::default(),
        }
    }
}

/// Evaluate historical rules only on entries the comparison engine has
/// already confirmed comparable. Incomplete or scope-incompatible entries
/// remain in the separate indeterminate count and can never produce a finding.
pub fn historical(
    comparison: &Comparison,
    previous: &Snapshot,
    current: &Snapshot,
    previous_id: i64,
    current_id: i64,
) -> Report {
    let provenance = Provenance {
        kind: "historical_snapshot_comparison".into(),
        source: "Drift normalized snapshot observations".into(),
        previous_snapshot_id: Some(previous_id),
        current_snapshot_id: Some(current_id),
        previous_timestamp: Some(previous.timestamp),
        current_timestamp: Some(current.timestamp),
    };
    let mut report = Report::default();
    for entry in &comparison.entries {
        tally(&mut report.historical_comparisons, &entry.kind);
        match (entry.collector.as_str(), &entry.kind) {
            ("network", Kind::Added) => {
                if let Some(resource) = &entry.current {
                    report
                        .findings
                        .push(listener_added(entry, resource, &provenance));
                }
            }
            ("files", Kind::Changed) => {
                if let (Some(old), Some(new)) = (&entry.previous, &entry.current) {
                    if let Some(finding) = permissions_broadened(entry, old, new, &provenance) {
                        report.findings.push(finding);
                    }
                }
            }
            ("systemd", Kind::Changed) => {
                if let (Some(old), Some(new)) = (&entry.previous, &entry.current) {
                    if let Some(finding) = service_failed(entry, old, new, &provenance) {
                        report.findings.push(finding);
                    }
                }
            }
            // A different TCP address/port is a raw removal plus addition.
            // The existing model has only PID/comm ownership annotations, not
            // a stable service identity; pairing on either risks false drift.
            _ => {}
        }
    }
    finish(&mut report);
    report
}

/// Declared/runtime findings are produced only from the live adapter audit
/// returned alongside the same scan. They are deliberately not attached to a
/// historical diff, because those audit results are not persisted in snapshots.
pub fn declared(audit: &Audit) -> Report {
    let mut report = Report::default();
    let declarations: BTreeMap<_, _> = audit
        .declarations
        .iter()
        .map(|declaration| (declaration.unit.as_str(), declaration))
        .collect();
    let provenance = Provenance {
        kind: "current_declared_runtime_comparison".into(),
        source: "matched booted NixOS generation and live systemd adapter".into(),
        previous_snapshot_id: None,
        current_snapshot_id: None,
        previous_timestamp: None,
        current_timestamp: None,
    };
    for result in &audit.comparisons {
        if result.outcome == Outcome::Unsupported {
            report.declared_comparisons.unsupported += 1;
            continue;
        }
        if result.outcome == Outcome::Indeterminate {
            report.declared_comparisons.indeterminate += 1;
            continue;
        }
        let declaration = declarations.get(result.unit.as_str()).copied();
        if declaration.is_some_and(|d| matches!(d.support, Support::Unsupported(_))) {
            report.declared_comparisons.unsupported += 1;
            continue;
        }
        let generation = audit.generation.as_ref();
        let comparable = audit.version == 1
            && audit.capability == Capability::Supported
            && generation.is_some_and(|g| g.compatible())
            && declaration.is_some_and(|d| {
                d.support == Support::Supported
                    && generation.is_some_and(|g| d.generation == g.booted)
            });
        if !comparable {
            report.declared_comparisons.indeterminate += 1;
            continue;
        }
        let Some(declaration) = declaration else {
            report.declared_comparisons.indeterminate += 1;
            continue;
        };
        match result.outcome {
            Outcome::Match => {
                report.declared_comparisons.unchanged += 1;
                continue;
            }
            Outcome::Mismatch
                if matches!(
                    result.reason_code,
                    ReasonCode::FragmentMismatch
                        | ReasonCode::TypeMismatch
                        | ReasonCode::ActivityMismatch
                ) =>
            {
                report.declared_comparisons.mismatched += 1
            }
            Outcome::Mismatch => {
                report.declared_comparisons.indeterminate += 1;
                continue;
            }
            Outcome::Indeterminate | Outcome::Unsupported => unreachable!(),
        }
        let (category, summary) = match result.reason_code {
            ReasonCode::ActivityMismatch => (
                Category::Reliability,
                "Service activity differs from an explicit generation-pinned expectation",
            ),
            _ => (
                Category::Informational,
                "Loaded systemd service differs from the supported declaration",
            ),
        };
        let finding = make_finding(FindingDraft {
            rule_id: "nixos.declared_runtime_mismatch",
            category,
            severity: Severity::Low,
            resource_type: "systemd_service",
            stable_identifier: result.unit.clone(),
            summary,
            explanation: format!(
                "The NixOS adapter confirmed a mismatch for the matched booted generation: {}. This establishes a configuration/runtime difference only; it does not establish cause, impact, or compromise.",
                result.reason
            ),
            previous_evidence: Some(json!({
                "generation": declaration.generation,
                "declared_type": declaration.service_type,
                "declared_fragment": declaration.fragment,
            })),
            current_evidence: Some(json!({
                "outcome": result.outcome,
                "reason_code": result.reason_code,
                "reason": result.reason,
            })),
            provenance: provenance.clone(),
            confidence: Confidence::ConstrainedComparison,
            evidence_limitations: vec![
                "The adapter supports only a narrow subset of systemd unit semantics.".into(),
                "A mismatch does not identify why runtime state differs or whether the difference is harmful.".into(),
            ],
            investigation_guidance: "Review the declared unit, runtime drop-in/manager state, and recent operator actions; do not change service state until the intended configuration is understood.",
        });
        report.findings.push(finding);
    }
    finish(&mut report);
    report
}

fn listener_added(entry: &Entry, resource: &Resource, provenance: &Provenance) -> Finding {
    let family = property(resource, "family");
    let address = property(resource, "address");
    let port = property(resource, "port");
    make_finding(FindingDraft {
        rule_id: "network.tcp_listener_new",
        category: Category::Informational,
        severity: Severity::Info,
        resource_type: "tcp_listener",
        stable_identifier: entry.id.clone(),
        summary: "TCP listener newly observed",
        explanation: format!(
            "A TCP listener was present in the current complete network observation and absent from the previous comparable observation ({family} {address}:{port}). The observation says nothing about firewall policy or reachability outside the host."
        ),
        previous_evidence: entry.previous.as_ref().map(resource_evidence),
        current_evidence: entry.current.as_ref().map(resource_evidence),
        provenance: provenance.clone(),
        confidence: Confidence::DirectObservation,
        evidence_limitations: vec![
            "Socket observation is limited to the collected network namespace and scan times.".into(),
            "External reachability and firewall behavior were not tested.".into(),
            "Optional process attribution is not used as service identity.".into(),
        ],
        investigation_guidance: "Check whether the listener is expected and review its owning service and host firewall policy using your normal administrative process.",
    })
}

fn permissions_broadened(
    entry: &Entry,
    previous: &Resource,
    current: &Resource,
    provenance: &Provenance,
) -> Option<Finding> {
    if previous.properties.get("exists").map(String::as_str) != Some("true")
        || current.properties.get("exists").map(String::as_str) != Some("true")
        || previous.properties.get("type").map(String::as_str) != Some("file")
        || current.properties.get("type").map(String::as_str) != Some("file")
    {
        return None;
    }
    let old_mode = u16::from_str_radix(previous.properties.get("mode")?, 8).ok()? & 0o777;
    let new_mode = u16::from_str_radix(current.properties.get("mode")?, 8).ok()? & 0o777;
    let granted = new_mode & !old_mode;
    if granted == 0 {
        return None;
    }
    Some(make_finding(FindingDraft {
        rule_id: "file.permissions_broadened",
        category: Category::Security,
        severity: Severity::Low,
        resource_type: "monitored_file",
        stable_identifier: entry.id.clone(),
        summary: "Monitored file permission bits broadened",
        explanation: format!(
            "Read/write/execute permission bits changed from {:03o} to {:03o}; newly set bits are {:03o}. This reports metadata only and does not classify the file as sensitive or the change as a vulnerability.",
            old_mode, new_mode, granted
        ),
        previous_evidence: Some(resource_evidence(previous)),
        current_evidence: Some(resource_evidence(current)),
        provenance: provenance.clone(),
        confidence: Confidence::DirectObservation,
        evidence_limitations: vec![
            "Only configured path metadata is observed; file contents and sensitivity are unknown.".into(),
            "Special permission bits are retained in evidence but excluded from this rwx broadening rule.".into(),
        ],
        investigation_guidance: "Confirm the permission change was intended and review access under the relevant user and group identities.",
    }))
}

fn service_failed(
    entry: &Entry,
    previous: &Resource,
    current: &Resource,
    provenance: &Provenance,
) -> Option<Finding> {
    let old_active = previous.properties.get("active")?;
    if old_active == "failed" || current.properties.get("active")?.as_str() != "failed" {
        return None;
    }
    Some(make_finding(FindingDraft {
        rule_id: "systemd.service_failed",
        category: Category::Reliability,
        severity: Severity::Low,
        resource_type: "systemd_service",
        stable_identifier: entry.id.clone(),
        summary: "systemd service transitioned into failed state",
        explanation: format!(
            "The same service was observed as '{}' previously and 'failed' currently in complete comparable systemd collections. An ordinary inactive or oneshot state is not treated as failure.",
            old_active
        ),
        previous_evidence: Some(resource_evidence(previous)),
        current_evidence: Some(resource_evidence(current)),
        provenance: provenance.clone(),
        confidence: Confidence::DirectObservation,
        evidence_limitations: vec![
            "The snapshots establish state at collection times, not how long the service was failed.".into(),
            "Service criticality, user impact, and cause are not inferred.".into(),
        ],
        investigation_guidance: "Inspect the service journal and systemd result details to understand the failure before taking action.",
    }))
}

struct FindingDraft {
    rule_id: &'static str,
    category: Category,
    severity: Severity,
    resource_type: &'static str,
    stable_identifier: String,
    summary: &'static str,
    explanation: String,
    previous_evidence: Option<Value>,
    current_evidence: Option<Value>,
    provenance: Provenance,
    confidence: Confidence,
    evidence_limitations: Vec<String>,
    investigation_guidance: &'static str,
}

fn make_finding(draft: FindingDraft) -> Finding {
    Finding {
        id: format!(
            "{}:{}:{}",
            draft.rule_id, draft.resource_type, draft.stable_identifier
        ),
        rule_id: draft.rule_id.into(),
        category: draft.category,
        severity: draft.severity,
        resource_type: draft.resource_type.into(),
        stable_identifier: draft.stable_identifier,
        summary: draft.summary.into(),
        explanation: draft.explanation,
        previous_evidence: draft.previous_evidence,
        current_evidence: draft.current_evidence,
        provenance: draft.provenance,
        confidence: draft.confidence,
        evidence_limitations: draft.evidence_limitations,
        investigation_guidance: draft.investigation_guidance.into(),
    }
}

fn resource_evidence(resource: &Resource) -> Value {
    // Process annotations can be transient and contain PID/name churn. Findings
    // use only the normalized, comparable properties; never expose arguments.
    json!(resource.properties)
}

fn property<'a>(resource: &'a Resource, key: &str) -> &'a str {
    resource
        .properties
        .get(key)
        .map(String::as_str)
        .unwrap_or("unknown")
}

fn tally(counts: &mut ComparisonCounts, kind: &Kind) {
    match kind {
        Kind::Added => counts.added += 1,
        Kind::Removed => counts.removed += 1,
        Kind::Changed => counts.changed += 1,
        Kind::Unchanged => counts.unchanged += 1,
        Kind::Indeterminate => counts.indeterminate += 1,
    }
}

fn finish(report: &mut Report) {
    report.findings.sort_by(|a, b| a.id.cmp(&b.id));
    report.counts = FindingCounts {
        total: report.findings.len(),
        by_category: BTreeMap::new(),
        by_severity: BTreeMap::new(),
    };
    for finding in &report.findings {
        *report
            .counts
            .by_category
            .entry(finding.category)
            .or_default() += 1;
        *report
            .counts
            .by_severity
            .entry(finding.severity)
            .or_default() += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        diff,
        model::{resource, Collection, Snapshot, Status, VERSION},
        nixos::model::{Comparison as DeclaredComparison, Generation, Support},
    };
    use std::{collections::BTreeMap, path::PathBuf};

    fn snapshot(collectors: BTreeMap<String, Collection>, timestamp: u64) -> Snapshot {
        Snapshot {
            version: VERSION,
            timestamp,
            host: "fixture".into(),
            collectors,
        }
    }

    fn collection(source: &str, coverage: &[&str]) -> Collection {
        Collection::new(source, coverage.iter().map(|s| (*s).into()).collect())
    }

    fn resource_with(pairs: &[(&str, &str)]) -> Resource {
        resource(
            pairs
                .iter()
                .map(|(key, value)| ((*key).into(), (*value).into())),
        )
    }

    fn analyze(old: Collection, new: Collection) -> Report {
        let a = snapshot(BTreeMap::from([(collector_name(&old), old)]), 100);
        let b = snapshot(BTreeMap::from([(collector_name(&new), new)]), 200);
        let compared = diff::compare(&a, &b).unwrap();
        historical(&compared, &a, &b, 4, 9)
    }

    fn collector_name(collection: &Collection) -> String {
        if collection.source.starts_with("systemctl") {
            "systemd".into()
        } else if collection.source.starts_with("symlink_metadata") {
            "files".into()
        } else {
            "network".into()
        }
    }

    #[test]
    fn newly_observed_tcp_listener_is_informational_and_has_snapshot_provenance() {
        let mut old = collection("Linux /proc/net TCP tables", &["tcp4", "tcp6"]);
        old.scope = Some("boot:netns".into());
        let mut new = old.clone();
        new.resources.insert(
            "tcp4:0.0.0.0:8080".into(),
            resource_with(&[("family", "ipv4"), ("address", "0.0.0.0"), ("port", "8080")]),
        );
        let report = analyze(old, new);
        let finding = &report.findings[0];
        assert_eq!(finding.rule_id, "network.tcp_listener_new");
        assert_eq!(finding.category, Category::Informational);
        assert_eq!(finding.severity, Severity::Info);
        assert_eq!(finding.provenance.previous_snapshot_id, Some(4));
        assert_eq!(finding.provenance.current_timestamp, Some(200));
        assert!(finding
            .explanation
            .contains("firewall policy or reachability"));
        assert_eq!(report.historical_comparisons.added, 1);
    }

    #[test]
    fn tcp_scope_mismatch_unknown_scope_and_partial_observations_never_find() {
        for (old_scope, new_scope, partial) in [
            (Some("ns-a"), Some("ns-b"), false),
            (None, None, false),
            (Some("ns-a"), Some("ns-a"), true),
        ] {
            let mut old = collection("Linux /proc/net TCP tables", &["tcp4"]);
            old.scope = old_scope.map(str::to_owned);
            old.resources.insert(
                "tcp4:127.0.0.1:80".into(),
                resource_with(&[("family", "ipv4"), ("address", "127.0.0.1"), ("port", "80")]),
            );
            let mut new = collection("Linux /proc/net TCP tables", &["tcp4"]);
            new.scope = new_scope.map(str::to_owned);
            if partial {
                new.status = Status::Partial;
            }
            let report = analyze(old, new);
            assert!(report.findings.is_empty());
            assert!(report.historical_comparisons.indeterminate > 0);
        }
    }

    #[test]
    fn tcp_listener_addresses_cover_v4_v6_without_reachability_claims() {
        for (id, family, address) in [
            ("tcp4:127.0.0.1:9", "ipv4", "127.0.0.1"),
            ("tcp4:0.0.0.0:9", "ipv4", "0.0.0.0"),
            ("tcp6:::1:9", "ipv6", "::1"),
            ("tcp6::::9", "ipv6", "::"),
        ] {
            let mut old = collection("Linux /proc/net TCP tables", &["tcp4", "tcp6"]);
            old.scope = Some("same".into());
            let mut new = old.clone();
            new.resources.insert(
                id.into(),
                resource_with(&[("family", family), ("address", address), ("port", "9")]),
            );
            let report = analyze(old, new);
            assert_eq!(report.findings.len(), 1);
            assert!(report.findings[0]
                .evidence_limitations
                .iter()
                .any(|line| line.contains("reachability")));
        }
    }

    #[test]
    fn unrelated_endpoint_and_owner_remain_raw_add_remove_without_widened_bind_claim() {
        let mut old = collection("Linux /proc/net TCP tables", &["tcp4"]);
        old.scope = Some("same".into());
        let mut old_listener = resource_with(&[
            ("family", "ipv4"),
            ("address", "127.0.0.1"),
            ("port", "8080"),
        ]);
        old_listener.processes.push(crate::model::Process {
            pid: 500,
            name: "old-owner".into(),
        });
        old.resources
            .insert("tcp4:127.0.0.1:8080".into(), old_listener);
        let mut new = old.clone();
        new.resources.remove("tcp4:127.0.0.1:8080");
        let mut new_listener =
            resource_with(&[("family", "ipv4"), ("address", "0.0.0.0"), ("port", "8080")]);
        new_listener.processes.push(crate::model::Process {
            pid: 700,
            name: "unrelated-owner".into(),
        });
        new.resources
            .insert("tcp4:0.0.0.0:8080".into(), new_listener);
        let report = analyze(old, new);
        assert_eq!(report.historical_comparisons.removed, 1);
        assert_eq!(report.historical_comparisons.added, 1);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].rule_id, "network.tcp_listener_new");
        assert!(!report
            .findings
            .iter()
            .any(|finding| finding.rule_id.contains("widen")));
    }

    #[test]
    fn process_pid_churn_does_not_create_drift_or_widened_bind_claims() {
        let mut old = collection("Linux /proc/net TCP tables", &["tcp4"]);
        old.scope = Some("same".into());
        let mut old_resource =
            resource_with(&[("family", "ipv4"), ("address", "127.0.0.1"), ("port", "9")]);
        old_resource.processes.push(crate::model::Process {
            pid: 20,
            name: "demo".into(),
        });
        old.resources
            .insert("tcp4:127.0.0.1:9".into(), old_resource);
        let mut new = old.clone();
        new.resources.get_mut("tcp4:127.0.0.1:9").unwrap().processes[0].pid = 40;
        let report = analyze(old, new);
        assert!(report.findings.is_empty());
        assert_eq!(report.historical_comparisons.unchanged, 1);
    }

    #[test]
    fn file_permission_broadening_is_reported_but_narrowing_and_symlinks_are_not() {
        let mut old = collection(
            "symlink_metadata (no final symlink dereference)",
            &["/etc/example"],
        );
        old.resources.insert(
            "/etc/example".into(),
            resource_with(&[
                ("exists", "true"),
                ("type", "file"),
                ("mode", "0600"),
                ("uid", "1"),
                ("gid", "1"),
            ]),
        );
        let mut new = old.clone();
        new.resources
            .get_mut("/etc/example")
            .unwrap()
            .properties
            .insert("mode".into(), "0644".into());
        let report = analyze(old.clone(), new.clone());
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].rule_id, "file.permissions_broadened");
        assert_eq!(report.findings[0].severity, Severity::Low);
        assert!(report.findings[0].explanation.contains("not classify"));

        let mut narrowed = new.clone();
        narrowed
            .resources
            .get_mut("/etc/example")
            .unwrap()
            .properties
            .insert("mode".into(), "0600".into());
        assert!(analyze(new.clone(), narrowed).findings.is_empty());

        let mut symlink_old = old;
        symlink_old
            .resources
            .get_mut("/etc/example")
            .unwrap()
            .properties
            .insert("type".into(), "symlink".into());
        let mut symlink_new = symlink_old.clone();
        symlink_new
            .resources
            .get_mut("/etc/example")
            .unwrap()
            .properties
            .insert("mode".into(), "0777".into());
        assert!(analyze(symlink_old, symlink_new).findings.is_empty());
    }

    #[test]
    fn missing_file_metadata_does_not_create_permission_finding() {
        let mut old = collection(
            "symlink_metadata (no final symlink dereference)",
            &["/etc/example"],
        );
        old.resources.insert(
            "/etc/example".into(),
            resource_with(&[("exists", "true"), ("type", "file"), ("mode", "0600")]),
        );
        let mut new = collection(
            "symlink_metadata (no final symlink dereference)",
            &["/etc/example"],
        );
        new.status = Status::Partial;
        new.fail("permission denied");
        let report = analyze(old, new);
        assert!(report.findings.is_empty());
        assert_eq!(report.historical_comparisons.indeterminate, 1);

        let mut unavailable = collection(
            "symlink_metadata (no final symlink dereference)",
            &["/etc/example"],
        );
        unavailable.resources.clear();
        let mut first_observation = collection(
            "symlink_metadata (no final symlink dereference)",
            &["/etc/example"],
        );
        first_observation.resources.insert(
            "/etc/example".into(),
            resource_with(&[("exists", "true"), ("type", "file"), ("mode", "0644")]),
        );
        let report = analyze(unavailable, first_observation);
        assert!(report.findings.is_empty());
        assert_eq!(report.historical_comparisons.added, 1);
    }

    #[test]
    fn failed_transition_finds_failure_not_inactive_or_oneshot() {
        let source = "systemctl system services";
        let mut old = collection(source, &["system services"]);
        old.resources.insert(
            "demo.service".into(),
            resource_with(&[("active", "active"), ("sub", "running")]),
        );
        let mut failed = old.clone();
        failed
            .resources
            .get_mut("demo.service")
            .unwrap()
            .properties
            .insert("active".into(), "failed".into());
        let report = analyze(old.clone(), failed.clone());
        assert_eq!(report.findings.len(), 1);
        assert_eq!(report.findings[0].rule_id, "systemd.service_failed");
        assert_eq!(report.findings[0].category, Category::Reliability);
        assert_eq!(report.findings[0].severity, Severity::Low);

        let mut inactive = old.clone();
        inactive
            .resources
            .get_mut("demo.service")
            .unwrap()
            .properties
            .insert("active".into(), "inactive".into());
        assert!(analyze(old.clone(), inactive).findings.is_empty());
        assert!(analyze(failed.clone(), failed).findings.is_empty());

        let mut old_oneshot = collection(source, &["system services"]);
        old_oneshot.resources.insert(
            "oneshot.service".into(),
            resource_with(&[("active", "inactive"), ("sub", "dead"), ("type", "oneshot")]),
        );
        let new_oneshot = old_oneshot.clone();
        assert!(analyze(old_oneshot, new_oneshot).findings.is_empty());

        let mut partial = collection(source, &["system services"]);
        partial.status = Status::Partial;
        partial.fail("system manager query incomplete");
        let report = analyze(old, partial);
        assert!(report.findings.is_empty());
        assert_eq!(report.historical_comparisons.indeterminate, 1);
    }

    #[test]
    fn declared_mismatch_requires_supported_matched_generation_and_confirmed_code() {
        let generation = PathBuf::from("/nix/store/example-nixos-system");
        let mut audit = Audit::unavailable(Capability::Supported, ReasonCode::LayoutDetected, "ok");
        audit.generation = Some(Generation {
            booted: generation.clone(),
            activated: Some(generation.clone()),
            units: PathBuf::from("/nix/store/example-units"),
            live_units: Some(PathBuf::from("/nix/store/example-units")),
            evidence: vec![],
        });
        audit.declarations.push(crate::nixos::model::Declaration {
            unit: "demo.service".into(),
            unit_type: "service".into(),
            generation: generation.clone(),
            evidence: PathBuf::from("/nix/store/example/demo.service"),
            fragment: Some(PathBuf::from("/nix/store/example/demo.service")),
            service_type: Some(crate::nixos::model::ServiceType::Simple),
            support: Support::Supported,
            reason_code: ReasonCode::SupportedService,
        });
        audit.comparisons.push(DeclaredComparison {
            unit: "demo.service".into(),
            outcome: Outcome::Mismatch,
            reason_code: ReasonCode::TypeMismatch,
            reason: "declared and runtime Type differ".into(),
            evidence: vec![],
        });
        let report = declared(&audit);
        assert_eq!(report.findings.len(), 1);
        assert_eq!(
            report.findings[0].rule_id,
            "nixos.declared_runtime_mismatch"
        );
        assert_eq!(report.findings[0].category, Category::Informational);

        audit.generation.as_mut().unwrap().activated = Some(PathBuf::from("/different"));
        let incompatible = declared(&audit);
        assert!(incompatible.findings.is_empty());
        assert_eq!(incompatible.declared_comparisons.indeterminate, 1);
        audit.generation.as_mut().unwrap().activated = Some(generation.clone());
        audit.declarations[0].support = Support::Unsupported("not supported".into());
        let unsupported = declared(&audit);
        assert!(unsupported.findings.is_empty());
        assert_eq!(unsupported.declared_comparisons.unsupported, 1);
        audit.declarations[0].support = Support::Supported;
        audit.comparisons[0].reason_code = ReasonCode::ScopeIncompatible;
        let unknown = declared(&audit);
        assert!(unknown.findings.is_empty());
        assert_eq!(unknown.declared_comparisons.indeterminate, 1);
    }

    #[test]
    fn finding_ids_order_and_json_are_deterministic_and_evidence_attributed() {
        let mut old = collection("Linux /proc/net TCP tables", &["tcp4"]);
        old.scope = Some("same".into());
        let mut new = old.clone();
        for id in ["tcp4:127.0.0.1:80", "tcp4:127.0.0.1:81"] {
            new.resources.insert(
                id.into(),
                resource_with(&[
                    ("family", "ipv4"),
                    ("address", "127.0.0.1"),
                    ("port", id.rsplit(':').next().unwrap()),
                ]),
            );
        }
        let first = analyze(old.clone(), new.clone());
        let second = analyze(old, new);
        assert_eq!(first.findings, second.findings);
        assert!(first
            .findings
            .windows(2)
            .all(|pair| pair[0].id < pair[1].id));
        let json = serde_json::to_value(first).unwrap();
        assert_eq!(json["schema_version"], 1);
        assert_eq!(json["findings"][0]["provenance"]["previous_snapshot_id"], 4);
        assert_eq!(json["counts"]["total"], 2);
        assert_eq!(json["counts"]["by_category"]["informational"], 2);
        assert_eq!(json["counts"]["by_severity"]["info"], 2);
        assert!(json["findings"][0]["rule_id"].is_string());
    }
}
