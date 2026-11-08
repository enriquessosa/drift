//! Synthetic evidence only; no host collection or database access.
use drift::{
    diff, findings,
    model::{resource, Collection, Snapshot, VERSION},
};
use std::collections::BTreeMap;
fn main() -> anyhow::Result<()> {
    let mut network = Collection::new("fixture TCP", vec!["tcp4".into()]);
    network.scope = Some("fixture-namespace".into());
    let mut files = Collection::new("fixture metadata", vec!["/fixture/file".into()]);
    files.resources.insert(
        "/fixture/file".into(),
        resource([
            ("exists".into(), "true".into()),
            ("type".into(), "file".into()),
            ("mode".into(), "0600".into()),
        ]),
    );
    let mut systemd = Collection::new("fixture systemd", vec!["services".into()]);
    systemd.resources.insert(
        "fixture.service".into(),
        resource([("active".into(), "active".into())]),
    );
    let before = Snapshot {
        version: VERSION,
        timestamp: 1,
        host: "synthetic".into(),
        collectors: BTreeMap::from([
            ("network".into(), network),
            ("files".into(), files),
            ("systemd".into(), systemd),
        ]),
    };
    let mut after = before.clone();
    after.timestamp = 2;
    after
        .collectors
        .get_mut("network")
        .unwrap()
        .resources
        .insert(
            "tcp4:127.0.0.1:8080".into(),
            resource([
                ("family".into(), "ipv4".into()),
                ("address".into(), "127.0.0.1".into()),
                ("port".into(), "8080".into()),
            ]),
        );
    after
        .collectors
        .get_mut("files")
        .unwrap()
        .resources
        .get_mut("/fixture/file")
        .unwrap()
        .properties
        .insert("mode".into(), "0644".into());
    after
        .collectors
        .get_mut("systemd")
        .unwrap()
        .resources
        .get_mut("fixture.service")
        .unwrap()
        .properties
        .insert("active".into(), "failed".into());
    if std::env::args().any(|arg| arg == "--incomplete") {
        for collection in after.collectors.values_mut() {
            collection.fail("synthetic unavailable evidence");
        }
    }
    let comparison = diff::compare(&before, &after)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&findings::historical(&comparison, &before, &after, 1, 2))?
    );
    Ok(())
}
