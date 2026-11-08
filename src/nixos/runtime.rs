use super::model::{Runtime, RuntimeUnit};
use std::{collections::BTreeMap, process::Command};
pub fn collect(units: &[String]) -> Runtime {
    if units.is_empty() {
        return Ok(BTreeMap::new());
    }
    const PROPERTIES: &str = "Id,LoadState,ActiveState,SubState,Type,FragmentPath,DropInPaths,NeedDaemonReload,TriggeredBy";
    let out = Command::new("systemctl")
        .args(["show", "--no-pager", "--property", PROPERTIES, "--"])
        .args(units)
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| format!("systemctl show unavailable: {e}"))?;
    if !out.status.success() {
        return Err(format!("systemctl show failed: {}", out.status));
    }
    parse(
        &String::from_utf8(out.stdout)
            .map_err(|_| "systemctl show returned non-UTF-8 properties")?,
    )
}
pub fn parse(text: &str) -> Runtime {
    let mut result = BTreeMap::new();
    for block in text.split("\n\n").filter(|s| !s.trim().is_empty()) {
        let mut fields = BTreeMap::new();
        for line in block.lines().filter(|s| !s.is_empty()) {
            let (key, value) = line.split_once('=').ok_or("malformed systemctl property")?;
            if fields.insert(key, value).is_some() {
                return Err("duplicate systemctl property".into());
            }
        }
        let get = |key| {
            fields
                .get(key)
                .copied()
                .map(str::to_owned)
                .ok_or_else(|| format!("missing systemctl property {key}"))
        };
        let r = RuntimeUnit {
            id: get("Id")?,
            load: get("LoadState")?,
            active: get("ActiveState")?,
            sub: get("SubState")?,
            service_type: get("Type")?,
            fragment: get("FragmentPath")?.into(),
            drop_ins: get("DropInPaths")?,
            reload: get("NeedDaemonReload")?,
            triggered_by: get("TriggeredBy")?,
        };
        if !super::units::valid_service_name(&r.id) {
            return Err("unexpected runtime unit identity".into());
        }
        if let Some(old) = result.insert(r.id.clone(), r.clone()) {
            if old != r {
                return Err("inconsistent duplicate unit observations".into());
            }
        }
    }
    Ok(result)
}
