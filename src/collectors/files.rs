use crate::model::{resource, Collection};
use std::{os::unix::fs::MetadataExt, path::PathBuf};
pub fn collect(paths: &[PathBuf]) -> Collection {
    collect_with_metadata(paths, |path| std::fs::symlink_metadata(path))
}
/// Injectable metadata syscall boundary for deterministic access-error fixtures.
pub fn collect_with_metadata(
    paths: &[PathBuf],
    metadata: impl Fn(&std::path::Path) -> std::io::Result<std::fs::Metadata>,
) -> Collection {
    let mut c = Collection::new(
        "symlink_metadata (no final symlink dereference)",
        paths
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
    );
    for path in paths {
        let id = path.to_string_lossy().into_owned();
        // Reject symlink ancestors, including directory symlinks. The final
        // component is deliberately retained as a symlink observation.
        let ancestor_error = path.ancestors().skip(1).find_map(|ancestor| {
            match std::fs::symlink_metadata(ancestor) {
                Ok(m) if m.is_symlink() => {
                    Some(format!("symlink ancestor: {}", ancestor.display()))
                }
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                    Some(format!("{}: {e}", ancestor.display()))
                }
                _ => None,
            }
        });
        if let Some(error) = ancestor_error {
            c.fail(error);
            continue;
        }
        let pairs = match metadata(path) {
            Ok(m) => vec![
                ("exists".into(), "true".into()),
                (
                    "type".into(),
                    if m.is_symlink() {
                        "symlink"
                    } else if m.is_dir() {
                        "directory"
                    } else if m.is_file() {
                        "file"
                    } else {
                        "special"
                    }
                    .into(),
                ),
                ("mode".into(), format!("{:04o}", m.mode() & 0o7777)),
                ("uid".into(), m.uid().to_string()),
                ("gid".into(), m.gid().to_string()),
            ],
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                vec![("exists".into(), "false".into())]
            }
            Err(e) => {
                c.fail(format!("{}: {e}", path.display()));
                continue;
            }
        };
        c.resources.insert(id, resource(pairs));
    }
    c
}
