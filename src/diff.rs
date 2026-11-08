use crate::model::{Resource, Snapshot, Status, VERSION};
use anyhow::{bail, Result};
use serde::Serialize;
use std::collections::BTreeSet;
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Added,
    Removed,
    Changed,
    Unchanged,
    Indeterminate,
}
#[derive(Debug, Serialize)]
pub struct Entry {
    pub collector: String,
    pub id: String,
    pub kind: Kind,
    pub previous: Option<Resource>,
    pub current: Option<Resource>,
}
#[derive(Debug, Serialize)]
pub struct Comparison {
    pub entries: Vec<Entry>,
    pub outcomes: Vec<String>,
}
pub fn compare(a: &Snapshot, b: &Snapshot) -> Result<Comparison> {
    if a.version != VERSION || b.version != VERSION {
        bail!("incompatible observation version");
    }
    if a.host != b.host {
        bail!("snapshots belong to different hosts");
    }
    let mut result = Comparison {
        entries: vec![],
        outcomes: vec![],
    };
    for name in a
        .collectors
        .keys()
        .chain(b.collectors.keys())
        .collect::<BTreeSet<_>>()
    {
        let old = a.collectors.get(name);
        let new = b.collectors.get(name);
        let comparable = matches!((old, new), (Some(x), Some(y)) if x.status == Status::Complete && y.status == Status::Complete && x.coverage == y.coverage && x.source == y.source && (name != "network" || (x.scope.is_some() && x.scope == y.scope)));
        result.outcomes.push(format!(
            "{name}: {:?} -> {:?}; coverage {}",
            old.map(|c| &c.status),
            new.map(|c| &c.status),
            if comparable {
                "complete and comparable"
            } else {
                "indeterminate"
            }
        ));
        if name == "network" {
            result.outcomes.push(format!(
                "network namespace: {:?} -> {:?}",
                old.and_then(|c| c.scope.as_ref()),
                new.and_then(|c| c.scope.as_ref())
            ));
        }
        for (label, collection) in [("previous", old), ("current", new)] {
            if let Some(c) = collection {
                for error in &c.errors {
                    result.outcomes.push(format!("{name} {label}: {error}"));
                }
            }
        }
        for id in old
            .into_iter()
            .flat_map(|c| c.resources.keys())
            .chain(new.into_iter().flat_map(|c| c.resources.keys()))
            .collect::<BTreeSet<_>>()
        {
            let previous = old.and_then(|c| c.resources.get(id)).cloned();
            let current = new.and_then(|c| c.resources.get(id)).cloned();
            let kind = match (&previous, &current) {
                _ if !comparable => Kind::Indeterminate,
                (Some(x), Some(y)) if x.properties == y.properties => Kind::Unchanged,
                (Some(_), Some(_)) => Kind::Changed,
                (None, Some(_)) if comparable => Kind::Added,
                (Some(_), None) if comparable => Kind::Removed,
                _ => Kind::Indeterminate,
            };
            result.entries.push(Entry {
                collector: name.clone(),
                id: id.clone(),
                kind,
                previous,
                current,
            });
        }
    }
    Ok(result)
}
