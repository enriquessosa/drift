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
    let mut output = String::new();
    let config = Config::load(cli.config.as_deref())?;
    if matches!(cli.command, Command::Scan) {
        return Ok(report::scan(&collect(&config)?));
    }
    let mut store = Store::open(&config.database_path()?)?;
    match cli.command {
        Command::Scan => unreachable!(),
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
