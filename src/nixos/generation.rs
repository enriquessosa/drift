use super::model::{Capability, Generation};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone)]
pub struct SystemPaths {
    pub os_release: PathBuf,
    pub booted: PathBuf,
    pub activated: PathBuf,
    pub live_units: PathBuf,
    pub store: PathBuf,
    pub container: PathBuf,
}
impl Default for SystemPaths {
    fn default() -> Self {
        Self {
            os_release: "/etc/os-release".into(),
            booted: "/run/booted-system".into(),
            activated: "/run/current-system".into(),
            live_units: "/etc/systemd/system".into(),
            store: "/nix/store".into(),
            container: "/run/systemd/container".into(),
        }
    }
}
pub fn detect(paths: &SystemPaths) -> Result<Generation, (Capability, String)> {
    let unavailable = |s: String| (Capability::Unavailable, s);
    let release = fs::read_to_string(&paths.os_release)
        .map_err(|e| unavailable(format!("cannot detect NixOS: {e}")))?;
    let ids: Vec<_> = release
        .lines()
        .filter_map(|line| line.strip_prefix("ID="))
        .map(|s| s.trim_matches(['\"', '\'']))
        .collect();
    if ids != ["nixos"] {
        return Err((
            Capability::Unsupported,
            "NixOS ID not established; ordinary Linux observations remain available".into(),
        ));
    }
    match fs::symlink_metadata(&paths.container) {
        Ok(_) => {
            return Err((
                Capability::Unsupported,
                "container layout is not supported for booted-generation auditing".into(),
            ))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => {
            return Err(unavailable(format!(
                "cannot establish container capability: {e}"
            )))
        }
    }
    let store = paths
        .store
        .canonicalize()
        .map_err(|e| unavailable(format!("Nix store unavailable: {e}")))?;
    let booted = resolve_reference(&paths.booted, &store).map_err(unavailable)?;
    let version = booted.join("nixos-version");
    if !fs::metadata(&version).is_ok_and(|m| m.is_file()) {
        return Err(unavailable(
            "booted generation lacks nixos-version metadata".into(),
        ));
    }
    let units = booted
        .join("etc/systemd/system")
        .canonicalize()
        .map_err(|e| unavailable(format!("booted unit artifacts unavailable: {e}")))?;
    if !units.starts_with(&store) || !units.is_dir() {
        return Err(unavailable(
            "booted unit directory is outside the supported store layout".into(),
        ));
    }
    Ok(Generation {
        booted,
        activated: resolve_reference(&paths.activated, &store).ok(),
        units,
        live_units: paths.live_units.canonicalize().ok(),
        evidence: vec![
            paths.os_release.clone(),
            paths.booted.clone(),
            paths.activated.clone(),
            paths.live_units.clone(),
            version,
        ],
    })
}
fn resolve_reference(reference: &Path, store: &Path) -> Result<PathBuf, String> {
    if !fs::symlink_metadata(reference).is_ok_and(|m| m.is_symlink()) {
        return Err(format!(
            "{} must be an accessible generation symlink",
            reference.display()
        ));
    }
    let resolved = reference
        .canonicalize()
        .map_err(|e| format!("{}: {e}", reference.display()))?;
    let name = resolved
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let valid_hash = name.get(..32).is_some_and(|s| {
        s.bytes()
            .all(|b| b"0123456789abcdfghijklmnpqrsvwxyz".contains(&b))
    });
    if resolved.parent() != Some(store)
        || !valid_hash
        || !name
            .get(32..)
            .is_some_and(|s| s.starts_with("-nixos-system-"))
        || !resolved.is_dir()
    {
        return Err(format!(
            "{} does not identify a supported NixOS store generation",
            reference.display()
        ));
    }
    Ok(resolved)
}
