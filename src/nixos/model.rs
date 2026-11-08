use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    Supported,
    Unavailable,
    Unsupported,
    Disabled,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Generation {
    pub booted: PathBuf,
    pub activated: Option<PathBuf>,
    pub units: PathBuf,
    pub live_units: Option<PathBuf>,
    pub evidence: Vec<PathBuf>,
}
impl Generation {
    pub fn compatible(&self) -> bool {
        self.activated.as_ref() == Some(&self.booted)
            && self.live_units.as_ref() == Some(&self.units)
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ServiceType {
    Simple,
    Exec,
    Notify,
    NotifyReload,
    Dbus,
    Idle,
}
impl ServiceType {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "simple" => Some(Self::Simple),
            "exec" => Some(Self::Exec),
            "notify" => Some(Self::Notify),
            "notify-reload" => Some(Self::NotifyReload),
            "dbus" => Some(Self::Dbus),
            "idle" => Some(Self::Idle),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", content = "reason", rename_all = "snake_case")]
pub enum Support {
    Supported,
    Unsupported(String),
    Indeterminate(String),
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Declaration {
    pub unit: String,
    pub unit_type: String,
    pub generation: PathBuf,
    pub evidence: PathBuf,
    pub fragment: Option<PathBuf>,
    pub service_type: Option<ServiceType>,
    pub support: Support,
}
/// Explicit operator intent, not an inference from enablement or WantedBy.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActiveExpectation {
    pub unit: String,
    pub generation: PathBuf,
    pub active: bool,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeUnit {
    pub id: String,
    pub load: String,
    pub active: String,
    pub sub: String,
    pub service_type: String,
    pub fragment: PathBuf,
    pub drop_ins: String,
    pub reload: String,
    pub triggered_by: String,
}
pub type Runtime = Result<BTreeMap<String, RuntimeUnit>, String>;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Match,
    Mismatch,
    Indeterminate,
    Unsupported,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Comparison {
    pub unit: String,
    pub outcome: Outcome,
    pub reason: String,
    pub evidence: Vec<PathBuf>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Audit {
    pub version: u32,
    pub capability: Capability,
    pub reason: String,
    pub generation: Option<Generation>,
    pub diagnostics: Vec<String>,
    pub declarations: Vec<Declaration>,
    pub comparisons: Vec<Comparison>,
    pub skipped_non_services: usize,
}
impl Audit {
    pub fn unavailable(capability: Capability, reason: impl Into<String>) -> Self {
        Self {
            version: 1,
            capability,
            reason: reason.into(),
            generation: None,
            diagnostics: vec![],
            declarations: vec![],
            comparisons: vec![],
            skipped_non_services: 0,
        }
    }
    pub fn disabled() -> Self {
        Self::unavailable(
            Capability::Disabled,
            "declared-state auditing disabled by --no-declared",
        )
    }
}
