use clap::Parser;
use drift::{
    cli::Cli,
    collectors::{files, network, systemd},
    diff::{compare, Kind},
    model::{resource, Collection, Snapshot, Status, VERSION},
    storage::sqlite::Store,
};
use std::{
    collections::BTreeMap,
    os::unix::fs::{symlink, PermissionsExt},
};
fn private_tempdir() -> tempfile::TempDir {
    tempfile::Builder::new()
        .permissions(std::fs::Permissions::from_mode(0o700))
        .tempdir()
        .unwrap()
}
fn snapshot(items: &[(&str, &str)]) -> Snapshot {
    let mut c = Collection::new("fixture", vec!["all".into()]);
    for (id, value) in items {
        c.resources
            .insert((*id).into(), resource([("state".into(), (*value).into())]));
    }
    Snapshot {
        version: VERSION,
        timestamp: 1,
        host: "fixture-host".into(),
        collectors: BTreeMap::from([("fixture".into(), c)]),
    }
}
#[test]
fn diff_categories_and_order() {
    let a = snapshot(&[("removed", "x"), ("changed", "a"), ("same", "a")]);
    let mut b = snapshot(&[("same", "a"), ("added", "x"), ("changed", "b")]);
    b.timestamp = 999;
    let d = compare(&a, &b).unwrap();
    assert_eq!(
        d.entries
            .iter()
            .map(|e| (e.id.as_str(), &e.kind))
            .collect::<Vec<_>>(),
        vec![
            ("added", &Kind::Added),
            ("changed", &Kind::Changed),
            ("removed", &Kind::Removed),
            ("same", &Kind::Unchanged)
        ]
    );
    assert_ne!(d.entries[1].previous, d.entries[1].current);
}
#[test]
fn missing_and_incomplete_are_not_removals() {
    let a = snapshot(&[("a", "x")]);
    for status in [Status::Partial, Status::Failed, Status::Disabled] {
        let mut b = snapshot(&[]);
        b.collectors.get_mut("fixture").unwrap().status = status;
        assert_eq!(
            compare(&a, &b).unwrap().entries[0].kind,
            Kind::Indeterminate
        );
    }
    let mut b = snapshot(&[]);
    b.collectors.clear();
    assert_eq!(
        compare(&a, &b).unwrap().entries[0].kind,
        Kind::Indeterminate
    );
    let mut b = snapshot(&[]);
    b.collectors
        .get_mut("fixture")
        .unwrap()
        .coverage
        .push("other".into());
    assert_eq!(
        compare(&a, &b).unwrap().entries[0].kind,
        Kind::Indeterminate
    );
    assert_eq!(
        compare(&snapshot(&[]), &snapshot(&[]))
            .unwrap()
            .entries
            .len(),
        0
    );
}
#[test]
fn incompatible_snapshots_rejected() {
    let a = snapshot(&[]);
    let mut b = a.clone();
    b.host = "other".into();
    assert!(compare(&a, &b).is_err());
    b.host = a.host.clone();
    b.version += 1;
    assert!(compare(&a, &b).is_err());
}
#[test]
fn serialization_and_sqlite_roundtrip() {
    let dir = private_tempdir();
    let path = dir.path().join("private/state.sqlite3");
    let s = snapshot(&[("a", "x")]);
    assert_eq!(
        serde_json::from_str::<Snapshot>(&serde_json::to_string(&s).unwrap()).unwrap(),
        s
    );
    let mut store = Store::open(&path).unwrap();
    let first = store.save(&s).unwrap();
    let second = store.save(&s).unwrap();
    assert!(second > first);
    assert_eq!(store.get(first).unwrap(), s);
    assert_eq!(store.list().unwrap(), vec![(second, 1), (first, 1)]);
    assert!(store.get(999).is_err());
    drop(store);
    assert_eq!(Store::open(&path).unwrap().get(first).unwrap(), s);
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        std::fs::metadata(path.parent().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
}
#[test]
fn incompatible_and_corrupt_databases() {
    let dir = private_tempdir();
    let path = dir.path().join("db");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("PRAGMA user_version=99").unwrap();
    drop(conn);
    assert!(Store::open(&path).is_err());
    std::fs::write(&path, b"not a sqlite database").unwrap();
    assert!(Store::open(&path).is_err());
    let link = dir.path().join("link");
    symlink(&path, &link).unwrap();
    assert!(Store::open(&link).is_err());
}
#[test]
fn metadata_does_not_follow_final_symlink() {
    let dir = private_tempdir();
    let file = dir.path().join("file");
    let link = dir.path().join("link");
    let missing = dir.path().join("missing");
    std::fs::write(&file, b"fixture contents never collected").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o640)).unwrap();
    symlink(&missing, &link).unwrap();
    let c = files::collect(&[file.clone(), link.clone(), missing.clone()]);
    assert_eq!(c.status, Status::Complete);
    assert_eq!(
        c.resources[file.to_str().unwrap()].properties["mode"],
        "0640"
    );
    assert_eq!(
        c.resources[link.to_str().unwrap()].properties["type"],
        "symlink"
    );
    assert_eq!(
        c.resources[missing.to_str().unwrap()].properties["exists"],
        "false"
    );
    let inaccessible = file.join("child");
    let c = files::collect(&[inaccessible]);
    assert_eq!(c.status, Status::Partial);
    assert!(c.resources.is_empty());
}
#[test]
fn listener_identity_normalization_and_fixtures() {
    // proc stores address words in host byte order.
    let hex = format!("{:08X}:1F90", u32::from_ne_bytes([127, 0, 0, 1]));
    let (id, addr, port) = network::endpoint(&hex, false).unwrap();
    assert_eq!(
        (id, addr, port),
        ("tcp4:[127.0.0.1]:8080".into(), "127.0.0.1".into(), 8080)
    );
    assert!(network::endpoint("garbage", true).is_none());
    assert_eq!(
        network::endpoint("00000000000000000000000000000000:0050", true)
            .unwrap()
            .0,
        "tcp6:[::]:80"
    );
    let dir = private_tempdir();
    std::fs::write(
        dir.path().join("tcp"),
        format!("local_address\n0: {hex} 00000000:0000 0A\n"),
    )
    .unwrap();
    std::fs::write(dir.path().join("tcp6"), "local_address\n").unwrap();
    let c = network::collect_at(dir.path());
    assert_eq!(c.status, Status::Complete);
    assert_eq!(c.resources.len(), 1);
    std::fs::write(dir.path().join("tcp"), "local_address\nmalformed\n").unwrap();
    assert_ne!(network::collect_at(dir.path()).status, Status::Complete);
}
#[test]
fn unavailable_collectors_are_explicit() {
    let dir = private_tempdir();
    assert_eq!(network::collect_at(dir.path()).status, Status::Failed);
    assert_eq!(
        systemd::collect_with(dir.path().join("absent").to_str().unwrap()).status,
        Status::Failed
    );
}
#[test]
fn cli_validation() {
    for args in [
        vec!["drift", "scan"],
        vec!["drift", "snapshot", "create"],
        vec!["drift", "snapshot", "list"],
        vec!["drift", "diff", "--latest"],
        vec!["drift", "diff", "--from", "1", "--to", "2"],
    ] {
        assert!(Cli::try_parse_from(args).is_ok());
    }
    for args in [
        vec!["drift", "diff"],
        vec!["drift", "diff", "--from", "1"],
        vec!["drift", "diff", "--latest", "--from", "1", "--to", "2"],
    ] {
        assert!(Cli::try_parse_from(args).is_err());
    }
}
#[test]
fn config_validation() {
    let dir = private_tempdir();
    let path = dir.path().join("config");
    std::fs::write(&path, "paths = ['relative']").unwrap();
    assert!(drift::config::Config::load(Some(&path)).is_err());
    std::fs::write(&path, "unknown = true").unwrap();
    assert!(drift::config::Config::load(Some(&path)).is_err());
    std::fs::write(&path, "paths = ['/etc/passwd']\nnetwork = false").unwrap();
    assert!(!drift::config::Config::load(Some(&path)).unwrap().network);
}

#[test]
fn cli_end_to_end_isolated_state() {
    let dir = private_tempdir();
    let config = dir.path().join("config.toml");
    let database = dir.path().join("state/snapshots.sqlite3");
    let monitored = dir.path().join("monitored");
    std::fs::write(&monitored, b"fixture").unwrap();
    std::fs::write(
        &config,
        format!(
            "systemd = false\nnetwork = false\npaths = [{:?}]\ndatabase = {:?}\n",
            monitored.to_str().unwrap(),
            database.to_str().unwrap()
        ),
    )
    .unwrap();
    let run = |args: &[&str]| {
        let cli = Cli::try_parse_from(
            ["drift", "--config", config.to_str().unwrap()]
                .into_iter()
                .chain(args.iter().copied()),
        )
        .unwrap();
        drift::app::run_with_auditor(
            cli,
            |config| {
                drift::collectors::collect_with_host(config, "0123456789abcdef0123456789abcdef")
            },
            |_, _| drift::nixos::model::Audit::disabled(),
        )
    };
    assert!(run(&["scan"]).is_ok());
    assert!(!database.exists());
    assert!(run(&["diff", "--latest"])
        .unwrap_err()
        .to_string()
        .contains("at least two"));
    std::fs::set_permissions(&monitored, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert!(run(&["snapshot", "create"]).is_ok());
    std::fs::set_permissions(&monitored, std::fs::Permissions::from_mode(0o640)).unwrap();
    assert!(run(&["snapshot", "create"]).is_ok());
    assert_eq!(run(&["snapshot", "list"]).unwrap().lines().count(), 2);
    assert!(run(&["diff", "--latest"])
        .unwrap()
        .contains("Changed files"));
    assert!(run(&["diff", "--from", "1", "--to", "2"])
        .unwrap()
        .contains("Changed files"));
}

#[test]
fn symlink_ancestors_are_incomplete() {
    let dir = private_tempdir();
    let real = dir.path().join("real");
    std::fs::create_dir(&real).unwrap();
    let alias = dir.path().join("alias");
    symlink(&real, &alias).unwrap();
    let c = files::collect(&[alias.join("missing")]);
    assert_eq!(c.status, Status::Partial);
    assert!(c.resources.is_empty());
    assert!(Store::open(&alias.join("db")).is_err());
}

#[test]
fn systemd_successful_and_partial_fixtures() {
    let units = "z.service loaded inactive dead Z\na.service loaded active running A\n";
    let files =
        "a.service enabled enabled\nz.service disabled disabled\nunloaded.service static -\n";
    let c = systemd::collect_outputs(units, Ok(files));
    assert_eq!(c.status, Status::Complete);
    assert_eq!(
        c.resources.keys().map(String::as_str).collect::<Vec<_>>(),
        vec!["a.service", "unloaded.service", "z.service"]
    );
    assert_eq!(c.resources["a.service"].properties["active"], "active");
    assert_eq!(c.resources["a.service"].properties["sub"], "running");
    assert_eq!(
        c.resources["unloaded.service"].properties["unit_file"],
        "static"
    );
    assert_eq!(
        systemd::collect_outputs(units, Err("denied")).status,
        Status::Partial
    );
    assert_eq!(
        systemd::collect_outputs("malformed", Ok("")).status,
        Status::Partial
    );
}

#[test]
fn permission_denied_is_unknown_not_missing() {
    let dir = private_tempdir();
    let path = dir.path().join("denied");
    let c = files::collect_with_metadata(&[path], |_| {
        Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
    });
    assert_eq!(c.status, Status::Partial);
    assert!(c.resources.is_empty());
    assert!(c.errors[0].contains("permission denied"));
}

#[test]
fn rejected_databases_and_sidecars_are_unchanged() {
    use std::os::unix::fs::MetadataExt;
    let dir = private_tempdir();
    for (i, contents) in [b"unrelated text".as_slice(), b"".as_slice()]
        .into_iter()
        .enumerate()
    {
        let path = dir.path().join(format!("file{i}"));
        std::fs::write(&path, contents).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
        let before = std::fs::metadata(&path).unwrap();
        assert!(Store::open(&path).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), contents);
        let after = std::fs::metadata(&path).unwrap();
        assert_eq!(
            (
                before.mode(),
                before.ino(),
                before.mtime(),
                before.mtime_nsec()
            ),
            (after.mode(), after.ino(), after.mtime(), after.mtime_nsec())
        );
    }
    let path = dir.path().join("unrelated.sqlite3");
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch(
        "CREATE TABLE unrelated(value TEXT); INSERT INTO unrelated VALUES ('preserve');",
    )
    .unwrap();
    drop(db);
    let before = std::fs::read(&path).unwrap();
    let mode = std::fs::metadata(&path).unwrap().mode();
    assert!(Store::open(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert_eq!(std::fs::metadata(&path).unwrap().mode(), mode);
    let path = dir.path().join("newdb");
    let sidecar = dir.path().join("newdb-journal");
    std::fs::write(&sidecar, b"unrelated").unwrap();
    assert!(Store::open(&path).is_err());
    assert!(!path.exists());
    assert_eq!(std::fs::read(sidecar).unwrap(), b"unrelated");
}

#[test]
fn marked_database_with_unexpected_schema_is_unchanged() {
    let dir = private_tempdir();
    let path = dir.path().join("db");
    drop(Store::open(&path).unwrap());
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("CREATE TABLE unexpected(value TEXT)")
        .unwrap();
    drop(db);
    let before = std::fs::read(&path).unwrap();
    assert!(Store::open(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

fn network_snapshot(scope: Option<&str>, items: &[(&str, &str)]) -> Snapshot {
    let mut s = snapshot(items);
    let mut c = s.collectors.remove("fixture").unwrap();
    c.scope = scope.map(str::to_owned);
    s.collectors.insert("network".into(), c);
    s
}
#[test]
fn namespace_compatibility() {
    let a = network_snapshot(Some("boot:net1"), &[("old", "a")]);
    let b = network_snapshot(Some("boot:net1"), &[("new", "a")]);
    let d = compare(&a, &b).unwrap();
    assert_eq!(d.entries[0].kind, Kind::Added);
    assert_eq!(d.entries[1].kind, Kind::Removed);
    for scope in [Some("boot:net2"), Some("otherboot:net1"), None] {
        let b = network_snapshot(scope, &[("new", "a")]);
        assert!(compare(&a, &b)
            .unwrap()
            .entries
            .iter()
            .all(|e| e.kind == Kind::Indeterminate));
    }
    let a = network_snapshot(None, &[("old", "a")]);
    let b = network_snapshot(None, &[]);
    assert_eq!(
        compare(&a, &b).unwrap().entries[0].kind,
        Kind::Indeterminate
    );
}
#[test]
fn metadata_and_pid_changes_do_not_create_listener_drift() {
    let a = network_snapshot(Some("boot:net"), &[("endpoint", "a")]);
    let mut b = a.clone();
    b.timestamp += 100;
    let c = b.collectors.get_mut("network").unwrap();
    c.errors.push("incidental diagnostic".into());
    c.resources
        .get_mut("endpoint")
        .unwrap()
        .processes
        .push(drift::model::Process {
            pid: 999,
            name: "fixture".into(),
        });
    assert_eq!(compare(&a, &b).unwrap().entries[0].kind, Kind::Unchanged);
    let mut legacy = serde_json::to_value(&a).unwrap();
    legacy["collectors"]["network"]
        .as_object_mut()
        .unwrap()
        .remove("scope");
    let legacy: Snapshot = serde_json::from_value(legacy).unwrap();
    assert_eq!(
        compare(&a, &legacy).unwrap().entries[0].kind,
        Kind::Indeterminate
    );
}

#[test]
fn process_association_fixture_and_unknown_ownership() {
    let dir = private_tempdir();
    let proc = dir.path();
    for path in ["net", "self/ns", "sys/kernel/random", "42/ns", "42/fd"] {
        std::fs::create_dir_all(proc.join(path)).unwrap();
    }
    std::fs::write(
        proc.join("sys/kernel/random/boot_id"),
        "01234567-89ab-cdef-0123-456789abcdef\n",
    )
    .unwrap();
    std::fs::write(proc.join("self/ns/net"), b"fixture").unwrap();
    std::fs::hard_link(proc.join("self/ns/net"), proc.join("42/ns/net")).unwrap();
    std::fs::write(proc.join("42/comm"), "fixture-server\n").unwrap();
    std::fs::write(
        proc.join("42/stat"),
        format!("42 (fixture server) S {} 12345", vec!["0"; 18].join(" ")),
    )
    .unwrap();
    symlink("socket:[123]", proc.join("42/fd/3")).unwrap();
    let hex = format!("{:08X}:1F90", u32::from_ne_bytes([127, 0, 0, 1]));
    std::fs::write(
        proc.join("net/tcp"),
        format!("local_address\n0: {hex} 00000000:0000 0A 0 0 0 1000 0 123\n"),
    )
    .unwrap();
    std::fs::write(proc.join("net/tcp6"), "local_address\n").unwrap();
    let c = network::collect_proc(proc);
    assert!(c.scope.is_some());
    let processes = &c.resources.values().next().unwrap().processes;
    assert_eq!(
        processes,
        &vec![drift::model::Process {
            pid: 42,
            name: "fixture-server".into()
        }]
    );
    std::fs::remove_file(proc.join("42/stat")).unwrap();
    assert!(network::collect_proc(proc)
        .resources
        .values()
        .next()
        .unwrap()
        .processes
        .is_empty());
    std::fs::remove_file(proc.join("self/ns/net")).unwrap();
    assert!(network::collect_proc(proc).scope.is_none());
}

#[test]
fn legacy_databases_and_invalid_versions_are_preserved() {
    let dir = private_tempdir();
    let legacy = dir.path().join("legacy");
    let db = rusqlite::Connection::open(&legacy).unwrap();
    db.execute_batch("CREATE TABLE snapshots (id INTEGER PRIMARY KEY AUTOINCREMENT, timestamp INTEGER NOT NULL, payload TEXT NOT NULL); PRAGMA user_version=1;").unwrap();
    drop(db);
    let bytes = std::fs::read(&legacy).unwrap();
    assert!(Store::open(&legacy)
        .err()
        .unwrap()
        .to_string()
        .contains("no Drift writes"));
    assert_eq!(std::fs::read(&legacy).unwrap(), bytes);
    let path = dir.path().join("marked");
    drop(Store::open(&path).unwrap());
    let db = rusqlite::Connection::open(&path).unwrap();
    db.execute_batch("PRAGMA user_version=99").unwrap();
    drop(db);
    let bytes = std::fs::read(&path).unwrap();
    assert!(format!("{:#}", Store::open(&path).err().unwrap())
        .contains("incompatible database schema 99"));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn unsafe_links_and_existing_modes_are_preserved() {
    let dir = private_tempdir();
    let target = dir.path().join("target");
    std::fs::write(&target, b"unrelated").unwrap();
    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o644)).unwrap();
    let link = dir.path().join("link");
    symlink(&target, &link).unwrap();
    assert!(Store::open(&link).is_err());
    let hardlink = dir.path().join("hardlink");
    std::fs::hard_link(&target, &hardlink).unwrap();
    assert!(Store::open(&hardlink).is_err());
    assert_eq!(std::fs::read(&target).unwrap(), b"unrelated");
    assert_eq!(
        std::fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o644
    );
    let valid = dir.path().join("valid");
    drop(Store::open(&valid).unwrap());
    std::fs::set_permissions(&valid, std::fs::Permissions::from_mode(0o644)).unwrap();
    let bytes = std::fs::read(&valid).unwrap();
    assert!(Store::open(&valid).is_err());
    assert_eq!(std::fs::read(&valid).unwrap(), bytes);
    assert_eq!(
        std::fs::metadata(&valid).unwrap().permissions().mode() & 0o777,
        0o644
    );
}
