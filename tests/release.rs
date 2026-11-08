use std::process::Command;
#[test]
fn executable_exit_codes_and_output_channels() {
    let binary = env!("CARGO_BIN_EXE_drift");
    let help = Command::new(binary).arg("--help").output().unwrap();
    assert!(help.status.success());
    assert!(help.stderr.is_empty());
    let invalid = Command::new(binary)
        .args(["diff", "--json"])
        .output()
        .unwrap();
    assert_eq!(invalid.status.code(), Some(2));
    assert!(invalid.stdout.is_empty());
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("bad.toml");
    for content in ["not = [", "unknown = true", "paths = ['relative']"] {
        std::fs::write(&config, content).unwrap();
        let output = Command::new(binary)
            .arg("--config")
            .arg(&config)
            .args(["scan", "--json"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }
    let missing = Command::new(binary)
        .arg("--config")
        .arg(dir.path().join("missing"))
        .arg("scan")
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(1));
    assert!(String::from_utf8(missing.stderr)
        .unwrap()
        .contains("check the path and read permissions"));
}
