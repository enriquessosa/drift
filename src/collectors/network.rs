use crate::model::{resource, Collection, Process};
use std::{collections::BTreeMap, fs, os::unix::fs::MetadataExt};
use std::{
    net::{Ipv4Addr, Ipv6Addr},
    path::Path,
};
pub fn collect() -> Collection {
    collect_proc(Path::new("/proc"))
}
pub fn collect_proc(proc_root: &Path) -> Collection {
    let before = namespace(proc_root);
    let mut c = collect_at(&proc_root.join("net"));
    c.scope = before.clone();
    if before.is_none() {
        c.fail("network namespace identity unavailable; comparisons indeterminate");
    }
    if let Some(scope) = before {
        let sockets = socket_identities(&proc_root.join("net"));
        let associations = associate(proc_root, &scope, &sockets);
        // Recheck the table and namespace after walking proc: do not retain an
        // association when its endpoint/inode mapping has changed during collection.
        let after = socket_identities(&proc_root.join("net"));
        for (inode, processes) in associations {
            if let Some(id) = sockets
                .get(&inode)
                .filter(|id| after.get(&inode) == Some(*id))
            {
                if let Some(resource) = c.resources.get_mut(id) {
                    resource.processes.extend(processes);
                }
            }
        }
        for resource in c.resources.values_mut() {
            resource.processes.sort();
            resource.processes.dedup();
        }
        if namespace(proc_root).as_ref() != Some(&scope) {
            c.scope = None;
            for resource in c.resources.values_mut() {
                resource.processes.clear();
            }
            c.fail("network namespace changed during collection");
        }
    }
    c
}
fn namespace(proc_root: &Path) -> Option<String> {
    let boot = fs::read_to_string(proc_root.join("sys/kernel/random/boot_id")).ok()?;
    let boot = boot.trim();
    if boot.len() != 36 || !boot.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') {
        return None;
    }
    let m = fs::metadata(proc_root.join("self/ns/net")).ok()?;
    Some(format!("{boot}:{}:{}", m.dev(), m.ino()))
}
fn socket_identities(root: &Path) -> BTreeMap<u64, String> {
    let mut result = BTreeMap::new();
    for (file, v6) in [("tcp", false), ("tcp6", true)] {
        if let Ok(s) = fs::read_to_string(root.join(file)) {
            for line in s.lines().skip(1) {
                let fields: Vec<_> = line.split_whitespace().collect();
                if fields.len() > 9 && fields[3] == "0A" {
                    if let (Ok(inode), Some((id, _, _))) =
                        (fields[9].parse::<u64>(), endpoint(fields[1], v6))
                    {
                        if inode != 0 {
                            result.insert(inode, id);
                        }
                    }
                }
            }
        }
    }
    result
}
fn process_start(path: &Path) -> Option<String> {
    // stat contains no command arguments or environment. Only the start-time
    // field is used transiently to reject PID reuse; it is never persisted.
    let stat = fs::read_to_string(path.join("stat")).ok()?;
    stat.rsplit_once(')')?
        .1
        .split_whitespace()
        .nth(19)
        .map(str::to_owned)
}
fn associate(
    proc_root: &Path,
    scope: &str,
    sockets: &BTreeMap<u64, String>,
) -> BTreeMap<u64, Vec<Process>> {
    let mut result: BTreeMap<u64, Vec<Process>> = BTreeMap::new();
    let Some((boot, _)) = scope.split_once(':') else {
        return result;
    };
    let Ok(entries) = fs::read_dir(proc_root) else {
        return result;
    };
    for entry in entries.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let path = entry.path();
        let Some(start) = process_start(&path) else {
            continue;
        };
        let Ok(ns) = fs::metadata(path.join("ns/net")) else {
            continue;
        };
        if format!("{boot}:{}:{}", ns.dev(), ns.ino()) != scope {
            continue;
        }
        let Ok(name) = fs::read_to_string(path.join("comm")) else {
            continue;
        };
        let name = name.trim_end_matches('\n').to_owned();
        let Ok(fds) = fs::read_dir(path.join("fd")) else {
            continue;
        };
        let mut found = Vec::new();
        for fd in fds.flatten() {
            let Ok(link) = fs::read_link(fd.path()) else {
                continue;
            };
            let Some(inode) = link
                .to_str()
                .and_then(|s| s.strip_prefix("socket:["))
                .and_then(|s| s.strip_suffix(']'))
                .and_then(|s| s.parse::<u64>().ok())
            else {
                continue;
            };
            if sockets.contains_key(&inode) && fs::read_link(fd.path()).ok().as_ref() == Some(&link)
            {
                found.push(inode);
            }
        }
        if process_start(&path).as_ref() != Some(&start)
            || fs::metadata(path.join("ns/net"))
                .ok()
                .map(|m| (m.dev(), m.ino()))
                != Some((ns.dev(), ns.ino()))
        {
            continue;
        }
        for inode in found {
            result.entry(inode).or_default().push(Process {
                pid,
                name: name.clone(),
            });
        }
    }
    result
}
pub fn collect_at(root: &Path) -> Collection {
    let mut c = Collection::new(
        "Linux /proc/net TCP tables (current network namespace)",
        vec!["tcp4".into(), "tcp6".into()],
    );
    for (file, v6) in [("tcp", false), ("tcp6", true)] {
        match std::fs::read_to_string(root.join(file)) {
            Ok(s) => {
                if !s
                    .lines()
                    .next()
                    .is_some_and(|header| header.contains("local_address"))
                {
                    c.fail(format!("missing or invalid {file} header"));
                    continue;
                }
                for line in s.lines().skip(1) {
                    let f: Vec<_> = line.split_whitespace().collect();
                    if f.len() < 4 {
                        c.fail(format!("malformed {file} row"));
                        continue;
                    }
                    if f[3] != "0A" {
                        continue;
                    }
                    match endpoint(f[1], v6) {
                        Some((id, address, port)) => {
                            c.resources.insert(
                                id,
                                resource([
                                    ("family".into(), if v6 { "ipv6" } else { "ipv4" }.into()),
                                    ("address".into(), address),
                                    ("port".into(), port.to_string()),
                                ]),
                            );
                        }
                        None => c.fail(format!("invalid {file} endpoint")),
                    }
                }
            }
            Err(e) => c.fail(format!("{file}: {e}")),
        }
    }
    if c.resources.is_empty() && !c.errors.is_empty() {
        c.status = crate::model::Status::Failed;
    }
    c
}
pub fn endpoint(value: &str, v6: bool) -> Option<(String, String, u16)> {
    let (hex, port) = value.split_once(':')?;
    let port = u16::from_str_radix(port, 16).ok()?;
    let address = if v6 {
        if hex.len() != 32 || !hex.is_ascii() {
            return None;
        }
        let mut bytes = [0u8; 16];
        for (i, chunk) in bytes.chunks_mut(4).enumerate() {
            chunk.copy_from_slice(
                &u32::from_str_radix(&hex[i * 8..i * 8 + 8], 16)
                    .ok()?
                    .to_ne_bytes(),
            );
        }
        Ipv6Addr::from(bytes).to_string()
    } else {
        if hex.len() != 8 {
            return None;
        }
        Ipv4Addr::from(u32::from_str_radix(hex, 16).ok()?.to_ne_bytes()).to_string()
    };
    Some((
        format!("tcp{}:[{address}]:{port}", if v6 { "6" } else { "4" }),
        address,
        port,
    ))
}
