use super::model::{Audit, Capability, Outcome, Support};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Stable categories describe decisions, never localized OS error strings or paths.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ReasonCode {
    #[default]
    Unknown,
    SupportedService,
    ImplicitSimpleService,
    ImplicitDbusService,
    MaskedUnit,
    ArtifactOutsideStore,
    UnsupportedIdentity,
    ArtifactNaming,
    SocketActivation,
    TimerActivation,
    PathActivation,
    GenerationDropIns,
    ArtifactUnavailable,
    ContinuedDirective,
    UnsupportedSyntax,
    ConditionalSemantics,
    RepeatedType,
    UnsupportedType,
    ImplicitType,
    ScopeIncompatible,
    ObservationIncomplete,
    UnitObservationMissing,
    RuntimeUnavailable,
    RuntimeUnitMissing,
    RuntimeActivation,
    RuntimeDropIns,
    UnitNotLoaded,
    DaemonReloadRequired,
    ObservationChanged,
    FragmentUnavailable,
    FragmentMismatch,
    RuntimeTypeUnavailable,
    RuntimeTypeUnsupported,
    TypeMismatch,
    ExpectationGenerationMismatch,
    TransitionalActivity,
    ActivityMismatch,
    ActivityMatch,
    ArtifactMatch,
    ExpectationDeclarationMissing,
    GenerationChanged,
    LayoutDetected,
    OsDetectionUnavailable,
    NonNixos,
    ContainerUnsupported,
    ContainerDetectionUnavailable,
    StoreUnavailable,
    GenerationUnavailable,
    GenerationMetadataUnavailable,
    UnitsUnavailable,
    Disabled,
}
impl std::fmt::Display for ReasonCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Use the same spelling as Serde rather than a second code-name table.
        let name = serde_json::to_value(self).map_err(|_| std::fmt::Error)?;
        f.write_str(name.as_str().ok_or(std::fmt::Error)?)
    }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Counts {
    pub total: usize,
    pub by_status: BTreeMap<String, usize>,
    pub by_reason: BTreeMap<ReasonCode, usize>,
}
impl Counts {
    fn record(&mut self, status: &str, code: ReasonCode) {
        self.total += 1;
        *self.by_status.entry(status.into()).or_default() += 1;
        *self.by_reason.entry(code).or_default() += 1;
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScopeStatus {
    Compatible,
    Incompatible,
    Unavailable,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Coverage {
    pub scope: ScopeStatus,
    pub declarations: Counts,
    /// One result per declaration. Operator assertions with no declaration are separate.
    pub comparisons: Counts,
    pub expectation_only: Counts,
    pub skipped_non_services: usize,
    pub limitations: BTreeSet<ReasonCode>,
}
impl Coverage {
    pub fn from_audit(audit: &Audit) -> Self {
        let scope = match &audit.generation {
            _ if audit.reason_code == ReasonCode::Unknown => ScopeStatus::Unavailable,
            Some(_)
                if audit
                    .diagnostic_codes
                    .contains(&ReasonCode::GenerationChanged) =>
            {
                ScopeStatus::Unavailable
            }
            Some(g) if g.compatible() => ScopeStatus::Compatible,
            Some(g) if g.activated.is_some() && g.live_units.is_some() => ScopeStatus::Incompatible,
            _ => ScopeStatus::Unavailable,
        };
        let mut c = Self {
            scope,
            declarations: Counts::default(),
            comparisons: Counts::default(),
            expectation_only: Counts::default(),
            skipped_non_services: audit.skipped_non_services,
            limitations: audit.diagnostic_codes.iter().copied().collect(),
        };
        if audit.capability != Capability::Supported || audit.reason_code == ReasonCode::Unknown {
            c.limitations.insert(audit.reason_code);
        }
        let names: BTreeSet<_> = audit.declarations.iter().map(|d| d.unit.as_str()).collect();
        for d in &audit.declarations {
            let status = match d.support {
                Support::Supported => "supported",
                Support::Unsupported(_) => "unsupported",
                Support::Indeterminate(_) => "indeterminate",
            };
            c.declarations.record(status, d.reason_code);
            if matches!(d.support, Support::Indeterminate(_)) {
                c.limitations.insert(d.reason_code);
            }
        }
        for r in &audit.comparisons {
            let status = match r.outcome {
                Outcome::Match => "match",
                Outcome::Mismatch => "mismatch",
                Outcome::Indeterminate => "indeterminate",
                Outcome::Unsupported => "unsupported",
            };
            let group = if names.contains(r.unit.as_str()) {
                &mut c.comparisons
            } else {
                &mut c.expectation_only
            };
            group.record(status, r.reason_code);
            if r.outcome == Outcome::Indeterminate {
                c.limitations.insert(r.reason_code);
            }
        }
        c
    }
}
