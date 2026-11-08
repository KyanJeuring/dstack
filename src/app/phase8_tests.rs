use std::{ffi::OsString, fs, io, path::PathBuf};

use tempfile::TempDir;

use super::run_with_session_executor;
use crate::{
    AppError, AppOutcome, Cli,
    config::AppPaths,
    process::{CapturePolicy, ProcessStatus, testing::RecordingExecutor},
    registry::{RegistryStore, StackProfile},
    session::{SESSION_SCHEMA_VERSION, SessionEnvironment, SessionOrigin, SessionState},
};

const WEB_1: &str = "aaaaaaaaaaaa";
const WEB_2: &str = "bbbbbbbbbbbb";
const DB_1: &str = "cccccccccccc";
const REDIS_1: &str = "dddddddddddd";

#[derive(Clone, Copy)]
struct Container<'a> {
    id: &'a str,
    service: &'a str,
    status: &'a str,
    running: bool,
    paused: bool,
    restarting: bool,
}

fn paths(temp: &TempDir) -> AppPaths {
    AppPaths::with_home(
        temp.path().join("state"),
        temp.path().join("home"),
        temp.path().join("legacy/registry"),
        None,
    )
}

fn project(parent: &std::path::Path, name: &str, files: &[&str]) -> PathBuf {
    let path = parent.join(name);
    fs::create_dir_all(&path).unwrap();
    for file in files {
        fs::write(path.join(file), "services: {}\n").unwrap();
    }
    fs::canonicalize(path).unwrap()
}

fn session(name: &str, project: &std::path::Path, files: &[&str]) -> SessionEnvironment {
    SessionEnvironment::with_value(
        SessionState {
            version: SESSION_SCHEMA_VERSION,
            name: name.to_owned(),
            project: project.to_path_buf(),
            files: files.iter().map(|file| project.join(file)).collect(),
            origin: SessionOrigin::Registered,
        }
        .serialize()
        .unwrap(),
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

fn running(id: &'static str, service: &'static str) -> Container<'static> {
    Container {
        id,
        service,
        status: "running",
        running: true,
        paused: false,
        restarting: false,
    }
}

fn stopped(id: &'static str, service: &'static str) -> Container<'static> {
    Container {
        id,
        service,
        status: "exited",
        running: false,
        paused: false,
        restarting: false,
    }
}

fn queue_fixture(executor: &RecordingExecutor, configured: &[&str], containers: &[Container<'_>]) {
    executor.push_capture(format!("{}\n", configured.join("\n")), 0);
    if containers.is_empty() {
        executor.push_capture(Vec::new(), 0);
        return;
    }
    executor.push_capture(
        format!(
            "{}\n",
            containers
                .iter()
                .map(|container| container.id)
                .collect::<Vec<_>>()
                .join("\n")
        ),
        0,
    );
    let objects = containers
        .iter()
        .map(|container| {
            format!(
                r#"{{"Id":"{}","Config":{{"Labels":{{"com.docker.compose.service":"{}","com.docker.compose.oneoff":"False"}}}},"State":{{"Status":"{}","Running":{},"Paused":{},"Restarting":{},"Health":{{"Status":"unhealthy"}}}}}}"#,
                container.id,
                container.service,
                container.status,
                container.running,
                container.paused,
                container.restarting,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    executor.push_capture(format!("[{objects}]"), 0);
}

#[test]
fn start_already_running_messages_cover_whole_one_and_multiple_service_targets() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = project(temp.path(), "jellyfin", &["compose.yaml"]);

    for (argv, configured, containers, expected) in [
        (
            vec!["dstack", "start"],
            vec!["web", "db", "redis"],
            vec![
                running(WEB_1, "web"),
                running(DB_1, "db"),
                running(REDIS_1, "redis"),
            ],
            "[INFO] Stack 'jellyfin' is already running.\n",
        ),
        (
            vec!["dstack", "start", "web"],
            vec!["web", "db"],
            vec![running(WEB_1, "web")],
            "[INFO] Service 'web' is already running.\n",
        ),
        (
            vec!["dstack", "start", "web", "redis"],
            vec!["web", "redis", "db"],
            vec![running(WEB_1, "web"), running(REDIS_1, "redis")],
            "[INFO] Requested services are already running: web, redis\n",
        ),
    ] {
        let executor = RecordingExecutor::default();
        queue_fixture(&executor, &configured, &containers);
        let (outcome, stdout, stderr) = run(
            &argv,
            &paths,
            &cwd,
            &SessionEnvironment::absent(),
            &executor,
        )
        .unwrap();
        assert_eq!(outcome, AppOutcome::Success);
        assert!(stdout.is_empty());
        assert_eq!(String::from_utf8(stderr).unwrap(), expected);
        assert!(executor.requests().is_empty());
    }
}

#[test]
fn start_mixed_scaled_missing_and_unknown_targets_execute_the_original_command() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = project(temp.path(), "project", &["compose.yaml"]);

    let cases = [
        (
            vec!["dstack", "start", "web", "redis"],
            vec!["web", "redis"],
            vec![running(WEB_1, "web"), stopped(REDIS_1, "redis")],
            vec!["start", "web", "redis"],
        ),
        (
            vec!["dstack", "start", "web"],
            vec!["web"],
            vec![running(WEB_1, "web"), stopped(WEB_2, "web")],
            vec!["start", "web"],
        ),
        (
            vec!["dstack", "start"],
            vec!["web", "db"],
            vec![running(WEB_1, "web")],
            vec!["start"],
        ),
        (
            vec!["dstack", "start", "db"],
            vec!["web", "db"],
            vec![],
            vec!["start", "db"],
        ),
    ];
    for (argv, configured, containers, expected) in cases {
        let executor = RecordingExecutor::default();
        queue_fixture(&executor, &configured, &containers);
        executor.push_status(19);
        let (outcome, stdout, stderr) = run(
            &argv,
            &paths,
            &cwd,
            &SessionEnvironment::absent(),
            &executor,
        )
        .unwrap();
        assert_eq!(outcome, AppOutcome::Child(ProcessStatus::from_code(19)));
        assert!(stdout.is_empty());
        assert!(stderr.is_empty());
        assert_eq!(executor.requests().len(), 1);
        assert!(
            executor.requests()[0]
                .args
                .ends_with(&expected.into_iter().map(OsString::from).collect::<Vec<_>>())
        );
    }

    let executor = RecordingExecutor::default();
    executor.push_capture(b"web\n".to_vec(), 0);
    executor.push_status(21);
    let (outcome, _, stderr) = run(
        &["dstack", "start", "does-not-exist"],
        &paths,
        &cwd,
        &SessionEnvironment::absent(),
        &executor,
    )
    .unwrap();
    assert_eq!(outcome, AppOutcome::Child(ProcessStatus::from_code(21)));
    assert!(stderr.is_empty());
    assert!(
        executor.requests()[0]
            .args
            .ends_with(&[OsString::from("start"), OsString::from("does-not-exist")])
    );
}

#[test]
fn stop_already_stopped_requires_existing_complete_explicit_targets() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = project(temp.path(), "jellyfin", &["compose.yaml"]);

    for (argv, configured, containers, expected) in [
        (
            vec!["dstack", "stop"],
            vec!["web", "db"],
            vec![stopped(WEB_1, "web"), stopped(DB_1, "db")],
            "[INFO] Stack 'jellyfin' is already stopped.\n",
        ),
        (
            vec!["dstack", "stop", "web", "redis"],
            vec!["web", "redis", "db"],
            vec![stopped(WEB_1, "web"), stopped(REDIS_1, "redis")],
            "[INFO] Requested services are already stopped: web, redis\n",
        ),
        (
            vec!["dstack", "stop", "web"],
            vec!["web"],
            vec![stopped(WEB_1, "web"), stopped(WEB_2, "web")],
            "[INFO] Service 'web' is already stopped.\n",
        ),
    ] {
        let executor = RecordingExecutor::default();
        queue_fixture(&executor, &configured, &containers);
        let (outcome, stdout, stderr) = run(
            &argv,
            &paths,
            &cwd,
            &SessionEnvironment::absent(),
            &executor,
        )
        .unwrap();
        assert_eq!(outcome, AppOutcome::Success);
        assert!(stdout.is_empty());
        assert_eq!(String::from_utf8(stderr).unwrap(), expected);
        assert!(executor.requests().is_empty());
    }
}

#[test]
fn stop_zero_mixed_scaled_paused_and_restarting_targets_execute_normally() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = project(temp.path(), "project", &["compose.yaml"]);
    let paused = Container {
        paused: true,
        ..running(WEB_1, "web")
    };
    let restarting = Container {
        status: "restarting",
        restarting: true,
        ..running(WEB_1, "web")
    };
    let cases = [
        (vec!["dstack", "stop"], vec![]),
        (vec!["dstack", "stop", "redis"], vec![]),
        (
            vec!["dstack", "stop", "web", "redis"],
            vec![stopped(WEB_1, "web"), running(REDIS_1, "redis")],
        ),
        (
            vec!["dstack", "stop", "web"],
            vec![stopped(WEB_1, "web"), running(WEB_2, "web")],
        ),
        (vec!["dstack", "stop", "web"], vec![paused]),
        (vec!["dstack", "stop", "web"], vec![restarting]),
    ];
    for (argv, containers) in cases {
        let executor = RecordingExecutor::default();
        queue_fixture(&executor, &["web", "redis"], &containers);
        executor.push_status(22);
        let (outcome, _, stderr) = run(
            &argv,
            &paths,
            &cwd,
            &SessionEnvironment::absent(),
            &executor,
        )
        .unwrap();
        assert_eq!(outcome, AppOutcome::Child(ProcessStatus::from_code(22)));
        assert!(stderr.is_empty());
        assert_eq!(executor.requests().len(), 1);
        assert!(
            executor.requests()[0]
                .args
                .contains(&OsString::from("stop"))
        );
    }
}

#[test]
fn targeted_host_commands_ignore_malformed_session_state() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let malformed = SessionEnvironment::with_value("not valid session json");
    for argv in [
        ["dstack", "container", "remove", "web"].as_slice(),
        ["dstack", "image", "inspect", "img"].as_slice(),
        ["dstack", "image", "remove", "img"].as_slice(),
        ["dstack", "network", "remove", "frontend"].as_slice(),
    ] {
        let executor = RecordingExecutor::default();
        run(argv, &paths, temp.path(), &malformed, &executor).unwrap();
        assert_eq!(executor.requests().len(), 1, "{argv:?}");
        assert!(executor.capture_requests().is_empty(), "{argv:?}");
        assert!(!paths.application_dir().exists(), "{argv:?}");
    }
}

#[test]
fn probe_failures_are_silent_and_the_lifecycle_command_remains_authoritative() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = project(temp.path(), "project", &["compose.yaml"]);

    for command in ["start", "stop"] {
        for (stdout, code) in [
            (b"probe stdout must not leak".to_vec(), 17),
            (b"malformed probe output".to_vec(), 0),
        ] {
            let executor = RecordingExecutor::default();
            executor.push_capture(stdout, code);
            executor.push_status(37);
            let (outcome, stdout, stderr) = run(
                &["dstack", command],
                &paths,
                &cwd,
                &SessionEnvironment::absent(),
                &executor,
            )
            .unwrap();
            assert_eq!(outcome, AppOutcome::Child(ProcessStatus::from_code(37)));
            assert!(stdout.is_empty());
            assert!(stderr.is_empty());
            assert_eq!(executor.requests().len(), 1);
        }
    }

    let executor = RecordingExecutor::default();
    executor.push_capture_spawn_error(io::ErrorKind::NotFound, "probe docker missing");
    executor.push_spawn_error(io::ErrorKind::NotFound, "canonical docker missing");
    let error = run(
        &["dstack", "start"],
        &paths,
        &cwd,
        &SessionEnvironment::absent(),
        &executor,
    )
    .unwrap_err();
    assert!(matches!(error, AppError::Process(_)));
    assert!(error.to_string().contains("canonical docker missing"));
    assert!(!error.to_string().contains("probe docker missing"));
}

#[test]
fn state_probe_and_fallback_share_explicit_session_and_cwd_contexts() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = project(temp.path(), "cwd", &["compose.yaml"]);
    let explicit = project(
        temp.path(),
        "explicit",
        &["compose.yaml", "compose.prod.yaml"],
    );
    let active = project(
        temp.path(),
        "active",
        &["compose.yaml", "compose.session.yaml"],
    );
    RegistryStore::new(
        paths.registry_file().to_path_buf(),
        paths.registry_lock().to_path_buf(),
    )
    .add(
        StackProfile::for_registration(
            "explicit".to_owned(),
            explicit.clone(),
            vec![
                PathBuf::from("compose.yaml"),
                PathBuf::from("compose.prod.yaml"),
            ],
        )
        .unwrap(),
    )
    .unwrap();
    let active_session = session("active", &active, &["compose.yaml", "compose.session.yaml"]);

    for (argv, session, expected_project, files) in [
        (
            vec!["dstack", "-s", "explicit", "start", "web"],
            SessionEnvironment::absent(),
            explicit.clone(),
            vec![
                explicit.join("compose.yaml"),
                explicit.join("compose.prod.yaml"),
            ],
        ),
        (
            vec![
                "dstack",
                "-s",
                "explicit",
                "--file",
                "compose.yaml",
                "--file",
                "compose.prod.yaml",
                "start",
                "web",
            ],
            SessionEnvironment::absent(),
            explicit.clone(),
            vec![
                explicit.join("compose.yaml"),
                explicit.join("compose.prod.yaml"),
            ],
        ),
        (
            vec!["dstack", "stop", "web"],
            active_session.clone(),
            active.clone(),
            vec![
                active.join("compose.yaml"),
                active.join("compose.session.yaml"),
            ],
        ),
        (
            vec!["dstack", "start", "web"],
            SessionEnvironment::absent(),
            cwd.clone(),
            vec![cwd.join("compose.yaml")],
        ),
    ] {
        let executor = RecordingExecutor::default();
        executor.push_capture(b"web\n".to_vec(), 0);
        executor.push_capture(b"unsupported ps output".to_vec(), 0);
        run(&argv, &paths, &cwd, &session, &executor).unwrap();

        let expected_prefix = compose_prefix(&files);
        for request in executor.capture_requests() {
            assert_eq!(request.process.current_dir, expected_project);
            assert!(request.process.args.starts_with(&expected_prefix));
            assert_eq!(request.policy, CapturePolicy::StdoutWithNullStderr);
        }
        let streamed = executor.requests();
        assert_eq!(streamed.len(), 1);
        assert_eq!(streamed[0].current_dir, expected_project);
        assert!(streamed[0].args.starts_with(&expected_prefix));
    }
}

#[test]
fn only_start_and_stop_perform_state_probes() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = project(temp.path(), "project", &["compose.yaml"]);
    for argv in [
        ["dstack", "up"].as_slice(),
        ["dstack", "restart"].as_slice(),
        ["dstack", "recreate"].as_slice(),
        ["dstack", "pull"].as_slice(),
        ["dstack", "update"].as_slice(),
        ["dstack", "rebuild"].as_slice(),
        ["dstack", "down"].as_slice(),
    ] {
        let executor = RecordingExecutor::default();
        run(argv, &paths, &cwd, &SessionEnvironment::absent(), &executor).unwrap();
        assert!(executor.capture_requests().is_empty(), "{argv:?}");
        assert!(!executor.requests().is_empty(), "{argv:?}");
    }
}

fn compose_prefix(files: &[PathBuf]) -> Vec<OsString> {
    std::iter::once(OsString::from("compose"))
        .chain(
            files
                .iter()
                .flat_map(|file| [OsString::from("--file"), file.as_os_str().to_os_string()]),
        )
        .collect()
}
