use crate::model::{Snapshot, VERSION};
use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection, OpenFlags};
use std::{
    fs::{self, File, OpenOptions},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::Path,
};
const DATABASE_VERSION: u32 = 1;
const APPLICATION_ID: u32 = 0x44524654; // DRFT
const SCHEMA: &str = "CREATE TABLE snapshots (id INTEGER PRIMARY KEY AUTOINCREMENT, timestamp INTEGER NOT NULL, payload TEXT NOT NULL)";

pub struct Store(Connection);
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if !path.is_absolute()
            || path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            bail!("database path must be absolute and cannot contain '..'");
        }
        let parent = path
            .parent()
            .context("database requires parent directory")?;
        private_directory(parent)?;
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY)
            .open(parent)?;
        let owner = fs::metadata("/proc/self")?.uid();
        let metadata = directory.metadata()?;
        if metadata.uid() != owner || metadata.mode() & 0o077 != 0 {
            bail!("database parent must be owned by the current user and private (0700)");
        }
        // Anchor operations to the held directory, rather than re-resolving its ancestors.
        let anchored = std::path::PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd()))
            .join(path.file_name().context("database requires a filename")?);
        for suffix in ["-journal", "-wal", "-shm"] {
            let mut sidecar = anchored.as_os_str().to_os_string();
            sidecar.push(suffix);
            match fs::symlink_metadata(&sidecar) {
                Ok(_) => bail!(
                    "database sidecar exists ({suffix}); inspect/recover trusted state separately"
                ),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(e.into()),
            }
        }
        let (file, created) = match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&anchored)
        {
            Ok(file) => (file, true),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let existing = fs::symlink_metadata(&anchored)?;
                if !existing.is_file() || existing.nlink() != 1 || existing.uid() != owner {
                    bail!("existing database must be a regular, singly linked file owned by the current user");
                }
                (
                    OpenOptions::new()
                        .read(true)
                        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
                        .open(&anchored)
                        .context("open existing database without following symlinks")?,
                    false,
                )
            }
            Err(e) => return Err(e.into()),
        };
        let m = file.metadata()?;
        if !m.is_file() || m.nlink() != 1 || m.uid() != owner {
            bail!("database must be a regular, singly linked file owned by the current user");
        }
        if !created {
            // Immutable read-only validation cannot recover journals or create WAL sidecars.
            // This is SQLite access, not a claim that arbitrary DBs can be validated without opening them.
            let uri = format!("file:/proc/self/fd/{}?immutable=1", file.as_raw_fd());
            let validation = Connection::open_with_flags(
                uri,
                OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
            )?;
            validate(&validation).context("reject existing database; no Drift writes performed")?;
            if m.mode() & 0o077 != 0 {
                bail!("existing Drift database must already be private (0600); permissions were not changed");
            }
        }
        same_file(&file, &anchored)?;
        // SQLite resolves the intentional /proc/self/fd directory link itself.
        // Our initial file open rejects final symlinks and both inode checks
        // detect replacement. The private directory's owner is a trust boundary.
        let mut conn = Connection::open_with_flags(&anchored, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
        same_file(&file, &anchored)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        if created {
            let tx = conn.transaction()?;
            tx.execute_batch(SCHEMA)?;
            tx.pragma_update(None, "application_id", APPLICATION_ID)?;
            tx.pragma_update(None, "user_version", DATABASE_VERSION)?;
            tx.commit()?;
        } else {
            validate(&conn)?;
        }
        Ok(Self(conn))
    }
    pub fn save(&mut self, snapshot: &Snapshot) -> Result<i64> {
        if snapshot.version != VERSION {
            bail!("incompatible observation version");
        }
        let tx = self.0.transaction()?;
        tx.execute(
            "INSERT INTO snapshots(timestamp,payload) VALUES (?1,?2)",
            params![snapshot.timestamp, serde_json::to_string(snapshot)?],
        )?;
        let id = tx.last_insert_rowid();
        tx.commit()?;
        Ok(id)
    }
    pub fn list(&self) -> Result<Vec<(i64, u64)>> {
        let mut stmt = self
            .0
            .prepare("SELECT id,timestamp FROM snapshots ORDER BY id DESC")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
    pub fn get(&self, id: i64) -> Result<Snapshot> {
        let payload: String = self
            .0
            .query_row("SELECT payload FROM snapshots WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .with_context(|| format!("snapshot {id} not found or unreadable"))?;
        let s: Snapshot = serde_json::from_str(&payload).context("invalid snapshot payload")?;
        if s.version != VERSION {
            bail!("incompatible observation version");
        }
        Ok(s)
    }
}
fn validate(conn: &Connection) -> Result<()> {
    let id: u32 = conn.pragma_query_value(None, "application_id", |r| r.get(0))?;
    if id != APPLICATION_ID {
        bail!("not an identified Drift database (legacy unmarked databases are not automatically adopted)");
    }
    let version: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version != DATABASE_VERSION {
        bail!("incompatible database schema {version}");
    }
    let mut stmt = conn.prepare(
        "SELECT name, sql FROM sqlite_master WHERE name != 'sqlite_sequence' ORDER BY name",
    )?;
    let entries = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if entries != vec![("snapshots".into(), SCHEMA.into())] {
        bail!("unexpected Drift database schema");
    }
    let check: String = conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
    if check != "ok" {
        bail!("corrupt Drift database: {check}");
    }
    Ok(())
}
fn same_file(file: &File, path: &Path) -> Result<()> {
    let a = file.metadata()?;
    let b = fs::symlink_metadata(path)?;
    if a.dev() != b.dev() || a.ino() != b.ino() || b.nlink() != 1 {
        bail!("database changed during open");
    }
    Ok(())
}
fn private_directory(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        private_directory(parent)?;
    }
    match fs::symlink_metadata(path) {
        Ok(m) if !m.is_dir() || m.file_type().is_symlink() => {
            bail!("state parent must be a directory, not a symlink")
        }
        Ok(m) if m.mode() & 0o022 != 0 && m.mode() & 0o1000 == 0 => {
            bail!(
                "state ancestor is writable by other users without sticky protection: {}",
                path.display()
            );
        }
        Ok(_) => (),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            use std::os::unix::fs::DirBuilderExt;
            fs::DirBuilder::new().mode(0o700).create(path)?;
        }
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacement_is_detected_without_touching_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("db");
        fs::write(&path, b"original").unwrap();
        let original = File::open(&path).unwrap();
        fs::rename(&path, dir.path().join("old")).unwrap();
        fs::write(&path, b"replacement").unwrap();
        assert!(same_file(&original, &path).is_err());
        assert_eq!(fs::read(&path).unwrap(), b"replacement");
    }
}
