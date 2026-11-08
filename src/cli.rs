use clap::{Parser, Subcommand};
use std::path::PathBuf;
#[derive(Debug, Parser)]
#[command(version, about = "Read-only local Linux historical state auditor")]
pub struct Cli {
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Command,
}
#[derive(Debug, Subcommand)]
pub enum Command {
    Scan {
        /// Disable NixOS declared-state checks; preserve ordinary observations.
        #[arg(long)]
        no_declared: bool,
        /// Emit observation and declared-state results as structured JSON.
        #[arg(long)]
        json: bool,
        /// Evaluate and display evidence-backed findings for this scan.
        #[arg(long)]
        findings: bool,
    },
    Snapshot {
        #[command(subcommand)]
        command: SnapshotCommand,
    },
    Diff {
        #[arg(long, conflicts_with_all = ["from", "to"])]
        latest: bool,
        #[arg(long, requires = "to", required_unless_present = "latest")]
        from: Option<i64>,
        #[arg(long, requires = "from", required_unless_present = "latest")]
        to: Option<i64>,
        /// Evaluate evidence-backed findings from this historical comparison.
        #[arg(long)]
        findings: bool,
        /// Emit a machine-readable historical comparison and findings report.
        #[arg(long, requires = "findings")]
        json: bool,
    },
}
#[derive(Debug, Subcommand)]
pub enum SnapshotCommand {
    Create,
    List,
}
