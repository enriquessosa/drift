use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const VERSION: u32 = 1;
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Status {
    Complete,
    Partial,
    Failed,
    Disabled,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Resource {
    pub properties: BTreeMap<String, String>,
    #[serde(default)]
    pub processes: Vec<Process>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Process {
    pub pid: u32,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Collection {
    pub source: String,
    pub coverage: Vec<String>,
    #[serde(default)]
    pub scope: Option<String>,
    pub status: Status,
    pub errors: Vec<String>,
    pub resources: BTreeMap<String, Resource>,
}
impl Collection {
    pub fn new(source: &str, mut coverage: Vec<String>) -> Self {
        coverage.sort();
        coverage.dedup();
        Self {
            source: source.into(),
            coverage,
            scope: None,
            status: Status::Complete,
            errors: vec![],
            resources: BTreeMap::new(),
        }
    }
    pub fn fail(&mut self, error: impl ToString) {
        self.status = Status::Partial;
        self.errors.push(error.to_string());
    }
    pub fn unavailable(source: &str, error: impl ToString) -> Self {
        let mut c = Self::new(source, vec![]);
        c.status = Status::Failed;
        c.errors.push(error.to_string());
        c
    }
}
pub fn resource(pairs: impl IntoIterator<Item = (String, String)>) -> Resource {
    Resource {
        properties: pairs.into_iter().collect(),
        processes: vec![],
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snapshot {
    pub version: u32,
    pub timestamp: u64,
    pub host: String,
    pub collectors: BTreeMap<String, Collection>,
}
