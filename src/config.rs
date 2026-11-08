use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub systemd: bool,
    pub network: bool,
    pub files: bool,
    pub paths: Vec<PathBuf>,
    pub database: Option<PathBuf>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            systemd: true,
            network: true,
            files: true,
            paths: vec![],
            database: None,
        }
    }
}
impl Config {
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let mut c: Self = match path {
            Some(p) => toml::from_str(&std::fs::read_to_string(p).context("read configuration")?)
                .context("parse configuration")?,
            None => Self::default(),
        };
        if c.paths
            .iter()
            .any(|p| !p.is_absolute() || p.to_str().is_none())
        {
            bail!("monitored paths must be absolute UTF-8 paths");
        }
        for path in &mut c.paths {
            if path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
            {
                bail!("monitored paths cannot contain '..'");
            }
            // Normalize separators and '.' without resolving any symlink.
            *path = path.components().collect();
        }
        c.paths.sort();
        c.paths.dedup();
        if c.database.as_ref().is_some_and(|p| !p.is_absolute()) {
            bail!("database path must be absolute");
        }
        Ok(c)
    }
    pub fn database_path(&self) -> Result<PathBuf> {
        if let Some(p) = &self.database {
            return Ok(p.clone());
        }
        if let Some(p) = std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
        {
            return Ok(p.join("drift/snapshots.sqlite3"));
        }
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .context("set an absolute XDG_STATE_HOME, HOME, or database configuration")?;
        Ok(home.join(".local/state/drift/snapshots.sqlite3"))
    }
}
