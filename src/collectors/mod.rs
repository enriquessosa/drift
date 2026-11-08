pub mod files;
pub mod network;
pub mod systemd;
use crate::{
    config::Config,
    model::{Collection, Snapshot, Status, VERSION},
};
use anyhow::{Context, Result};
use std::{
    collections::BTreeMap,
    time::{SystemTime, UNIX_EPOCH},
};
pub fn collect(config: &Config) -> Result<Snapshot> {
    let host = std::fs::read_to_string("/etc/machine-id")
        .context("read host identity /etc/machine-id")?
        .trim()
        .to_owned();
    collect_with_host(config, &host)
}
/// The caller supplies an identity so orchestration can be tested with fixtures.
pub fn collect_with_host(config: &Config, host: &str) -> Result<Snapshot> {
    if host.len() != 32 || !host.bytes().all(|b| b.is_ascii_hexdigit()) {
        anyhow::bail!("invalid machine-id; cannot establish local host identity");
    }
    let mut collectors = BTreeMap::new();
    for (name, enabled, collect) in [
        (
            "systemd",
            config.systemd,
            systemd::collect as fn() -> Collection,
        ),
        ("network", config.network, network::collect),
    ] {
        let c = if enabled { collect() } else { disabled() };
        collectors.insert(name.into(), c);
    }
    collectors.insert(
        "files".into(),
        if config.files {
            files::collect(&config.paths)
        } else {
            disabled()
        },
    );
    Ok(Snapshot {
        version: VERSION,
        timestamp: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        host: host.to_owned(),
        collectors,
    })
}
fn disabled() -> Collection {
    let mut c = Collection::new("disabled", vec![]);
    c.status = Status::Disabled;
    c
}
