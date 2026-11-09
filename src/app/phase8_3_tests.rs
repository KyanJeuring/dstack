use std::{ffi::OsString, fs, io, path::PathBuf};

use tempfile::TempDir;

use super::run_with_session_executor;
use crate::{
    AppError, AppOutcome, Cli,
    config::AppPaths,
    process::{ProcessStatus, StdioPolicy, testing::RecordingExecutor},
    session::SessionEnvironment,
};

fn paths(temp: &TempDir) -> AppPaths {
    AppPaths::with_home(
        temp.path().join("isolated-config"),
        temp.path().join("home"),
        temp.path().join("legacy/registry"),
        None,
    )
}

fn cwd(temp: &TempDir) -> PathBuf {
    let cwd = temp.path().join("ordinary host cwd");
    fs::create_dir_all(&cwd).unwrap();
    cwd
}

fn run(
    argv: &[&str],
    paths: &AppPaths,
    cwd: &std::path::Path,
    session: &SessionEnvironment,
    executor: &RecordingExecutor,
) -> Result<(AppOutcome, Vec<u8>, Vec<u8>), AppError> {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let outcome = run_with_session_executor(
        Cli::try_parse_from(argv).unwrap(),
        paths,
        cwd,
        executor,
        session,
        &mut stdout,
        &mut stderr,
    )?;
    Ok((outcome, stdout, stderr))
}

fn os(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}

#[test]
fn prune_routes_build_exact_deterministic_argv_and_execute_once_with_inherited_stdio() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = cwd(&temp);
    let cases = [
        (
            vec!["dstack", "container", "prune"],
            vec!["container", "prune"],
        ),
        (
            vec!["dstack", "container", "prune", "--force"],
            vec!["container", "prune", "--force"],
        ),
        (
            vec![
                "dstack",
                "container",
                "prune",
                "--filter",
                "label=a",
                "--filter",
                "until=24h",
                "--filter",
                "label=b",
                "--force",
            ],
            vec![
                "container",
                "prune",
                "--filter",
                "label=a",
                "--filter",
                "until=24h",
                "--filter",
                "label=b",
                "--force",
            ],
        ),
        (vec!["dstack", "image", "prune"], vec!["image", "prune"]),
        (
            vec!["dstack", "image", "prune", "--all"],
            vec!["image", "prune", "--all"],
        ),
        (
            vec!["dstack", "image", "prune", "--force"],
            vec!["image", "prune", "--force"],
        ),
        (
            vec!["dstack", "image", "prune", "--all", "--force"],
            vec!["image", "prune", "--all", "--force"],
        ),
        (
            vec![
                "dstack",
                "image",
                "prune",
                "--filter",
                "label=foo=bar",
                "--all",
                "--filter",
                "label!=keep",
                "--force",
            ],
            vec![
                "image",
                "prune",
                "--all",
                "--filter",
                "label=foo=bar",
                "--filter",
                "label!=keep",
                "--force",
            ],
        ),
        (vec!["dstack", "volume", "prune"], vec!["volume", "prune"]),
        (
            vec!["dstack", "volume", "prune", "--all"],
            vec!["volume", "prune", "--all"],
        ),
        (
            vec!["dstack", "volume", "prune", "--force"],
            vec!["volume", "prune", "--force"],
        ),
        (
            vec!["dstack", "volume", "prune", "--all", "--force"],
            vec!["volume", "prune", "--all", "--force"],
        ),
        (
            vec![
                "dstack",
                "volume",
                "prune",
                "--filter",
                "label=temporary",
                "--all",
                "--filter",
                "label=project=dstack",
                "--force",
            ],
            vec![
                "volume",
                "prune",
                "--all",
                "--filter",
                "label=temporary",
                "--filter",
                "label=project=dstack",
                "--force",
            ],
        ),
        (vec!["dstack", "network", "prune"], vec!["network", "prune"]),
        (
            vec!["dstack", "network", "prune", "--force"],
            vec!["network", "prune", "--force"],
        ),
        (
            vec![
                "dstack",
                "network",
                "prune",
                "--filter",
                "until=168h",
                "--filter",
                "label!=keep",
                "--force",
            ],
            vec![
                "network",
                "prune",
                "--filter",
                "until=168h",
                "--filter",
                "label!=keep",
                "--force",
            ],
        ),
    ];

    for (argv, expected) in cases {
        let executor = RecordingExecutor::default();
        let (outcome, stdout, stderr) = run(
            &argv,
            &paths,
            &cwd,
            &SessionEnvironment::absent(),
            &executor,
        )
        .unwrap();
        assert_eq!(
            outcome,
            AppOutcome::Child(ProcessStatus::from_code(0)),
            "{argv:?}"
        );
        assert!(stdout.is_empty(), "{argv:?}");
        assert!(stderr.is_empty(), "{argv:?}");
        let requests = executor.requests();
        assert_eq!(requests.len(), 1, "{argv:?}");
        assert_eq!(requests[0].program, OsString::from("docker"), "{argv:?}");
        assert_eq!(requests[0].args, os(&expected), "{argv:?}");
        assert_eq!(requests[0].current_dir, cwd, "{argv:?}");
        assert_eq!(requests[0].stdio, StdioPolicy::Inherit, "{argv:?}");
        assert!(executor.capture_requests().is_empty(), "{argv:?}");
        assert!(!paths.application_dir().exists(), "{argv:?}");
    }
}

#[test]
fn every_prune_propagates_child_status_and_typed_spawn_failure() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = cwd(&temp);
    let commands = [
        ["dstack", "container", "prune"].as_slice(),
        ["dstack", "image", "prune"].as_slice(),
        ["dstack", "volume", "prune"].as_slice(),
        ["dstack", "network", "prune"].as_slice(),
    ];

    for (index, argv) in commands.into_iter().enumerate() {
        let executor = RecordingExecutor::default();
        let code = 51 + i32::try_from(index).unwrap();
        executor.push_status(code);
        assert_eq!(
            run(argv, &paths, &cwd, &SessionEnvironment::absent(), &executor)
                .unwrap()
                .0,
            AppOutcome::Child(ProcessStatus::from_code(code))
        );
        assert_eq!(executor.requests().len(), 1);
        assert!(executor.capture_requests().is_empty());

        let executor = RecordingExecutor::default();
        executor.push_spawn_error(io::ErrorKind::NotFound, "docker missing for prune");
        let error = run(argv, &paths, &cwd, &SessionEnvironment::absent(), &executor).unwrap_err();
        assert!(matches!(error, AppError::Process(_)), "{argv:?}");
        assert!(error.to_string().contains("docker missing for prune"));
        assert_eq!(executor.requests().len(), 1);
        assert!(executor.capture_requests().is_empty());
    }
}

#[test]
fn prune_ignores_malformed_session_and_never_initializes_dstack_config() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = cwd(&temp);
    let malformed = SessionEnvironment::with_value("future-or-invalid-session-state");

    for argv in [
        ["dstack", "container", "prune"].as_slice(),
        ["dstack", "image", "prune", "--all"].as_slice(),
        ["dstack", "volume", "prune", "--all"].as_slice(),
        ["dstack", "network", "prune"].as_slice(),
    ] {
        let executor = RecordingExecutor::default();
        let (outcome, stdout, stderr) = run(argv, &paths, &cwd, &malformed, &executor).unwrap();
        assert_eq!(
            outcome,
            AppOutcome::Child(ProcessStatus::from_code(0)),
            "{argv:?}"
        );
        assert!(stdout.is_empty(), "{argv:?}");
        assert!(stderr.is_empty(), "{argv:?}");
        assert_eq!(executor.requests().len(), 1, "{argv:?}");
        assert!(executor.capture_requests().is_empty(), "{argv:?}");
        assert!(!paths.application_dir().exists(), "{argv:?}");
        assert!(!paths.config_file().exists(), "{argv:?}");
        assert!(!paths.registry_file().exists(), "{argv:?}");
        assert!(!paths.registry_lock().exists(), "{argv:?}");
    }
}

#[test]
fn prune_rejects_root_stack_and_file_before_bootstrap_or_docker_execution() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = cwd(&temp);
    let commands = [
        ["container", "prune"].as_slice(),
        ["image", "prune"].as_slice(),
        ["volume", "prune"].as_slice(),
        ["network", "prune"].as_slice(),
    ];

    for command in commands {
        for option in [
            ["--stack", "app"].as_slice(),
            ["--file", "compose.yaml"].as_slice(),
        ] {
            let argv = [vec!["dstack"], option.to_vec(), command.to_vec()].concat();
            let executor = RecordingExecutor::default();
            let error = run(
                &argv,
                &paths,
                &cwd,
                &SessionEnvironment::with_value("invalid-session"),
                &executor,
            )
            .unwrap_err();
            assert!(matches!(error, AppError::InvalidOptions(_)), "{argv:?}");
            assert!(executor.requests().is_empty(), "{argv:?}");
            assert!(executor.capture_requests().is_empty(), "{argv:?}");
            assert!(!paths.application_dir().exists(), "{argv:?}");
        }
    }
}
