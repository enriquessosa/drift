use super::{
    coverage::ReasonCode,
    model::{Declaration, Generation, ServiceType, Support},
};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

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
        let mut d = Declaration {
            unit: name.clone(),
            unit_type: "service".into(),
            generation: generation.booted.clone(),
            evidence: entry.path(),
            fragment: None,
            service_type: None,
            support: Support::Supported,
            reason_code: ReasonCode::SupportedService,
        };
        match inspect(&entry.path(), &name, generation, store) {
            Ok((fragment, kind, support, code)) => {
                d.fragment = Some(fragment);
                d.service_type = kind;
                d.support = support;
                d.reason_code = code;
            }
            Err(reason) => {
                d.support = Support::Indeterminate(reason);
                d.reason_code = ReasonCode::ArtifactUnavailable;
            }
        }
        result.push(d);
    }
    result.sort_by(|a, b| a.unit.cmp(&b.unit));
    Ok((result, skipped))
}
fn inspect(
    path: &Path,
    name: &str,
    generation: &Generation,
    store: &Path,
) -> Result<(PathBuf, Option<ServiceType>, Support, ReasonCode), String> {
    let fragment = path
        .canonicalize()
        .map_err(|e| format!("unit artifact unavailable: {e}"))?;
    let unsupported =
        |code, reason: String| (fragment.clone(), None, Support::Unsupported(reason), code);
    if fragment == Path::new("/dev/null") {
        return Ok(unsupported(
            ReasonCode::MaskedUnit,
            "masked unit; no continuous activity expectation".into(),
        ));
    }
    if !fragment.starts_with(store) {
        return Ok(unsupported(
            ReasonCode::ArtifactOutsideStore,
            "unit artifact outside the generation's store".into(),
        ));
    }
    if !valid_service_name(name) || name.contains('@') {
        return Ok(unsupported(
            ReasonCode::UnsupportedIdentity,
            "template/instance or unsupported unit identity".into(),
        ));
    }
    if fragment.file_name() != path.file_name() {
        return Ok(unsupported(
            ReasonCode::ArtifactNaming,
            "alias or unsupported artifact naming; canonical unit identity differs".into(),
        ));
    }
    for (suffix, code) in [
        ("socket", ReasonCode::SocketActivation),
        ("timer", ReasonCode::TimerActivation),
        ("path", ReasonCode::PathActivation),
    ] {
        let companion = generation
            .units
            .join(name.trim_end_matches("service").to_owned() + suffix);
        match fs::symlink_metadata(companion) {
            Ok(_) => {
                return Ok(unsupported(
                    code,
                    format!("{suffix} activation companion; no continuous activity inference"),
                ))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(format!("activation metadata unavailable: {e}")),
        }
    }
    // Override precedence and specifier expansion remain outside this adapter.
    for dropin in [format!("{name}.d"), "service.d".into()] {
        match fs::symlink_metadata(generation.units.join(dropin)) {
            Ok(_) => {
                return Ok(unsupported(
                    ReasonCode::GenerationDropIns,
                    "generation drop-ins are not interpreted".into(),
                ))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(format!("drop-in metadata unavailable: {e}")),
        }
    }
    let file = fs::File::open(&fragment).map_err(|e| format!("unit artifact unreadable: {e}"))?;
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
    let (kind, support, code) = classify_content(&content);
    Ok((fragment, kind, support, code))
}
pub fn classify_content(content: &str) -> (Option<ServiceType>, Support, ReasonCode) {
    let unsupported = |code, s: &str| (None, Support::Unsupported(s.into()), code);
    let mut section = "";
    let mut kind = None;
    let mut starts = 0;
    let mut has_start = false;
    let mut bus_names = 0;
    let mut static_bus_name = false;
    let mut credentials = false;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(['#', ';']) {
            continue;
        }
        if line.ends_with('\\') {
            return unsupported(
                ReasonCode::ContinuedDirective,
                "continued directives are not interpreted",
            );
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = &line[1..line.len() - 1];
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return unsupported(ReasonCode::UnsupportedSyntax, "unsupported unit syntax");
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
            return unsupported(
                ReasonCode::ConditionalSemantics,
                "conditional, retained, or activation-dependent service semantics",
            );
        }
        if section == "Service" {
            // Inspect only presence/shape. Commands and bus-name values are never
            // executed, persisted, or included in coverage diagnostics.
            match key {
                "ExecStart" => {
                    starts += 1;
                    has_start = !value.is_empty();
                }
                "BusName" => {
                    bus_names += 1;
                    static_bus_name = valid_bus_name(value);
                }
                _ => (),
            }
            credentials |= key.contains("Credential");
        }
        if section == "Service" && key == "Type" {
            if kind.is_some() {
                return unsupported(
                    ReasonCode::RepeatedType,
                    "repeated Type directives are not interpreted",
                );
            }
            kind = ServiceType::parse(value);
            if kind.is_none() {
                return unsupported(ReasonCode::UnsupportedType,"oneshot, forking, or unknown service Type is outside supported continuous classes");
            }
        }
    }
    if kind.is_none() {
        // systemd 260.5: no Type + one ExecStart + no BusName/credentials
        // defaults to simple; a static BusName defaults to dbus. Credential-
        // implied exec, reset/repeated directives and missing starts are excluded.
        if starts == 1 && has_start && !credentials {
            if bus_names == 0 {
                return (
                    Some(ServiceType::Simple),
                    Support::Supported,
                    ReasonCode::ImplicitSimpleService,
                );
            }
            if bus_names == 1 && static_bus_name {
                return (
                    Some(ServiceType::Dbus),
                    Support::Supported,
                    ReasonCode::ImplicitDbusService,
                );
            }
        }
        return unsupported(
            ReasonCode::ImplicitType,
            "implicit service Type lacks unambiguous non-credential ExecStart/BusName evidence",
        );
    }
    (kind, Support::Supported, ReasonCode::SupportedService)
}
fn valid_bus_name(value: &str) -> bool {
    // Deliberately narrower than all valid D-Bus names: no escaping, specifiers,
    // unique names, or dynamic expansion is interpreted.
    value.len() <= 255
        && value.contains('.')
        && value.split('.').all(|part| {
            let mut bytes = part.bytes();
            bytes
                .next()
                .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
                && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
        })
}
