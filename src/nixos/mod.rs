pub mod compare;
pub mod coverage;
pub mod generation;
pub mod model;
pub mod runtime;
pub mod units;
use crate::{config::Config, model::Snapshot};
use coverage::{Coverage, ReasonCode};
use generation::SystemPaths;
use model::{Audit, Capability};

pub fn audit(config: &Config, snapshot: &Snapshot) -> Audit {
    audit_with(&SystemPaths::default(), config, snapshot, runtime::collect)
}
/// Explicit boundaries keep fixtures independent of the host's generation and manager.
pub fn audit_with(
    paths: &SystemPaths,
    config: &Config,
    snapshot: &Snapshot,
    collect: impl Fn(&[String]) -> model::Runtime,
) -> Audit {
    let generation = match generation::detect(paths) {
        Ok(g) => g,
        Err((capability, code, reason)) => return Audit::unavailable(capability, code, reason),
    };
    let store = match paths.store.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            return Audit::unavailable(
                Capability::Unavailable,
                ReasonCode::StoreUnavailable,
                e.to_string(),
            )
        }
    };
    let (declarations, skipped) = match units::declarations(&generation, &store) {
        Ok(result) => result,
        Err(e) => {
            let mut audit =
                Audit::unavailable(Capability::Unavailable, ReasonCode::UnitsUnavailable, e);
            audit.generation = Some(generation);
            return audit;
        }
    };
    let names: Vec<_> = declarations
        .iter()
        .filter(|d| d.support == model::Support::Supported)
        .map(|d| d.unit.clone())
        .collect();
    let mut diagnostics = vec![];
    let mut diagnostic_codes = vec![];
    let mut runtime = if config.systemd
        && snapshot
            .collectors
            .get("systemd")
            .is_some_and(|c| c.status == crate::model::Status::Complete)
    {
        collect(&names)
    } else {
        Err("supplemental systemd metadata skipped because the systemd collector is disabled or incomplete".into())
    };
    if let Err(reason) = &runtime {
        diagnostics.push(reason.clone());
        diagnostic_codes.push(ReasonCode::RuntimeUnavailable);
    }
    if let Ok(map) = &mut runtime {
        for r in map.values_mut() {
            // Only metadata is read here: runtime fragment content is never trusted
            // as a substitute for booted-generation artifacts.
            r.fragment = match r.fragment.canonicalize() {
                Ok(path) => path,
                Err(error) => {
                    diagnostics.push(format!(
                        "{}: runtime fragment metadata unavailable: {error}",
                        r.id
                    ));
                    diagnostic_codes.push(ReasonCode::FragmentUnavailable);
                    Default::default()
                }
            };
        }
    }
    if !generation.compatible() {
        diagnostic_codes.push(ReasonCode::ScopeIncompatible);
        diagnostics.push("booted generation differs from activated generation/live unit tree, or a reference is unavailable; comparisons are conservative".into());
    }
    let mut comparisons = compare::compare(
        &declarations,
        &generation,
        snapshot.collectors.get("systemd"),
        &runtime,
        &config.nixos_expectations,
    );
    if generation::detect(paths).ok().as_ref() != Some(&generation) {
        diagnostics.push("generation references changed during collection".into());
        diagnostic_codes.push(ReasonCode::GenerationChanged);
        for result in &mut comparisons {
            if result.outcome != model::Outcome::Unsupported {
                result.outcome = model::Outcome::Indeterminate;
                result.reason_code = ReasonCode::GenerationChanged;
                result.reason = "generation references changed during collection".into();
            }
        }
    }
    Audit {
        version: 1,
        capability: Capability::Supported,
        reason_code: ReasonCode::LayoutDetected,
        reason: "limited booted-generation system service artifact audit; enablement does not imply activity".into(),
        generation: Some(generation),
        diagnostics,
        diagnostic_codes,
        declarations,
        comparisons,
        skipped_non_services: skipped,
    }
}
pub fn render(audit: &Audit) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    writeln!(
        out,
        "Declared state: {:?} [{}]: {}",
        audit.capability, audit.reason_code, audit.reason
    )
    .unwrap();
    if let Some(g) = &audit.generation {
        writeln!(
            out,
            "  booted: {}\n  activated: {:?}",
            g.booted.display(),
            g.activated
        )
        .unwrap();
    }
    for diagnostic in &audit.diagnostics {
        writeln!(out, "  {diagnostic}").unwrap();
    }
    writeln!(
        out,
        "  {} service declarations; {} other entries skipped",
        audit.declarations.len(),
        audit.skipped_non_services
    )
    .unwrap();
    let coverage = Coverage::from_audit(audit);
    writeln!(
        out,
        "  scope: {:?}; limitations: {}",
        coverage.scope,
        coverage
            .limitations
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ")
    )
    .unwrap();
    for (label, group) in [
        ("declarations", &coverage.declarations),
        ("comparisons", &coverage.comparisons),
        ("expectation-only", &coverage.expectation_only),
    ] {
        writeln!(
            out,
            "  {label}: {}; status counts: {:?}",
            group.total, group.by_status
        )
        .unwrap();
        for (code, count) in &group.by_reason {
            writeln!(out, "    {code}: {count}").unwrap();
        }
    }
    for result in &audit.comparisons {
        writeln!(
            out,
            "  {:?} {} [{}]: {}\n    evidence: {:?}",
            result.outcome, result.unit, result.reason_code, result.reason, result.evidence
        )
        .unwrap();
    }
    out
}
