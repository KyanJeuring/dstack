use std::{ffi::OsString, fs};

use serde_json::json;
use tempfile::TempDir;

use super::run_with_session_executor;
use crate::{
    AppError, AppOutcome, Cli,
    config::AppPaths,
    container::commands::ip,
    process::{CapturePolicy, StdioPolicy, testing::RecordingExecutor},
    session::SessionEnvironment,
};

fn paths(temp: &TempDir) -> AppPaths {
    AppPaths::with_home(
        temp.path().join("state"),
        temp.path().join("home"),
        temp.path().join("legacy/registry"),
        None,
    )
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

#[test]
fn inspect_all_public_routes_emit_one_json_array_and_never_stream() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = temp.path().join("ordinary host cwd");
    fs::create_dir_all(&cwd).unwrap();

    for (namespace, list_args, inspect_prefix) in [
        (
            "container",
            vec!["container", "ls", "--all", "--quiet", "--no-trunc"],
            vec!["container", "inspect"],
        ),
        (
            "image",
            vec!["image", "ls", "--all", "--quiet", "--no-trunc"],
            vec!["image", "inspect"],
        ),
        (
            "volume",
            vec!["volume", "ls", "--quiet"],
            vec!["volume", "inspect"],
        ),
        (
            "network",
            vec!["network", "ls", "--quiet", "--no-trunc"],
            vec!["network", "inspect"],
        ),
    ] {
        let executor = RecordingExecutor::default();
        executor.push_capture(b"first\nsecond\nfirst\n".to_vec(), 0);
        executor.push_capture(br#"[{"target":"first"},{"target":"second"}]"#.to_vec(), 0);
        let (outcome, stdout, stderr) = run(
            &["dstack", namespace, "inspect", "--all"],
            &paths,
            &cwd,
            &SessionEnvironment::absent(),
            &executor,
        )
        .unwrap();
        assert_eq!(outcome, AppOutcome::Success);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&stdout).unwrap(),
            json!([{"target":"first"},{"target":"second"}])
        );
        assert_eq!(stdout.last(), Some(&b'\n'));
        assert!(stderr.is_empty());
        assert!(executor.requests().is_empty());
        let requests = executor.capture_requests();
        assert_eq!(
            requests[0].process.args,
            list_args
                .into_iter()
                .map(OsString::from)
                .collect::<Vec<_>>()
        );
        assert_eq!(
            requests[1].process.args,
            inspect_prefix
                .into_iter()
                .chain(["first", "second"])
                .map(OsString::from)
                .collect::<Vec<_>>()
        );
        assert!(
            requests
                .iter()
                .all(|request| request.policy == CapturePolicy::StdoutWithInheritedStderr)
        );
        assert!(!paths.application_dir().exists());
    }
}

#[test]
fn new_all_operations_ignore_malformed_session_and_do_not_create_config() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let malformed = SessionEnvironment::with_value("not valid session json");

    for argv in [
        ["dstack", "container", "inspect", "--all"].as_slice(),
        ["dstack", "image", "inspect", "--all"].as_slice(),
        ["dstack", "volume", "inspect", "--all"].as_slice(),
        ["dstack", "network", "inspect", "--all"].as_slice(),
        ["dstack", "container", "ip", "--all"].as_slice(),
    ] {
        let executor = RecordingExecutor::default();
        let (outcome, stdout, stderr) =
            run(argv, &paths, temp.path(), &malformed, &executor).unwrap();
        assert_eq!(outcome, AppOutcome::Success, "{argv:?}");
        if argv[2] == "ip" {
            assert_eq!(stdout, b"CONTAINER  NETWORK  IP\n");
        } else {
            assert_eq!(stdout, b"[]\n");
        }
        assert!(stderr.is_empty(), "{argv:?}");
        assert_eq!(executor.capture_requests().len(), 1, "{argv:?}");
        assert!(executor.requests().is_empty(), "{argv:?}");
        assert!(!paths.application_dir().exists(), "{argv:?}");
        assert!(!paths.config_file().exists(), "{argv:?}");
        assert!(!paths.registry_file().exists(), "{argv:?}");
        assert!(!paths.registry_lock().exists(), "{argv:?}");
    }
}

#[test]
fn new_all_operations_reject_root_stack_and_file_before_docker() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let commands = [
        ["container", "inspect", "--all"].as_slice(),
        ["image", "inspect", "--all"].as_slice(),
        ["volume", "inspect", "--all"].as_slice(),
        ["network", "inspect", "--all"].as_slice(),
        ["container", "ip", "--all"].as_slice(),
    ];

    for command in commands {
        for option in [
            ["--stack", "stack"].as_slice(),
            ["--file", "compose.yaml"].as_slice(),
        ] {
            let argv = [vec!["dstack"], option.to_vec(), command.to_vec()].concat();
            let executor = RecordingExecutor::default();
            let error = run(
                &argv,
                &paths,
                temp.path(),
                &SessionEnvironment::absent(),
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

#[test]
fn every_named_inspect_still_streams_exact_native_argv() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    for (argv, expected) in [
        (
            vec!["dstack", "container", "inspect", "c1", "c2"],
            vec!["container", "inspect", "c1", "c2"],
        ),
        (
            vec!["dstack", "image", "inspect", "i1", "i2"],
            vec!["image", "inspect", "i1", "i2"],
        ),
        (
            vec!["dstack", "volume", "inspect", "v1", "v2"],
            vec!["volume", "inspect", "v1", "v2"],
        ),
        (
            vec!["dstack", "network", "inspect", "n1", "n2"],
            vec!["network", "inspect", "n1", "n2"],
        ),
    ] {
        let executor = RecordingExecutor::default();
        run(
            &argv,
            &paths,
            temp.path(),
            &SessionEnvironment::absent(),
            &executor,
        )
        .unwrap();
        assert_eq!(executor.requests().len(), 1, "{argv:?}");
        assert_eq!(
            executor.requests()[0].args,
            expected.into_iter().map(OsString::from).collect::<Vec<_>>()
        );
        assert_eq!(executor.requests()[0].stdio, StdioPolicy::Inherit);
        assert!(executor.capture_requests().is_empty(), "{argv:?}");
        assert!(!paths.application_dir().exists(), "{argv:?}");
    }
}

#[test]
fn specific_container_ip_keeps_its_targeted_formatted_inspect_path() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let executor = RecordingExecutor::default();
    executor.push_capture(b"bridge\t172.17.0.2\n".to_vec(), 0);
    let (outcome, stdout, _) = run(
        &["dstack", "container", "ip", "web"],
        &paths,
        temp.path(),
        &SessionEnvironment::absent(),
        &executor,
    )
    .unwrap();
    assert_eq!(outcome, AppOutcome::Success);
    assert_eq!(stdout, b"NETWORK  IP\nbridge   172.17.0.2\n");
    assert_eq!(
        executor.capture_requests()[0].process.args,
        ["container", "inspect", "--format", ip::FORMAT, "web"].map(OsString::from)
    );
    assert_eq!(executor.capture_requests().len(), 1);
    assert!(executor.requests().is_empty());
}
