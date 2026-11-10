use std::{ffi::OsString, fs, io, path::PathBuf};

use tempfile::TempDir;

use super::run_with_session_executor;
use crate::{
    AppError, AppOutcome, Cli,
    config::AppPaths,
    docker::TARGET_BATCH_SIZE,
    process::{CapturePolicy, ProcessStatus, StdioPolicy, testing::RecordingExecutor},
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
fn system_routes_stream_exact_argv_once_with_inherited_stdio() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = cwd(&temp);
    let cases = [
        (vec!["dstack", "system", "df"], vec!["system", "df"]),
        (
            vec!["dstack", "system", "df", "--verbose"],
            vec!["system", "df", "--verbose"],
        ),
        (vec!["dstack", "system", "prune"], vec!["system", "prune"]),
        (
            vec!["dstack", "system", "prune", "--all"],
            vec!["system", "prune", "--all"],
        ),
        (
            vec!["dstack", "system", "prune", "--volumes"],
            vec!["system", "prune", "--volumes"],
        ),
        (
            vec!["dstack", "system", "prune", "--force"],
            vec!["system", "prune", "--force"],
        ),
        (
            vec![
                "dstack",
                "system",
                "prune",
                "--filter",
                "label=a",
                "--filter",
                "until=24h",
                "--filter",
                "label=b",
            ],
            vec![
                "system",
                "prune",
                "--filter",
                "label=a",
                "--filter",
                "until=24h",
                "--filter",
                "label=b",
            ],
        ),
        (
            vec![
                "dstack",
                "system",
                "prune",
                "--all",
                "--volumes",
                "--filter",
                "label=a",
                "--filter",
                "label=a",
                "--force",
            ],
            vec![
                "system",
                "prune",
                "--all",
                "--volumes",
                "--filter",
                "label=a",
                "--filter",
                "label=a",
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
        assert_eq!(outcome, AppOutcome::Child(ProcessStatus::from_code(0)));
        assert!(stdout.is_empty());
        assert!(stderr.is_empty());
        let requests = executor.requests();
        assert_eq!(requests.len(), 1, "{argv:?}");
        assert_eq!(requests[0].program, OsString::from("docker"));
        assert_eq!(requests[0].args, os(&expected));
        assert_eq!(requests[0].current_dir, cwd);
        assert_eq!(requests[0].stdio, StdioPolicy::Inherit);
        assert!(executor.capture_requests().is_empty());
        assert!(!paths.application_dir().exists());
    }
}

#[test]
fn system_commands_propagate_child_status_and_spawn_errors() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = cwd(&temp);
    for argv in [
        ["dstack", "system", "df"].as_slice(),
        ["dstack", "system", "prune"].as_slice(),
    ] {
        let executor = RecordingExecutor::default();
        executor.push_status(57);
        assert_eq!(
            run(argv, &paths, &cwd, &SessionEnvironment::absent(), &executor)
                .unwrap()
                .0,
            AppOutcome::Child(ProcessStatus::from_code(57))
        );

        let executor = RecordingExecutor::default();
        executor.push_spawn_error(io::ErrorKind::NotFound, "docker missing for system command");
        let error = run(argv, &paths, &cwd, &SessionEnvironment::absent(), &executor).unwrap_err();
        assert!(matches!(error, AppError::Process(_)));
        assert!(
            error
                .to_string()
                .contains("docker missing for system command")
        );
        assert_eq!(executor.requests().len(), 1);
        assert!(executor.capture_requests().is_empty());
    }
}

#[test]
fn targeted_container_stop_streams_exact_operands_without_enumeration() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = cwd(&temp);
    let executor = RecordingExecutor::default();
    executor.push_status(31);
    let (outcome, stdout, stderr) = run(
        &["dstack", "container", "stop", "web", "db"],
        &paths,
        &cwd,
        &SessionEnvironment::with_value("malformed-session"),
        &executor,
    )
    .unwrap();

    assert_eq!(outcome, AppOutcome::Child(ProcessStatus::from_code(31)));
    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
    let requests = executor.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].args, os(&["container", "stop", "web", "db"]));
    assert_eq!(requests[0].stdio, StdioPolicy::Inherit);
    assert!(executor.capture_requests().is_empty());
    assert!(!paths.application_dir().exists());

    let executor = RecordingExecutor::default();
    executor.push_spawn_error(io::ErrorKind::PermissionDenied, "named stop denied");
    let error = run(
        &["dstack", "container", "stop", "web"],
        &paths,
        &cwd,
        &SessionEnvironment::absent(),
        &executor,
    )
    .unwrap_err();
    assert!(matches!(error, AppError::Process(_)));
    assert!(error.to_string().contains("named stop denied"));
    assert_eq!(executor.requests().len(), 1);
    assert!(executor.capture_requests().is_empty());
}

#[test]
fn stop_all_enumerates_only_running_containers_and_handles_empty_success() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = cwd(&temp);
    let executor = RecordingExecutor::default();
    executor.push_capture(Vec::new(), 0);
    let (outcome, stdout, stderr) = run(
        &["dstack", "container", "stop", "--all"],
        &paths,
        &cwd,
        &SessionEnvironment::absent(),
        &executor,
    )
    .unwrap();

    assert_eq!(outcome, AppOutcome::Success);
    assert!(stdout.is_empty());
    assert_eq!(stderr, b"[INFO] No running containers to stop.\n");
    assert!(executor.requests().is_empty());
    let captures = executor.capture_requests();
    assert_eq!(captures.len(), 1);
    assert_eq!(
        captures[0].process.args,
        os(&["container", "ls", "--quiet", "--no-trunc"])
    );
    assert!(!captures[0].process.args.contains(&OsString::from("--all")));
    assert_eq!(captures[0].policy, CapturePolicy::StdoutWithInheritedStderr);
}

#[test]
fn stop_all_deduplicates_crlf_identifiers_and_streams_one_bounded_batch() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = cwd(&temp);
    let executor = RecordingExecutor::default();
    executor.push_capture(b"id-a\r\nid-b\r\nid-a\r\n\r\nid-c\r\nid-b\r\n".to_vec(), 0);
    let outcome = run(
        &["dstack", "container", "stop", "--all"],
        &paths,
        &cwd,
        &SessionEnvironment::absent(),
        &executor,
    )
    .unwrap()
    .0;

    assert_eq!(outcome, AppOutcome::Child(ProcessStatus::from_code(0)));
    let requests = executor.requests();
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0].args,
        os(&["container", "stop", "id-a", "id-b", "id-c"])
    );
    assert_eq!(requests[0].stdio, StdioPolicy::Inherit);
}

#[test]
fn stop_all_batches_every_identifier_once_in_first_seen_order() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = cwd(&temp);
    let identifiers = (0..TARGET_BATCH_SIZE * 2 + 3)
        .map(|index| format!("id-{index:03}"))
        .collect::<Vec<_>>();
    let executor = RecordingExecutor::default();
    executor.push_capture(format!("{}\n", identifiers.join("\n")), 0);
    let outcome = run(
        &["dstack", "container", "stop", "--all"],
        &paths,
        &cwd,
        &SessionEnvironment::absent(),
        &executor,
    )
    .unwrap()
    .0;

    assert_eq!(outcome, AppOutcome::Child(ProcessStatus::from_code(0)));
    let requests = executor.requests();
    assert_eq!(requests.len(), 3);
    let stopped = requests
        .iter()
        .flat_map(|request| {
            assert_eq!(request.program, OsString::from("docker"));
            assert_eq!(&request.args[..2], os(&["container", "stop"]));
            assert!(request.args.len() - 2 <= TARGET_BATCH_SIZE);
            assert_eq!(request.stdio, StdioPolicy::Inherit);
            request.args[2..]
                .iter()
                .map(|value| value.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(stopped, identifiers);
}

#[test]
fn stop_all_enumeration_failure_and_malformed_output_never_stop_targets() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = cwd(&temp);

    let executor = RecordingExecutor::default();
    executor.push_capture(b"ignored\n".to_vec(), 43);
    assert_eq!(
        run(
            &["dstack", "container", "stop", "--all"],
            &paths,
            &cwd,
            &SessionEnvironment::absent(),
            &executor,
        )
        .unwrap()
        .0,
        AppOutcome::Child(ProcessStatus::from_code(43))
    );
    assert!(executor.requests().is_empty());

    let executor = RecordingExecutor::default();
    executor.push_capture(b"valid-id\nbad id\n".to_vec(), 0);
    assert!(matches!(
        run(
            &["dstack", "container", "stop", "--all"],
            &paths,
            &cwd,
            &SessionEnvironment::absent(),
            &executor,
        )
        .unwrap_err(),
        AppError::DockerOutput(_)
    ));
    assert!(executor.requests().is_empty());
}

#[test]
fn stop_all_stops_after_first_failed_batch_and_returns_its_status() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = cwd(&temp);
    let identifiers = (0..TARGET_BATCH_SIZE * 2 + 1)
        .map(|index| format!("id-{index:03}"))
        .collect::<Vec<_>>();
    let executor = RecordingExecutor::default();
    executor.push_capture(format!("{}\n", identifiers.join("\n")), 0);
    executor.push_status(0);
    executor.push_status(52);
    executor.push_status(0);
    let outcome = run(
        &["dstack", "container", "stop", "--all"],
        &paths,
        &cwd,
        &SessionEnvironment::absent(),
        &executor,
    )
    .unwrap()
    .0;

    assert_eq!(outcome, AppOutcome::Child(ProcessStatus::from_code(52)));
    assert_eq!(executor.requests().len(), 2);
    assert_eq!(executor.requests()[1].args.len() - 2, TARGET_BATCH_SIZE);
}

#[test]
fn stop_all_enumeration_and_stop_spawn_errors_remain_typed() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = cwd(&temp);

    let executor = RecordingExecutor::default();
    executor.push_capture_spawn_error(io::ErrorKind::NotFound, "enumeration docker missing");
    let error = run(
        &["dstack", "container", "stop", "--all"],
        &paths,
        &cwd,
        &SessionEnvironment::absent(),
        &executor,
    )
    .unwrap_err();
    assert!(matches!(error, AppError::Process(_)));
    assert!(executor.requests().is_empty());

    let executor = RecordingExecutor::default();
    executor.push_capture(b"id-a\n".to_vec(), 0);
    executor.push_spawn_error(io::ErrorKind::PermissionDenied, "stop denied");
    let error = run(
        &["dstack", "container", "stop", "--all"],
        &paths,
        &cwd,
        &SessionEnvironment::absent(),
        &executor,
    )
    .unwrap_err();
    assert!(matches!(error, AppError::Process(_)));
    assert_eq!(executor.requests().len(), 1);
}

#[test]
fn stop_all_uses_only_direct_docker_processes_without_prompt_or_shell() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = cwd(&temp);
    let executor = RecordingExecutor::default();
    executor.push_capture(b"id-a\nid-b\n".to_vec(), 0);
    let (_, stdout, stderr) = run(
        &["dstack", "container", "stop", "--all"],
        &paths,
        &cwd,
        &SessionEnvironment::absent(),
        &executor,
    )
    .unwrap();

    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
    for program in executor
        .capture_requests()
        .into_iter()
        .map(|request| request.process.program)
        .chain(
            executor
                .requests()
                .into_iter()
                .map(|request| request.program),
        )
    {
        assert_eq!(program, OsString::from("docker"));
        assert!(
            ![
                OsString::from("xargs"),
                OsString::from("sh"),
                OsString::from("bash"),
                OsString::from("cmd.exe"),
                OsString::from("powershell"),
            ]
            .contains(&program)
        );
    }
}

#[test]
fn phase8_4_host_routes_ignore_malformed_sessions_and_create_no_config() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = cwd(&temp);
    let malformed = SessionEnvironment::with_value("future-or-invalid-session-state");
    for argv in [
        ["dstack", "system", "df"].as_slice(),
        ["dstack", "system", "prune"].as_slice(),
        ["dstack", "container", "stop", "named"].as_slice(),
        ["dstack", "container", "stop", "--all"].as_slice(),
    ] {
        let executor = RecordingExecutor::default();
        if argv.ends_with(&["--all"]) {
            executor.push_capture(b"id-a\n".to_vec(), 0);
        }
        run(argv, &paths, &cwd, &malformed, &executor).unwrap();
        assert!(!paths.application_dir().exists(), "{argv:?}");
        assert!(!paths.config_file().exists(), "{argv:?}");
        assert!(!paths.registry_file().exists(), "{argv:?}");
        assert!(!paths.registry_lock().exists(), "{argv:?}");
    }
}

#[test]
fn phase8_4_host_routes_reject_root_stack_and_file_before_docker_or_bootstrap() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = cwd(&temp);
    let commands = [
        ["system", "df"].as_slice(),
        ["system", "prune"].as_slice(),
        ["container", "stop", "named"].as_slice(),
        ["container", "stop", "--all"].as_slice(),
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
            assert!(executor.requests().is_empty());
            assert!(executor.capture_requests().is_empty());
            assert!(!paths.application_dir().exists());
        }
    }
}
