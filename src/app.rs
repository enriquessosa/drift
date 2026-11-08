use crate::{
    cli::{Cli, Command, SnapshotCommand},
    config::Config,
    diff, report,
    storage::sqlite::Store,
};
use anyhow::{bail, Result};
pub fn run(
    cli: Cli,
    collect: impl Fn(&Config) -> Result<crate::model::Snapshot>,
) -> Result<String> {
    run_with_auditor(cli, collect, crate::nixos::audit)
}
pub fn run_with_auditor(
    cli: Cli,
    collect: impl Fn(&Config) -> Result<crate::model::Snapshot>,
    audit: impl Fn(&Config, &crate::model::Snapshot) -> crate::nixos::model::Audit,
) -> Result<String> {
    let mut output = String::new();
    let config = Config::load(cli.config.as_deref())?;
    if let Command::Scan { no_declared, json } = cli.command {
        let snapshot = collect(&config)?;
        let declared = if no_declared {
            crate::nixos::model::Audit::disabled()
        } else {
            audit(&config, &snapshot)
        };
        return if json {
            Ok(serde_json::to_string_pretty(
                &serde_json::json!({"observation":snapshot,"declared":declared}),
            )? + "\n")
        } else {
            Ok(report::scan(&snapshot) + &crate::nixos::render(&declared))
        };
    }
    let mut store = Store::open(&config.database_path()?)?;
    match cli.command {
        Command::Scan { .. } => unreachable!(),
        Command::Snapshot {
            command: SnapshotCommand::Create,
        } => {
            let s = collect(&config)?;
            let id = store.save(&s)?;
            output.push_str(&report::scan(&s));
            output.push_str(&format!("Saved snapshot {id}\n"));
        }
        Command::Snapshot {
            command: SnapshotCommand::List,
        } => {
            for (id, timestamp) in store.list()? {
                output.push_str(&format!("{id}\t{timestamp} (Unix seconds)\n"));
            }
        }
        Command::Diff { latest, from, to } => {
            let (from, to) = if latest {
                let ids = store.list()?;
                if ids.len() < 2 {
                    bail!("diff --latest requires at least two snapshots");
                }
                (ids[1].0, ids[0].0)
            } else {
                (
                    from.expect("validated by Clap"),
                    to.expect("validated by Clap"),
                )
            };
            output.push_str(&format!("Comparing snapshot {from} -> {to}\n"));
            output.push_str(&report::diff(&diff::compare(
                &store.get(from)?,
                &store.get(to)?,
            )?));
        }
    }
    Ok(output)
}
