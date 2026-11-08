use super::{coverage::ReasonCode, model::*};
use crate::model::{Collection, Status};

/// Pure declared/observed comparison. Resolved paths and systemctl properties
/// are supplied as evidence; no I/O or historical snapshot semantics live here.
pub fn compare(
    declarations: &[Declaration],
    generation: &Generation,
    observed: Option<&Collection>,
    runtime: &Runtime,
    expectations: &[ActiveExpectation],
) -> Vec<Comparison> {
    let mut results = Vec::new();
    for d in declarations {
        let mut evidence = vec![d.evidence.clone()];
        let (outcome, reason_code, reason) = match &d.support {
            Support::Unsupported(reason) => (Outcome::Unsupported, d.reason_code, reason.clone()),
            Support::Indeterminate(reason) => {
                (Outcome::Indeterminate, d.reason_code, reason.clone())
            }
            Support::Supported => {
                let evaluate = || -> (Outcome, ReasonCode, String) {
                    if d.generation != generation.booted || !generation.compatible() {
                        return (Outcome::Indeterminate,ReasonCode::ScopeIncompatible,"booted, activated, and live unit scopes do not agree or are unavailable".into());
                    }
                    let Some(c) = observed.filter(|c| {
                        c.status == Status::Complete
                            && c.source == "systemctl system services"
                            && c.coverage == ["system services"]
                    }) else {
                        return (Outcome::Indeterminate,ReasonCode::ObservationIncomplete,"systemd observation collector is missing, disabled, incomplete, or has an incompatible scope".into());
                    };
                    let Some(resource) = c.resources.get(&d.unit) else {
                        return (Outcome::Indeterminate,ReasonCode::UnitObservationMissing,"runtime unit observation unavailable; absence is not proof of a failed declaration".into());
                    };
                    let map = match runtime {
                        Ok(map) => map,
                        Err(reason) => {
                            return (
                                Outcome::Indeterminate,
                                ReasonCode::RuntimeUnavailable,
                                reason.clone(),
                            )
                        }
                    };
                    let Some(r) = map.get(&d.unit) else {
                        return (
                            Outcome::Indeterminate,
                            ReasonCode::RuntimeUnitMissing,
                            "structured unit properties unavailable (including possible alias)"
                                .into(),
                        );
                    };
                    if !r.drop_ins.is_empty() {
                        return (
                            Outcome::Unsupported,
                            ReasonCode::RuntimeDropIns,
                            "runtime drop-ins are not interpreted".into(),
                        );
                    }
                    if !r.triggered_by.is_empty() {
                        return (
                            Outcome::Unsupported,
                            ReasonCode::RuntimeActivation,
                            "runtime activation triggers are not interpreted".into(),
                        );
                    }
                    if r.reload != "no" {
                        return (
                            Outcome::Indeterminate,
                            ReasonCode::DaemonReloadRequired,
                            "manager reload state is pending or unavailable".into(),
                        );
                    }
                    if r.load != "loaded" {
                        return (
                            Outcome::Indeterminate,
                            ReasonCode::UnitNotLoaded,
                            "unit is not loaded".into(),
                        );
                    }
                    if [("load", &r.load), ("active", &r.active), ("sub", &r.sub)]
                        .iter()
                        .any(|(key, value)| resource.properties.get(*key) != Some(value))
                    {
                        return (Outcome::Indeterminate,ReasonCode::ObservationChanged,"systemd observations changed between collections or lack runtime state".into());
                    }
                    if r.fragment.as_os_str().is_empty() || d.fragment.is_none() {
                        return (
                            Outcome::Indeterminate,
                            ReasonCode::FragmentUnavailable,
                            "resolved runtime/declaration fragment evidence unavailable".into(),
                        );
                    }
                    if d.fragment.as_ref() != Some(&r.fragment) {
                        return (Outcome::Mismatch,ReasonCode::FragmentMismatch,"loaded unit fragment differs from the booted generation's declaration; no activity or risk inference".into());
                    }
                    if r.service_type.is_empty() {
                        return (
                            Outcome::Indeterminate,
                            ReasonCode::RuntimeTypeUnavailable,
                            "runtime service Type is unavailable".into(),
                        );
                    }
                    if ![
                        "simple",
                        "exec",
                        "notify",
                        "notify-reload",
                        "dbus",
                        "idle",
                        "oneshot",
                        "forking",
                    ]
                    .contains(&r.service_type.as_str())
                    {
                        return (
                            Outcome::Unsupported,
                            ReasonCode::RuntimeTypeUnsupported,
                            "runtime service Type is not interpreted".into(),
                        );
                    }
                    if ServiceType::parse(&r.service_type) != d.service_type {
                        return (
                            Outcome::Mismatch,
                            ReasonCode::TypeMismatch,
                            "loaded service Type differs from the supported declaration".into(),
                        );
                    }
                    if let Some(expected) = expectations.iter().find(|e| e.unit == d.unit) {
                        if expected.generation != generation.booted {
                            return (
                                Outcome::Indeterminate,
                                ReasonCode::ExpectationGenerationMismatch,
                                "operator expectation is pinned to a different generation".into(),
                            );
                        }
                        let actual = match r.active.as_str() {
                            "active" => true,
                            "inactive" | "failed" => false,
                            _ => return (
                                Outcome::Indeterminate,
                                ReasonCode::TransitionalActivity,
                                "service is transitioning; activity comparison is indeterminate"
                                    .into(),
                            ),
                        };
                        if actual != expected.active {
                            return (Outcome::Mismatch,ReasonCode::ActivityMismatch,format!("explicit generation-pinned operator activity expectation: expected {}, observed {}",expected.active,r.active));
                        }
                        return (Outcome::Match,ReasonCode::ActivityMatch,"unit artifact, Type, and explicit generation-pinned activity expectation agree".into());
                    }
                    (Outcome::Match,ReasonCode::ArtifactMatch,"unit artifact and Type agree; activity is informational, with no continuous activity expectation".into())
                };
                evaluate()
            }
        };
        if let Ok(map) = runtime {
            if let Some(r) = map.get(&d.unit) {
                evidence.push(r.fragment.clone());
            }
        }
        evidence.sort();
        evidence.dedup();
        results.push(Comparison {
            unit: d.unit.clone(),
            outcome,
            reason_code,
            reason,
            evidence,
        });
    }
    for expected in expectations
        .iter()
        .filter(|e| !declarations.iter().any(|d| d.unit == e.unit))
    {
        results.push(Comparison {
            unit: expected.unit.clone(),
            outcome: Outcome::Indeterminate,
            reason_code: ReasonCode::ExpectationDeclarationMissing,
            reason: "operator expectation has no supported booted-generation declaration".into(),
            evidence: vec![expected.generation.clone()],
        });
    }
    results.sort_by(|a, b| a.unit.cmp(&b.unit));
    results
}
