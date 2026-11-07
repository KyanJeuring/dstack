use std::{ffi::OsString, fs};

use tempfile::TempDir;

use super::run_with_executor;
use crate::{
    AppError, AppOutcome, Cli,
    config::AppPaths,
    process::{CapturePolicy, ProcessStatus, testing::RecordingExecutor},
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
    executor: &RecordingExecutor,
    stdout: &mut Vec<u8>,
) -> Result<AppOutcome, AppError> {
    run_with_executor(
        Cli::try_parse_from(argv).unwrap(),
        paths,
        cwd,
        executor,
        stdout,
        &mut Vec::new(),
    )
}

#[test]
fn host_container_list_skips_stack_bootstrap_and_uses_normal_process_cwd() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = temp.path().join("ordinary cwd without compose");
    fs::create_dir_all(&cwd).unwrap();
    let executor = RecordingExecutor::default();
    executor.push_capture(b"abc\t\tstandalone\tUp 1 minute\t\n".to_vec(), 0);
    let mut stdout = Vec::new();

    let outcome = run(
        &["dstack", "container", "list"],
        &paths,
        &cwd,
        &executor,
        &mut stdout,
    )
    .unwrap();

    assert_eq!(outcome, AppOutcome::Success);
    assert!(!paths.application_dir().exists());
    assert!(executor.requests().is_empty());
    let request = &executor.capture_requests()[0];
    assert_eq!(request.process.program, OsString::from("docker"));
    assert_eq!(request.process.current_dir, cwd);
    assert_eq!(request.policy, CapturePolicy::StdoutWithInheritedStderr);
    assert!(!request.process.args.contains(&OsString::from("compose")));
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        "ID   SERVICE  NAME        STATUS       PORTS\nabc  -        standalone  Up 1 minute  -\n"
    );
}

#[test]
fn container_list_cli_short_and_long_forms_preserve_exact_filters() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    for argv in [
        vec!["dstack", "container", "list", "--all"],
        vec!["dstack", "container", "list", "-a"],
        vec!["dstack", "container", "list", "--filter", "name=nginx"],
        vec!["dstack", "container", "list", "-f", "status=running"],
        vec![
            "dstack",
            "container",
            "list",
            "--filter",
            "name=foo",
            "--filter",
            "status=running",
        ],
    ] {
        let executor = RecordingExecutor::default();
        run(&argv, &paths, temp.path(), &executor, &mut Vec::new()).unwrap();
        let args = &executor.capture_requests()[0].process.args;
        assert_eq!(&args[..2], ["container", "ls"].map(OsString::from));
        assert!(args.ends_with(&[OsString::from("--format"), args.last().unwrap().clone()]));
        let expected_all = argv.contains(&"--all") || argv.contains(&"-a");
        assert_eq!(args.contains(&OsString::from("--all")), expected_all);
        let forwarded = args
            .windows(2)
            .filter(|pair| pair[0] == "--filter")
            .map(|pair| pair[1].to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let expected = argv
            .windows(2)
            .filter(|pair| pair[0] == "--filter" || pair[0] == "-f")
            .map(|pair| pair[1].to_owned())
            .collect::<Vec<_>>();
        assert_eq!(forwarded, expected);
    }
}

#[test]
fn host_scoped_stack_and_file_options_fail_before_bootstrap_or_execution() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    for argv in [
        vec!["dstack", "-s", "foo", "container", "list"],
        vec!["dstack", "--stack", "foo", "image", "list"],
        vec!["dstack", "container", "list", "-s", "foo"],
        vec!["dstack", "volume", "list", "--file", "compose.yaml"],
        vec!["dstack", "network", "list", "--stack", "foo"],
    ] {
        let executor = RecordingExecutor::default();
        let error = run(&argv, &paths, temp.path(), &executor, &mut Vec::new()).unwrap_err();
        assert!(matches!(error, AppError::InvalidOptions(_)));
        assert!(error.to_string().contains("commands are host-scoped"));
        assert!(error.to_string().contains("--stack or --file"));
        assert!(executor.requests().is_empty());
        assert!(executor.capture_requests().is_empty());
        assert!(!paths.application_dir().exists());
    }
}

#[test]
fn all_host_namespaces_route_directly_without_compose_or_bootstrap() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    for (argv, prefix, captured) in [
        (vec!["dstack", "image", "list"], vec!["image", "ls"], true),
        (vec!["dstack", "volume", "list"], vec!["volume", "ls"], true),
        (
            vec!["dstack", "network", "list"],
            vec!["network", "ls"],
            true,
        ),
        (
            vec!["dstack", "volume", "remove", "app_data", "cache"],
            vec!["volume", "rm"],
            false,
        ),
        (
            vec!["dstack", "network", "inspect", "bridge"],
            vec!["network", "inspect"],
            false,
        ),
    ] {
        let executor = RecordingExecutor::default();
        run(&argv, &paths, temp.path(), &executor, &mut Vec::new()).unwrap();
        let args = if captured {
            &executor.capture_requests()[0].process.args
        } else {
            &executor.requests()[0].args
        };
        assert_eq!(
            &args[..2],
            prefix.into_iter().map(OsString::from).collect::<Vec<_>>()
        );
        assert!(!args.contains(&OsString::from("compose")));
        assert!(!paths.application_dir().exists());
    }
}

#[test]
fn host_streaming_status_propagates_through_application_outcome() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let executor = RecordingExecutor::default();
    executor.push_status(37);

    let outcome = run(
        &["dstack", "container", "inspect", "nginx", "postgres"],
        &paths,
        temp.path(),
        &executor,
        &mut Vec::new(),
    )
    .unwrap();

    assert_eq!(outcome, AppOutcome::Child(ProcessStatus::from_code(37)));
    assert_eq!(
        executor.requests()[0].args,
        ["container", "inspect", "nginx", "postgres"].map(OsString::from)
    );
    assert!(!paths.application_dir().exists());
}

#[test]
fn host_capture_status_propagates_without_rendering_partial_stdout() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let executor = RecordingExecutor::default();
    executor.push_capture(b"partial\tmalformed\n".to_vec(), 17);
    let mut stdout = Vec::new();

    let outcome = run(
        &["dstack", "container", "list"],
        &paths,
        temp.path(),
        &executor,
        &mut stdout,
    )
    .unwrap();

    assert_eq!(outcome, AppOutcome::Child(ProcessStatus::from_code(17)));
    assert!(stdout.is_empty());
}
