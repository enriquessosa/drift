use clap::Parser;
use drift::{
    app,
    cli::Cli,
    config::Config,
    model::{resource, Collection, Snapshot, Status, VERSION},
    nixos::{
        self, compare,
        generation::{self, SystemPaths},
        model::*,
        runtime, units,
    },
};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::fs::{symlink, PermissionsExt},
    path::PathBuf,
};
struct Fixture {
    _dir: tempfile::TempDir,
    paths: SystemPaths,
    generation: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let dir = tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()
            .unwrap();
        let root = dir.path();
        let store = root.join("store");
        let generation = store.join(format!("{}-nixos-system-fixture-1", "a".repeat(32)));
        fs::create_dir_all(generation.join("etc/systemd/system")).unwrap();
        fs::write(generation.join("nixos-version"), "fixture\n").unwrap();
        let paths = SystemPaths {
            os_release: root.join("os-release"),
            booted: root.join("booted"),
            activated: root.join("current"),
            live_units: root.join("live"),
            store,
            container: root.join("container"),
        };
        fs::write(&paths.os_release, "ID=nixos\n").unwrap();
        symlink(&generation, &paths.booted).unwrap();
        symlink(&generation, &paths.activated).unwrap();
        symlink(generation.join("etc/systemd/system"), &paths.live_units).unwrap();
        Self {
            _dir: dir,
            paths,
            generation,
        }
    }
    fn unit(&self, name: &str, content: &str) -> PathBuf {
        let path = self.generation.join("etc/systemd/system").join(name);
        fs::write(&path, content).unwrap();
        path
    }
    fn supported(&self) -> PathBuf {
        self.unit("example.service","[Unit]\nDescription=fixture\n[Service]\nType=simple\nExecStart=/not/executed\n[Install]\nWantedBy=multi-user.target\n")
    }
    fn state(&self, active: &str) -> Snapshot {
        let mut c = Collection::new("systemctl system services", vec!["system services".into()]);
        c.resources.insert(
            "example.service".into(),
            resource([
                ("load".into(), "loaded".into()),
                ("active".into(), active.into()),
                ("sub".into(), "running".into()),
                ("unit_file".into(), "static".into()),
            ]),
        );
        Snapshot {
            version: VERSION,
            timestamp: 1,
            host: "fixture-host".into(),
            collectors: BTreeMap::from([("systemd".into(), c)]),
        }
    }
    fn runtime(&self, active: &str) -> Runtime {
        Ok(BTreeMap::from([(
            "example.service".into(),
            RuntimeUnit {
                id: "example.service".into(),
                load: "loaded".into(),
                active: active.into(),
                sub: "running".into(),
                service_type: "simple".into(),
                fragment: self.generation.join("etc/systemd/system/example.service"),
                drop_ins: String::new(),
                reload: "no".into(),
                triggered_by: String::new(),
            },
        )]))
    }
    fn audit(&self, active: &str, config: &Config) -> Audit {
        nixos::audit_with(&self.paths, config, &self.state(active), |_| {
            self.runtime(active)
        })
    }
}
#[test]
fn detects_nixos_and_resolves_booted_without_using_selected_profile() {
    let f = Fixture::new();
    f.supported();
    let g = generation::detect(&f.paths).unwrap();
    assert_eq!(g.booted, f.generation);
    assert!(g.compatible());
    fs::write(
        f._dir.path().join("selected-profile"),
        "unactivated source must be ignored",
    )
    .unwrap();
    assert_eq!(generation::detect(&f.paths).unwrap(), g);
    fs::write(&f.paths.os_release, "ID=debian\nID_LIKE=nixos\n").unwrap();
    let audit = f.audit("inactive", &Config::default());
    assert_eq!(audit.capability, Capability::Unsupported);
    assert!(audit.comparisons.is_empty());
}
#[test]
fn missing_container_and_ambiguous_generation_references() {
    let f = Fixture::new();
    fs::remove_file(&f.paths.booted).unwrap();
    assert_eq!(
        generation::detect(&f.paths).unwrap_err().0,
        Capability::Unavailable
    );
    fs::create_dir(&f.paths.booted).unwrap();
    assert!(generation::detect(&f.paths).is_err());
    fs::remove_dir(&f.paths.booted).unwrap();
    symlink(f._dir.path(), &f.paths.booted).unwrap();
    assert!(generation::detect(&f.paths).is_err());
    fs::write(&f.paths.container, "").unwrap();
    assert_eq!(
        generation::detect(&f.paths).unwrap_err().0,
        Capability::Unsupported
    );
}
#[test]
fn activated_and_live_scopes_must_match_booted() {
    let f = Fixture::new();
    f.supported();
    let other = f
        .paths
        .store
        .join(format!("{}-nixos-system-other-1", "b".repeat(32)));
    fs::create_dir(&other).unwrap();
    fs::remove_file(&f.paths.activated).unwrap();
    symlink(&other, &f.paths.activated).unwrap();
    let audit = f.audit("active", &Config::default());
    assert_eq!(audit.generation.unwrap().booted, f.generation);
    assert_eq!(audit.comparisons[0].outcome, Outcome::Indeterminate);
    fs::remove_file(&f.paths.activated).unwrap();
    assert_eq!(
        f.audit("active", &Config::default()).comparisons[0].outcome,
        Outcome::Indeterminate
    );
    symlink(&f.generation, &f.paths.activated).unwrap();
    fs::remove_file(&f.paths.live_units).unwrap();
    symlink(&other, &f.paths.live_units).unwrap();
    assert_eq!(
        f.audit("active", &Config::default()).comparisons[0].outcome,
        Outcome::Indeterminate
    );
}
#[test]
fn inactive_enabled_static_or_indirect_service_has_no_implicit_activity_expectation() {
    let f = Fixture::new();
    f.supported();
    for unit_file in ["enabled", "static", "indirect", "disabled"] {
        let mut s = f.state("inactive");
        s.collectors
            .get_mut("systemd")
            .unwrap()
            .resources
            .get_mut("example.service")
            .unwrap()
            .properties
            .insert("unit_file".into(), unit_file.into());
        let audit = nixos::audit_with(&f.paths, &Config::default(), &s, |_| f.runtime("inactive"));
        assert_eq!(audit.comparisons[0].outcome, Outcome::Match);
        assert!(audit.comparisons[0]
            .reason
            .contains("no continuous activity expectation"));
    }
}
#[test]
fn pinned_activity_expectations_match_mismatch_and_reject_wrong_generation() {
    let f = Fixture::new();
    f.supported();
    let mut config = Config {
        nixos_expectations: vec![ActiveExpectation {
            unit: "example.service".into(),
            generation: f.generation.clone(),
            active: true,
        }],
        ..Config::default()
    };
    assert_eq!(
        f.audit("active", &config).comparisons[0].outcome,
        Outcome::Match
    );
    assert_eq!(
        f.audit("inactive", &config).comparisons[0].outcome,
        Outcome::Mismatch
    );
    assert_eq!(
        f.audit("activating", &config).comparisons[0].outcome,
        Outcome::Indeterminate
    );
    config.nixos_expectations[0].generation = PathBuf::from("/different");
    assert_eq!(
        f.audit("active", &config).comparisons[0].outcome,
        Outcome::Indeterminate
    );
    config.nixos_expectations[0].unit = "absent.service".into();
    assert!(f
        .audit("active", &config)
        .comparisons
        .iter()
        .any(|c| c.unit == "absent.service" && c.outcome == Outcome::Indeterminate));
}
#[test]
fn supported_types_and_unsupported_activation_syntax() {
    for kind in ["simple", "exec", "notify", "notify-reload", "dbus", "idle"] {
        assert_eq!(
            units::classify_content(&format!("[Service]\nType={kind}\n")).1,
            Support::Supported
        );
    }
    for content in [
        "[Service]\nType=oneshot\n",
        "[Service]\nType=forking\n",
        "[Service]\nExecStart=x\n",
        "[Unit]\nConditionPathExists=/anything\n[Service]\nType=simple\n",
        "[Unit]\nAssertPathExists=/anything\n[Service]\nType=simple\n",
        "[Service]\nType=simple\nRemainAfterExit=yes\n",
        "[Service]\nType=simple\nExecCondition=anything\n",
        "[Service]\nType=simple\nType=notify\n",
        "[Service]\nType=simple\nExecStart=anything \\\n continuation\n",
    ] {
        assert!(matches!(
            units::classify_content(content).1,
            Support::Unsupported(_)
        ));
    }
}
#[test]
fn templates_masks_companions_and_dropins_are_unsupported() {
    let f = Fixture::new();
    f.supported();
    f.unit("example@.service", "[Service]\nType=simple\n");
    f.unit("example@instance.service", "[Service]\nType=simple\n");
    symlink(
        "/dev/null",
        f.generation.join("etc/systemd/system/masked.service"),
    )
    .unwrap();
    for suffix in ["socket", "timer", "path"] {
        let path = f.unit(&format!("example.{suffix}"), "");
        let audit = f.audit("inactive", &Config::default());
        assert!(audit
            .comparisons
            .iter()
            .all(|c| c.outcome == Outcome::Unsupported));
        fs::remove_file(path).unwrap();
    }
    fs::create_dir(f.generation.join("etc/systemd/system/example.service.d")).unwrap();
    assert_eq!(
        f.audit("active", &Config::default())
            .comparisons
            .iter()
            .find(|c| c.unit == "example.service")
            .unwrap()
            .outcome,
        Outcome::Unsupported
    );
}
#[test]
fn missing_and_partial_runtime_evidence_is_indeterminate() {
    let f = Fixture::new();
    f.supported();
    for status in [Status::Partial, Status::Failed, Status::Disabled] {
        let mut state = f.state("active");
        state.collectors.get_mut("systemd").unwrap().status = status;
        assert_eq!(
            nixos::audit_with(&f.paths, &Config::default(), &state, |_| f
                .runtime("active"))
            .comparisons[0]
                .outcome,
            Outcome::Indeterminate
        );
    }
    let mut state = f.state("active");
    state.collectors.clear();
    assert_eq!(
        nixos::audit_with(&f.paths, &Config::default(), &state, |_| f
            .runtime("active"))
        .comparisons[0]
            .outcome,
        Outcome::Indeterminate
    );
    let audit = nixos::audit_with(&f.paths, &Config::default(), &f.state("active"), |_| {
        Err("access denied".into())
    });
    assert_eq!(audit.comparisons[0].outcome, Outcome::Indeterminate);
    assert_eq!(audit.comparisons[0].reason, "access denied");
    let audit = nixos::audit_with(&f.paths, &Config::default(), &f.state("active"), |_| {
        Ok(BTreeMap::new())
    });
    assert_eq!(audit.comparisons[0].outcome, Outcome::Indeterminate);
}
#[test]
fn artifact_and_type_mismatches_are_narrow_and_evidenced() {
    let f = Fixture::new();
    f.supported();
    let g = generation::detect(&f.paths).unwrap();
    let (declarations, _) = units::declarations(&g, &f.paths.store).unwrap();
    for change in [
        "fragment",
        "type",
        "reload",
        "trigger",
        "dropin",
        "race",
        "missingpath",
        "missingtype",
        "unknowntype",
    ] {
        let mut runtime = f.runtime("active").unwrap();
        let r = runtime.get_mut("example.service").unwrap();
        let expected = match change {
            "fragment" => {
                r.fragment = "/another.service".into();
                Outcome::Mismatch
            }
            "type" => {
                r.service_type = "notify".into();
                Outcome::Mismatch
            }
            "reload" => {
                r.reload = "yes".into();
                Outcome::Indeterminate
            }
            "trigger" => {
                r.triggered_by = "activation.socket".into();
                Outcome::Unsupported
            }
            "dropin" => {
                r.drop_ins = "/override.conf".into();
                Outcome::Unsupported
            }
            "race" => {
                r.active = "inactive".into();
                Outcome::Indeterminate
            }
            "missingtype" => {
                r.service_type.clear();
                Outcome::Indeterminate
            }
            "unknowntype" => {
                r.service_type = "future-type".into();
                Outcome::Unsupported
            }
            _ => {
                r.fragment = PathBuf::new();
                Outcome::Indeterminate
            }
        };
        let result = compare::compare(
            &declarations,
            &g,
            f.state("active").collectors.get("systemd"),
            &Ok(runtime),
            &[],
        );
        assert_eq!(result[0].outcome, expected, "{change}");
        assert!(!result[0].reason.is_empty());
        assert!(!result[0].evidence.is_empty());
    }
}
#[test]
fn unreadable_or_escaped_artifacts_are_not_successful_empty_evidence() {
    let f = Fixture::new();
    symlink(
        f._dir.path().join("missing"),
        f.generation.join("etc/systemd/system/missing.service"),
    )
    .unwrap();
    let outside = f._dir.path().join("outside.service");
    fs::write(&outside, "[Service]\nType=simple\n").unwrap();
    symlink(
        outside,
        f.generation.join("etc/systemd/system/outside.service"),
    )
    .unwrap();
    let audit = f.audit("active", &Config::default());
    assert_eq!(audit.comparisons[0].outcome, Outcome::Indeterminate);
    assert_eq!(audit.comparisons[1].outcome, Outcome::Unsupported);
}
#[test]
fn generation_changes_during_collection_invalidate_comparisons() {
    let f = Fixture::new();
    f.supported();
    let audit = nixos::audit_with(&f.paths, &Config::default(), &f.state("active"), |_| {
        fs::remove_file(&f.paths.activated).unwrap();
        f.runtime("active")
    });
    assert_eq!(audit.comparisons[0].outcome, Outcome::Indeterminate);
    assert!(audit.comparisons[0].reason.contains("changed"));
}
#[test]
fn structured_property_parser_checks_completeness() {
    let text="Id=example.service\nLoadState=loaded\nActiveState=active\nSubState=running\nType=simple\nFragmentPath=/fixture/example.service\nDropInPaths=\nNeedDaemonReload=no\nTriggeredBy=\n";
    assert_eq!(
        runtime::parse(text).unwrap()["example.service"].service_type,
        "simple"
    );
    assert!(runtime::parse("Id=example.service\n").is_err());
    assert!(runtime::parse(&(text.to_owned() + "Id=duplicate.service\n")).is_err());
}
#[test]
fn cli_json_and_disable_are_deterministic_and_do_not_open_database() {
    let f = Fixture::new();
    f.supported();
    let config = f._dir.path().join("config.toml");
    let database = f._dir.path().join("new/state.sqlite3");
    fs::write(
        &config,
        format!("database = {:?}\n", database.to_str().unwrap()),
    )
    .unwrap();
    let run = |args: &[&str]| {
        app::run_with_auditor(
            Cli::try_parse_from(
                ["drift", "--config", config.to_str().unwrap()]
                    .into_iter()
                    .chain(args.iter().copied()),
            )
            .unwrap(),
            |_| Ok(f.state("inactive")),
            |config, state| nixos::audit_with(&f.paths, config, state, |_| f.runtime("inactive")),
        )
        .unwrap()
    };
    let a = run(&["scan", "--json"]);
    assert_eq!(a, run(&["scan", "--json"]));
    let json: serde_json::Value = serde_json::from_str(&a).unwrap();
    assert_eq!(json["declared"]["comparisons"][0]["outcome"], "match");
    assert_eq!(json["observation"]["version"], 1);
    assert!(run(&["scan"]).contains("Declared state: Supported"));
    assert!(!database.exists());
    let cli = Cli::try_parse_from(["drift", "scan", "--no-declared", "--json"]).unwrap();
    let disabled = app::run_with_auditor(
        cli,
        |_| Ok(f.state("inactive")),
        |_, _| panic!("disabled auditor must not run"),
    )
    .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&disabled).unwrap()["declared"]["capability"],
        "disabled"
    );
    assert!(Cli::try_parse_from(["drift", "snapshot", "create", "--json"]).is_err());
}
#[test]
fn expectations_validate_unit_identity_and_generation() {
    let f = Fixture::new();
    let path = f._dir.path().join("config.toml");
    for (unit, generation) in [
        ("../bad.service", "/absolute"),
        ("--bad.service", "/absolute"),
        ("a@.service", "/absolute"),
        ("a.service", "relative"),
    ] {
        fs::write(&path,format!("[[nixos_expectations]]\nunit = {unit:?}\ngeneration = {generation:?}\nactive = true\n")).unwrap();
        assert!(Config::load(Some(&path)).is_err());
    }
}
#[test]
fn milestone_one_snapshots_roundtrip_without_schema_evolution() {
    let json = r#"{"version":1,"timestamp":42,"host":"fixture","collectors":{"systemd":{"source":"systemctl system services","coverage":["system services"],"scope":null,"status":"Complete","errors":[],"resources":{"example.service":{"properties":{"load":"loaded","active":"inactive","sub":"dead","unit_file":"static"},"processes":[]}}}}}"#;
    let snapshot: Snapshot = serde_json::from_str(json).unwrap();
    let f = Fixture::new();
    let mut store =
        drift::storage::sqlite::Store::open(&f._dir.path().join("snapshots.sqlite3")).unwrap();
    let id = store.save(&snapshot).unwrap();
    assert_eq!(store.get(id).unwrap(), snapshot);
    assert_eq!(
        drift::diff::compare(&snapshot, &snapshot).unwrap().entries[0].kind,
        drift::diff::Kind::Unchanged
    );
    let value = serde_json::to_value(&snapshot).unwrap();
    assert!(value.get("declared").is_none());
    assert_eq!(value["version"], 1);
}

#[test]
fn missing_detection_metadata_and_unit_enumeration_fail_explicitly() {
    let f = Fixture::new();
    fs::remove_file(&f.paths.os_release).unwrap();
    assert_eq!(
        f.audit("active", &Config::default()).capability,
        Capability::Unavailable
    );
    fs::write(&f.paths.os_release, "ID=nixos\n").unwrap();
    fs::remove_dir(f.generation.join("etc/systemd/system")).unwrap();
    let audit = f.audit("active", &Config::default());
    assert_eq!(audit.capability, Capability::Unavailable);
    assert!(!audit.reason.is_empty());
}
#[test]
fn incompatible_systemd_observation_scope_is_indeterminate() {
    let f = Fixture::new();
    f.supported();
    for change in ["source", "coverage", "absent unit"] {
        let mut state = f.state("active");
        let c = state.collectors.get_mut("systemd").unwrap();
        match change {
            "source" => c.source = "other manager".into(),
            "coverage" => c.coverage = vec!["user services".into()],
            _ => c.resources.clear(),
        }
        assert_eq!(
            nixos::audit_with(&f.paths, &Config::default(), &state, |_| f
                .runtime("active"))
            .comparisons[0]
                .outcome,
            Outcome::Indeterminate
        );
    }
}
#[test]
fn stable_inventory_and_json_with_mixed_supported_classes() {
    let f = Fixture::new();
    f.supported();
    f.unit("z.service", "[Service]\nType=oneshot\n");
    f.unit("a.target", "[Unit]\nDescription=not a service\n");
    let a = f.audit("inactive", &Config::default());
    let b = f.audit("inactive", &Config::default());
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
    assert_eq!(
        a.declarations
            .iter()
            .map(|d| d.unit.as_str())
            .collect::<Vec<_>>(),
        vec!["example.service", "z.service"]
    );
    assert_eq!(a.skipped_non_services, 1);
    let restored: Audit = serde_json::from_str(&serde_json::to_string(&a).unwrap()).unwrap();
    assert_eq!(restored, a);
}

#[test]
fn disabled_or_incomplete_systemd_never_runs_supplemental_queries() {
    let f = Fixture::new();
    f.supported();
    for status in [Status::Disabled, Status::Failed, Status::Partial] {
        let mut state = f.state("active");
        state.collectors.get_mut("systemd").unwrap().status = status;
        let audit = nixos::audit_with(&f.paths, &Config::default(), &state, |_| {
            panic!("disabled or incomplete collector must not trigger supplemental queries")
        });
        assert_eq!(audit.comparisons[0].outcome, Outcome::Indeterminate);
    }
    let config = Config {
        systemd: false,
        ..Config::default()
    };
    assert_eq!(
        nixos::audit_with(&f.paths, &config, &f.state("active"), |_| panic!(
            "disabled configuration must not run queries"
        ))
        .comparisons[0]
            .outcome,
        Outcome::Indeterminate
    );
}
