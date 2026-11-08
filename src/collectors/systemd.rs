use crate::model::{resource, Collection};
use std::process::Command;
pub fn collect() -> Collection {
    collect_with("systemctl")
}
pub fn collect_with(program: &str) -> Collection {
    let source = "systemctl system services";
    let run = |args: &[&str]| -> Result<String, String> {
        let out = Command::new(program)
            .args(args)
            .env("LC_ALL", "C")
            .env("SYSTEMD_COLORS", "0")
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!("systemctl exited {}", out.status));
        }
        String::from_utf8(out.stdout).map_err(|e| e.to_string())
    };
    let units = match run(&[
        "list-units",
        "--all",
        "--type=service",
        "--plain",
        "--no-legend",
        "--no-pager",
    ]) {
        Ok(s) => s,
        Err(e) => return Collection::unavailable(source, e),
    };
    let unit_files = run(&[
        "list-unit-files",
        "--type=service",
        "--no-legend",
        "--no-pager",
    ]);
    collect_outputs(&units, unit_files.as_deref().map_err(String::as_str))
}
/// Parse controlled command results separately from process invocation.
pub fn collect_outputs(units: &str, unit_files: Result<&str, &str>) -> Collection {
    let source = "systemctl system services";
    let mut c = Collection::new(source, vec!["system services".into()]);
    for line in units.lines().filter(|l| !l.trim().is_empty()) {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() < 4 || !fields[0].ends_with(".service") {
            c.fail("unrecognized service row");
            continue;
        }
        c.resources.insert(
            fields[0].into(),
            resource([
                ("load".into(), fields[1].into()),
                ("active".into(), fields[2].into()),
                ("sub".into(), fields[3].into()),
            ]),
        );
    }
    match unit_files {
        Ok(s) => {
            for line in s.lines().filter(|l| !l.trim().is_empty()) {
                let f: Vec<_> = line.split_whitespace().collect();
                if f.len() < 2 || !f[0].ends_with(".service") {
                    c.fail("unrecognized unit-file row");
                    continue;
                }
                // Unloaded unit files are still stable service identities.
                c.resources
                    .entry(f[0].into())
                    .or_insert_with(|| resource([]))
                    .properties
                    .insert("unit_file".into(), f[1].into());
            }
        }
        Err(e) => c.fail(e),
    }
    c
}
