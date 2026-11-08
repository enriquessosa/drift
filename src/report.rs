use crate::{
    diff::{Comparison, Kind},
    model::Snapshot,
};
use std::fmt::Write;
pub fn scan(s: &Snapshot) -> String {
    let mut output = String::new();
    writeln!(
        output,
        "Snapshot observation v{} at {} (Unix seconds)",
        s.version, s.timestamp
    )
    .expect("writing to String cannot fail");
    for (name, c) in &s.collectors {
        writeln!(
            output,
            "{name}: {:?}, {} resources; source: {}",
            c.status,
            c.resources.len(),
            c.source
        )
        .expect("writing to String cannot fail");
        if name == "network" {
            writeln!(output, "  namespace: {:?}", c.scope).expect("writing to String cannot fail");
            for (id, r) in &c.resources {
                writeln!(
                    output,
                    "  {id}; observed processes: {:?} (empty means unknown)",
                    r.processes
                )
                .expect("writing to String cannot fail");
            }
        }
        for e in &c.errors {
            writeln!(output, "  unavailable evidence: {e}").expect("writing to String cannot fail");
        }
    }
    output
}
pub fn diff(d: &Comparison) -> String {
    let mut output = String::new();
    for outcome in &d.outcomes {
        writeln!(output, "{outcome}").expect("writing to String cannot fail");
    }
    for e in &d.entries {
        writeln!(output, "{:?} {} {}", e.kind, e.collector, e.id)
            .expect("writing to String cannot fail");
        if e.collector == "network" {
            writeln!(
                output,
                "  observed processes: {:?} (empty means unknown)",
                e.current.as_ref().map(|r| &r.processes)
            )
            .expect("writing to String cannot fail");
        }
        if e.kind != Kind::Unchanged {
            writeln!(
                output,
                "  previous: {:?}\n  current: {:?}",
                e.previous.as_ref().map(|r| &r.properties),
                e.current.as_ref().map(|r| &r.properties)
            )
            .expect("writing to String cannot fail");
        }
    }
    writeln!(
        output,
        "{} resource comparisons; changes carry no risk severity",
        d.entries.len()
    )
    .expect("writing to String cannot fail");
    output
}
