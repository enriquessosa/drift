use super::model::{Declaration, Generation, ServiceType, Support};
use std::{fs, io::Read, path::Path};

pub fn valid_service_name(name: &str) -> bool {
    name.ends_with(".service")
        && !name.starts_with('-')
        && name.len() <= 255
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.@:-\\".contains(&b))
}
pub fn declarations(
    generation: &Generation,
    store: &Path,
) -> Result<(Vec<Declaration>, usize), String> {
    let entries = fs::read_dir(&generation.units)
        .map_err(|e| format!("cannot enumerate booted units: {e}"))?;
    let mut result = vec![];
    let mut skipped = 0;
    for entry in entries {
        let entry = entry.map_err(|e| format!("incomplete booted unit enumeration: {e}"))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".service") {
            skipped += 1;
            continue;
        }
        let mut declaration = Declaration {
            unit: name.clone(),
            unit_type: "service".into(),
            generation: generation.booted.clone(),
            evidence: entry.path(),
            fragment: None,
            service_type: None,
            support: Support::Supported,
        };
        let classify = || -> Result<(std::path::PathBuf, Option<ServiceType>, Support), String> {
            let fragment = entry
                .path()
                .canonicalize()
                .map_err(|e| format!("unit artifact unavailable: {e}"))?;
            if fragment == Path::new("/dev/null") {
                return Ok((
                    fragment,
                    None,
                    Support::Unsupported("masked unit; no continuous activity expectation".into()),
                ));
            }
            if !fragment.starts_with(store) {
                return Ok((
                    fragment,
                    None,
                    Support::Unsupported("unit artifact outside the generation's store".into()),
                ));
            }
            if !valid_service_name(&name) || name.contains('@') {
                return Ok((
                    fragment,
                    None,
                    Support::Unsupported("template/instance or unsupported unit identity".into()),
                ));
            }
            if fragment.file_name() != Some(entry.file_name().as_os_str()) {
                return Ok((
                    fragment,
                    None,
                    Support::Unsupported(
                        "alias or unsupported artifact naming; canonical unit identity differs"
                            .into(),
                    ),
                ));
            }
            for suffix in ["socket", "timer", "path"] {
                let companion = generation
                    .units
                    .join(name.trim_end_matches("service").to_owned() + suffix);
                match fs::symlink_metadata(&companion) {
                    Ok(_) => {
                        return Ok((
                            fragment,
                            None,
                            Support::Unsupported(format!(
                                "{suffix} activation companion; no continuous activity inference"
                            )),
                        ))
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                    Err(e) => return Err(format!("activation metadata unavailable: {e}")),
                }
            }
            // No drop-ins are merged: this adapter never approximates systemd's
            // override precedence or specifier expansion.
            for name in [format!("{name}.d"), "service.d".into()] {
                match fs::symlink_metadata(generation.units.join(name)) {
                    Ok(_) => {
                        return Ok((
                            fragment,
                            None,
                            Support::Unsupported("generation drop-ins are not interpreted".into()),
                        ))
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                    Err(e) => return Err(format!("drop-in metadata unavailable: {e}")),
                }
            }
            let file =
                fs::File::open(&fragment).map_err(|e| format!("unit artifact unreadable: {e}"))?;
            if !file.metadata().map_err(|e| e.to_string())?.is_file() {
                return Err("unit artifact is not a regular file".into());
            }
            let mut content = String::new();
            file.take(1024 * 1024 + 1)
                .read_to_string(&mut content)
                .map_err(|e| format!("unit artifact unreadable: {e}"))?;
            if content.len() > 1024 * 1024 {
                return Err("unit artifact exceeds supported size".into());
            }
            let (kind, support) = classify_content(&content);
            Ok((fragment, kind, support))
        };
        match classify() {
            Ok((fragment, kind, support)) => {
                declaration.fragment = Some(fragment);
                declaration.service_type = kind;
                declaration.support = support;
            }
            Err(reason) => declaration.support = Support::Indeterminate(reason),
        }
        result.push(declaration);
    }
    result.sort_by(|a, b| a.unit.cmp(&b.unit));
    Ok((result, skipped))
}
pub fn classify_content(content: &str) -> (Option<ServiceType>, Support) {
    let unsupported = |s: &str| (None, Support::Unsupported(s.into()));
    let mut section = "";
    let mut kind = None;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(['#', ';']) {
            continue;
        }
        if line.ends_with('\\') {
            return unsupported("continued directives are not interpreted");
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = &line[1..line.len() - 1];
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return unsupported("unsupported unit syntax");
        };
        let key = key.trim();
        let value = value.trim();
        if section == "Unit"
            && (key.starts_with("Condition")
                || key.starts_with("Assert")
                || key == "StopWhenUnneeded")
            || section == "Service"
                && ["ExecCondition", "RemainAfterExit", "Sockets"].contains(&key)
        {
            return unsupported("conditional, retained, or activation-dependent service semantics");
        }
        if section == "Service" && key == "Type" {
            if kind.is_some() {
                return unsupported("repeated Type directives are not interpreted");
            }
            kind = ServiceType::parse(value);
            if kind.is_none() {
                return unsupported("oneshot, forking, or unknown service Type is outside supported continuous classes");
            }
        }
    }
    if kind.is_none() {
        return unsupported("implicit service Type is not interpreted");
    }
    (kind, Support::Supported)
}
