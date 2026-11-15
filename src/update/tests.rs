use std::{
    cell::RefCell,
    fs, io,
    path::{Path, PathBuf},
};

use tempfile::TempDir;

use super::{
    BackendFailureKind, Platform, ReleaseVersion, UpdateBackend, UpdateEnvironment, UpdateError,
    UpdateOutcome,
    backend::{
        CHECKSUM_ASSET, latest_compatible_release, map_apply_error, standalone_asset_matches,
    },
    command::run,
};
use crate::{
    AppError, AppOutcome, Cli, app, cli::Command, config::AppPaths, output::Output,
    process::testing::RecordingExecutor, session::SessionEnvironment,
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum BackendCall {
    Latest {
        current: ReleaseVersion,
        platform: Platform,
    },
    Apply {
        current: ReleaseVersion,
        target: ReleaseVersion,
        platform: Platform,
        executable: PathBuf,
    },
}

struct FakeBackend {
    latest: RefCell<Option<Result<Option<ReleaseVersion>, UpdateError>>>,
    apply: RefCell<Option<Result<(), UpdateError>>>,
    calls: RefCell<Vec<BackendCall>>,
}

impl FakeBackend {
    fn with_latest(latest: Option<ReleaseVersion>) -> Self {
        Self {
            latest: RefCell::new(Some(Ok(latest))),
            apply: RefCell::new(Some(Ok(()))),
            calls: RefCell::new(Vec::new()),
        }
    }

    fn latest_error(kind: BackendFailureKind, message: &str) -> Self {
        Self {
            latest: RefCell::new(Some(Err(UpdateError::backend(kind, message)))),
            apply: RefCell::new(Some(Ok(()))),
            calls: RefCell::new(Vec::new()),
        }
    }

    fn apply_error(kind: BackendFailureKind, message: &str) -> Self {
        Self {
            latest: RefCell::new(Some(Ok(Some(version("2.0.1"))))),
            apply: RefCell::new(Some(Err(UpdateError::backend(kind, message)))),
            calls: RefCell::new(Vec::new()),
        }
    }

    fn calls(&self) -> Vec<BackendCall> {
        self.calls.borrow().clone()
    }
}

impl UpdateBackend for FakeBackend {
    fn latest_compatible(
        &self,
        current: ReleaseVersion,
        platform: Platform,
    ) -> Result<Option<ReleaseVersion>, UpdateError> {
        self.calls
            .borrow_mut()
            .push(BackendCall::Latest { current, platform });
        self.latest
            .borrow_mut()
            .take()
            .expect("fake latest result was already consumed")
    }

    fn apply(
        &self,
        current: ReleaseVersion,
        target: ReleaseVersion,
        platform: Platform,
        executable: &Path,
    ) -> Result<(), UpdateError> {
        self.calls.borrow_mut().push(BackendCall::Apply {
            current,
            target,
            platform,
            executable: executable.to_path_buf(),
        });
        self.apply
            .borrow_mut()
            .take()
            .expect("fake apply result was already consumed")
    }
}

fn version(value: &str) -> ReleaseVersion {
    ReleaseVersion::parse_v2(value).unwrap()
}

fn environment(temp: &TempDir) -> UpdateEnvironment {
    UpdateEnvironment {
        platform: Platform::LinuxX86_64,
        executable: temp.path().join("standalone/bin/dstack"),
        package_marker: temp.path().join("missing-package-marker"),
        cargo_home: Some(temp.path().join("cargo-home")),
        home_dir: Some(temp.path().join("home")),
    }
}

fn execute(
    argv: &[&str],
    backend: &dyn UpdateBackend,
    environment: &UpdateEnvironment,
) -> (Result<UpdateOutcome, UpdateError>, String) {
    let cli = Cli::try_parse_from(argv).unwrap();
    let Some(Command::SelfUpdate(arguments)) = cli.command else {
        panic!("self-update command should parse");
    };
    let mut output = Vec::new();
    let result = run(
        &arguments,
        backend,
        environment,
        &mut Output::plain(&mut output),
    );
    (result, String::from_utf8(output).unwrap())
}

#[test]
fn self_update_has_exact_public_grammar() {
    let plain = Cli::try_parse_from(["dstack", "self-update"]).unwrap();
    assert!(matches!(
        plain.command,
        Some(Command::SelfUpdate(crate::cli::SelfUpdateArgs {
            check: false
        }))
    ));
    let check = Cli::try_parse_from(["dstack", "self-update", "--check"]).unwrap();
    assert!(matches!(
        check.command,
        Some(Command::SelfUpdate(crate::cli::SelfUpdateArgs {
            check: true
        }))
    ));

    for command in ["update-self", "upgrade", "self-upgrade"] {
        assert!(
            Cli::try_parse_from(["dstack", command]).is_err(),
            "{command}"
        );
    }
    assert!(Cli::try_parse_from(["dstack", "update", "--self"]).is_err());
    for flag in ["--yes", "--force", "--downgrade", "--version"] {
        assert!(
            Cli::try_parse_from(["dstack", "self-update", flag]).is_err(),
            "{flag}"
        );
    }
}

#[test]
fn supported_platform_mapping_is_complete_and_explicit() {
    let cases = [
        (
            "linux",
            "x86_64",
            Platform::LinuxX86_64,
            "linux-x86_64",
            "tar.gz",
            "dstack",
        ),
        (
            "linux",
            "aarch64",
            Platform::LinuxAarch64,
            "linux-aarch64",
            "tar.gz",
            "dstack",
        ),
        (
            "macos",
            "x86_64",
            Platform::MacosX86_64,
            "macos-x86_64",
            "tar.gz",
            "dstack",
        ),
        (
            "macos",
            "aarch64",
            Platform::MacosAarch64,
            "macos-aarch64",
            "tar.gz",
            "dstack",
        ),
        (
            "windows",
            "x86_64",
            Platform::WindowsX86_64,
            "windows-x86_64",
            "zip",
            "dstack.exe",
        ),
    ];
    for (os, arch, platform, key, extension, binary) in cases {
        assert_eq!(Platform::detect(os, arch).unwrap(), platform);
        assert_eq!(platform.key(), key);
        assert_eq!(platform.archive_extension(), extension);
        assert_eq!(platform.binary_name(), binary);
    }
    for (os, arch) in [
        ("windows", "aarch64"),
        ("linux", "x86"),
        ("freebsd", "x86_64"),
        ("openbsd", "x86_64"),
    ] {
        assert!(matches!(
            Platform::detect(os, arch),
            Err(UpdateError::UnsupportedPlatform { .. })
        ));
    }
}

#[test]
fn stable_v2_version_filter_rejects_other_lines_and_prereleases() {
    for accepted in ["2.0.0", "v2.1.23", "2.999.4"] {
        assert!(ReleaseVersion::parse_v2(accepted).is_some(), "{accepted}");
    }
    for rejected in [
        "1.9.0",
        "v3.0.0",
        "v2.1.0-beta.1",
        "2.1",
        "2.1.0.0",
        "V2.1.0",
        "foo",
    ] {
        assert!(ReleaseVersion::parse_v2(rejected).is_none(), "{rejected}");
    }
}

#[test]
fn standalone_asset_contract_is_exact_and_unambiguous() {
    let expected = Platform::LinuxX86_64.archive_name(version("2.4.1"));
    assert_eq!(expected, "dstack-v2.4.1-linux-x86_64.tar.gz");
    assert!(standalone_asset_matches(&expected, &expected));
    for candidate in [
        "dstack-compose_2.4.1_amd64.deb",
        "dstack-compose-2.4.1-1.x86_64.rpm",
        "dstack-compose-2.4.1-1-x86_64.pkg.tar.zst",
        "install-dstack.sh",
        "install-dstack.ps1",
        "dstack-v2.4.1-linux-x86_64.tar.gz.sha256",
        "SHA256SUMS",
        "dstack-v2.4.1-linux-aarch64.tar.gz",
    ] {
        assert!(
            !standalone_asset_matches(candidate, &expected),
            "{candidate}"
        );
    }
}

#[test]
fn published_release_selection_keeps_current_and_requires_the_exact_platform_archive() {
    fn release(version: &str, assets: &[&str]) -> self_update::update::Release {
        let mut builder = self_update::update::Release::builder();
        builder.version(version).assets(assets.iter().map(|name| {
            self_update::update::ReleaseAsset::new(
                (*name).to_owned(),
                format!("https://example.invalid/{name}"),
            )
        }));
        builder.build().unwrap()
    }

    let releases = [
        release("3.0.0", &["dstack-v3.0.0-linux-x86_64.tar.gz"]),
        release(
            "2.0.2-beta.1",
            &["dstack-v2.0.2-beta.1-linux-x86_64.tar.gz"],
        ),
        release("2.0.1", &["dstack-compose_2.0.1_amd64.deb"]),
        release("2.0.0", &["dstack-v2.0.0-linux-x86_64.tar.gz"]),
    ];

    assert_eq!(
        latest_compatible_release(&releases, Platform::LinuxX86_64),
        Some(version("2.0.0"))
    );
    assert_eq!(
        latest_compatible_release(&releases, Platform::MacosAarch64),
        None
    );
}

#[test]
fn checksum_manifest_policy_is_mandatory_and_stable() {
    assert_eq!(CHECKSUM_ASSET, "SHA256SUMS");
}

#[test]
fn check_reports_up_to_date_and_passes_the_compiled_version() {
    let temp = TempDir::new().unwrap();
    let backend = FakeBackend::with_latest(Some(version("2.0.0")));
    let (outcome, output) = execute(
        &["dstack", "self-update", "--check"],
        &backend,
        &environment(&temp),
    );
    assert_eq!(
        outcome.unwrap(),
        UpdateOutcome::UpToDate {
            current: version("2.0.0")
        }
    );
    assert_eq!(output, "[OK] DStack 2.0.0 is up to date.\n");
    assert_eq!(
        backend.calls(),
        [BackendCall::Latest {
            current: version("2.0.0"),
            platform: Platform::LinuxX86_64
        }]
    );
}

#[test]
fn check_reports_update_available_without_applying_it() {
    let temp = TempDir::new().unwrap();
    let backend = FakeBackend::with_latest(Some(version("2.0.1")));
    let (outcome, output) = execute(
        &["dstack", "self-update", "--check"],
        &backend,
        &environment(&temp),
    );
    assert_eq!(
        outcome.unwrap(),
        UpdateOutcome::UpdateAvailable {
            current: version("2.0.0"),
            latest: version("2.0.1")
        }
    );
    assert_eq!(output, "[INFO] Update available: 2.0.0 -> 2.0.1\n");
    assert_eq!(backend.calls().len(), 1);
}

#[test]
fn apply_reports_success_and_completion_reload_guidance() {
    let temp = TempDir::new().unwrap();
    let environment = environment(&temp);
    let backend = FakeBackend::with_latest(Some(version("2.0.1")));
    let (outcome, output) = execute(&["dstack", "self-update"], &backend, &environment);
    assert_eq!(
        outcome.unwrap(),
        UpdateOutcome::Updated {
            from: version("2.0.0"),
            to: version("2.0.1")
        }
    );
    assert!(output.contains("[OK] Updated DStack 2.0.0 -> 2.0.1."));
    assert!(output.contains("reload your shell profile"));
    assert_eq!(
        backend.calls()[1],
        BackendCall::Apply {
            current: version("2.0.0"),
            target: version("2.0.1"),
            platform: Platform::LinuxX86_64,
            executable: environment.executable
        }
    );
}

#[test]
fn no_compatible_release_is_a_successful_explicit_outcome() {
    let temp = TempDir::new().unwrap();
    for argv in [
        ["dstack", "self-update", "--check"].as_slice(),
        ["dstack", "self-update"].as_slice(),
    ] {
        let backend = FakeBackend::with_latest(None);
        let (outcome, output) = execute(argv, &backend, &environment(&temp));
        assert_eq!(
            outcome.unwrap(),
            UpdateOutcome::NoCompatiblePublishedRelease
        );
        assert_eq!(
            output,
            "[INFO] No compatible published DStack v2 release is available.\n"
        );
        assert_eq!(backend.calls().len(), 1);
    }
}

#[test]
fn backend_failures_preserve_network_checksum_archive_and_replacement_categories() {
    let temp = TempDir::new().unwrap();
    let network = FakeBackend::latest_error(BackendFailureKind::Network, "offline");
    let (error, _) = execute(
        &["dstack", "self-update", "--check"],
        &network,
        &environment(&temp),
    );
    assert!(matches!(
        error.unwrap_err(),
        UpdateError::Backend {
            kind: BackendFailureKind::Network,
            ..
        }
    ));

    for kind in [
        BackendFailureKind::Checksum,
        BackendFailureKind::Archive,
        BackendFailureKind::Replacement,
    ] {
        let backend = FakeBackend::apply_error(kind, "apply failed");
        let (error, _) = execute(&["dstack", "self-update"], &backend, &environment(&temp));
        assert!(matches!(
            error.unwrap_err(),
            UpdateError::Backend { kind: actual, .. } if actual == kind
        ));
    }
}

#[test]
fn backend_io_failures_distinguish_archive_staging_from_annotated_replacement() {
    let archive = map_apply_error(self_update::Error::Io(io::Error::other(
        "could not extract archive",
    )));
    assert!(matches!(
        archive,
        UpdateError::Backend {
            kind: BackendFailureKind::Archive,
            ..
        }
    ));

    let replacement = map_apply_error(self_update::Error::Io(io::Error::other(
        "installing to /tmp/dstack: device full",
    )));
    assert!(matches!(
        replacement,
        UpdateError::Backend {
            kind: BackendFailureKind::Replacement,
            ..
        }
    ));
}

#[test]
fn official_system_package_can_check_but_cannot_apply() {
    let temp = TempDir::new().unwrap();
    let marker = temp.path().join("install-method");
    fs::write(&marker, "system-package\n").unwrap();
    let environment = UpdateEnvironment {
        platform: Platform::LinuxX86_64,
        executable: PathBuf::from("/usr/bin/dstack"),
        package_marker: marker,
        cargo_home: None,
        home_dir: None,
    };

    let check_backend = FakeBackend::with_latest(Some(version("2.0.1")));
    assert!(
        execute(
            &["dstack", "self-update", "--check"],
            &check_backend,
            &environment
        )
        .0
        .is_ok()
    );
    let apply_backend = FakeBackend::with_latest(Some(version("2.0.1")));
    let (error, _) = execute(&["dstack", "self-update"], &apply_backend, &environment);
    assert!(matches!(
        error.unwrap_err(),
        UpdateError::SystemPackageManaged { .. }
    ));
    assert_eq!(apply_backend.calls().len(), 1);
}

#[test]
fn normal_cargo_install_can_check_but_cannot_apply() {
    let temp = TempDir::new().unwrap();
    let cargo_home = temp.path().join("cargo");
    let environment = UpdateEnvironment {
        platform: Platform::LinuxX86_64,
        executable: cargo_home.join("bin/dstack"),
        package_marker: temp.path().join("missing"),
        cargo_home: Some(cargo_home),
        home_dir: None,
    };
    let check_backend = FakeBackend::with_latest(Some(version("2.0.1")));
    assert!(
        execute(
            &["dstack", "self-update", "--check"],
            &check_backend,
            &environment
        )
        .0
        .is_ok()
    );
    let apply_backend = FakeBackend::with_latest(Some(version("2.0.1")));
    let (error, _) = execute(&["dstack", "self-update"], &apply_backend, &environment);
    assert!(matches!(
        error.unwrap_err(),
        UpdateError::CargoManaged { .. }
    ));
    assert_eq!(apply_backend.calls().len(), 1);
}

#[test]
fn standalone_install_is_allowed_to_reach_verified_apply_backend() {
    let temp = TempDir::new().unwrap();
    let backend = FakeBackend::with_latest(Some(version("2.0.1")));
    assert!(
        execute(&["dstack", "self-update"], &backend, &environment(&temp))
            .0
            .is_ok()
    );
    assert!(matches!(backend.calls()[1], BackendCall::Apply { .. }));
}

#[test]
fn early_route_ignores_config_session_and_docker_boundaries() {
    let temp = TempDir::new().unwrap();
    let paths = AppPaths::with_home(
        temp.path().join("state-that-must-not-exist"),
        temp.path().join("home"),
        temp.path().join("legacy/registry"),
        None,
    );
    let executor = RecordingExecutor::default();
    let backend = FakeBackend::with_latest(Some(version("2.0.0")));
    let outcome = app::run_with_update_backend(
        Cli::try_parse_from(["dstack", "self-update", "--check"]).unwrap(),
        app::UpdateRouteEnvironment {
            paths: &paths,
            current_dir: temp.path(),
            executor: &executor,
            session: &SessionEnvironment::with_value("malformed DSTACK_SESSION"),
        },
        &backend,
        &environment(&temp),
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .unwrap();
    assert_eq!(outcome, AppOutcome::Success);
    assert!(!paths.application_dir().exists());
    assert!(executor.requests().is_empty());
    assert!(executor.capture_requests().is_empty());
}

#[test]
fn root_stack_and_file_options_are_rejected_before_update() {
    let temp = TempDir::new().unwrap();
    let paths = AppPaths::with_home(
        temp.path().join("state"),
        temp.path().join("home"),
        temp.path().join("legacy/registry"),
        None,
    );
    for argv in [
        ["dstack", "--stack", "x", "self-update"].as_slice(),
        ["dstack", "--file", "compose.yaml", "self-update"].as_slice(),
    ] {
        let backend = FakeBackend::with_latest(Some(version("2.0.1")));
        let error = app::run_with_update_backend(
            Cli::try_parse_from(argv).unwrap(),
            app::UpdateRouteEnvironment {
                paths: &paths,
                current_dir: temp.path(),
                executor: &RecordingExecutor::default(),
                session: &SessionEnvironment::absent(),
            },
            &backend,
            &environment(&temp),
            &mut Vec::new(),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(matches!(error, AppError::InvalidOptions(_)));
        assert!(backend.calls().is_empty());
    }
}
