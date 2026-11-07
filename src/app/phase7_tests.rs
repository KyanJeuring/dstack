use std::{ffi::OsString, fs, path::PathBuf};

use tempfile::TempDir;

use super::run_with_session_executor;
use crate::{
    AppError, Cli,
    config::AppPaths,
    process::testing::RecordingExecutor,
    registry::{RegistryStore, StackProfile},
    session::{SESSION_SCHEMA_VERSION, SessionEnvironment, SessionOrigin, SessionState},
};

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

fn environment(name: &str, project: &std::path::Path, files: &[&str]) -> SessionEnvironment {
    let state = SessionState {
        version: SESSION_SCHEMA_VERSION,
        name: name.to_owned(),
        project: project.to_path_buf(),
        files: files.iter().map(|file| project.join(file)).collect(),
        origin: SessionOrigin::Registered,
    };
    SessionEnvironment::with_value(state.serialize().unwrap())
}

fn run(
    argv: &[&str],
    paths: &AppPaths,
    cwd: &std::path::Path,
    session: &SessionEnvironment,
    executor: &RecordingExecutor,
) -> Result<(Vec<u8>, Vec<u8>), AppError> {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    run_with_session_executor(
        Cli::try_parse_from(argv).unwrap(),
        paths,
        cwd,
        executor,
        session,
        &mut stdout,
        &mut stderr,
    )?;
    Ok((stdout, stderr))
}

#[test]
fn session_beats_cwd_and_explicit_file_replaces_session_files() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = project(temp.path(), "cwd", &["compose.yaml"]);
    let active = project(
        temp.path(),
        "active",
        &["compose.yaml", "compose.prod.yaml", "compose.debug.yaml"],
    );
    let session = environment("jellyfin", &active, &["compose.yaml", "compose.prod.yaml"]);
    let executor = RecordingExecutor::default();

    let (stdout, _) = run(
        &["dstack", "context", "show"],
        &paths,
        &cwd,
        &session,
        &executor,
    )
    .unwrap();
    let stdout = String::from_utf8(stdout).unwrap();
    assert!(stdout.contains("Stack: jellyfin\n"));
    assert!(stdout.contains("Source: session\n"));
    assert!(stdout.contains(&active.display().to_string()));
    assert!(!stdout.contains(&cwd.display().to_string()));

    run(&["dstack", "up"], &paths, &cwd, &session, &executor).unwrap();
    run(
        &["dstack", "--file", "compose.debug.yaml", "up"],
        &paths,
        &cwd,
        &session,
        &executor,
    )
    .unwrap();
    let requests = executor.requests();
    assert_eq!(requests[0].current_dir, active);
    assert_eq!(
        requests[0].args,
        [
            OsString::from("compose"),
            OsString::from("--file"),
            active.join("compose.yaml").into_os_string(),
            OsString::from("--file"),
            active.join("compose.prod.yaml").into_os_string(),
            OsString::from("up"),
            OsString::from("-d"),
            OsString::from("--build"),
        ]
    );
    assert_eq!(
        requests[1].args,
        [
            OsString::from("compose"),
            OsString::from("--file"),
            active.join("compose.debug.yaml").into_os_string(),
            OsString::from("up"),
            OsString::from("-d"),
            OsString::from("--build"),
        ]
    );
}

#[test]
fn explicit_stack_completely_beats_session_and_uses_its_profile_files() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let active = project(temp.path(), "active", &["compose.yaml"]);
    let explicit = project(temp.path(), "explicit", &["other.yaml"]);
    let store = RegistryStore::new(
        paths.registry_file().to_path_buf(),
        paths.registry_lock().to_path_buf(),
    );
    store
        .add(
            StackProfile::for_registration(
                "immich".to_owned(),
                explicit.clone(),
                vec![PathBuf::from("other.yaml")],
            )
            .unwrap(),
        )
        .unwrap();
    let session = environment("jellyfin", &active, &["compose.yaml"]);
    let executor = RecordingExecutor::default();
    let (stdout, _) = run(
        &["dstack", "-s", "immich", "context", "show"],
        &paths,
        temp.path(),
        &session,
        &executor,
    )
    .unwrap();
    let stdout = String::from_utf8(stdout).unwrap();
    assert!(stdout.contains("Stack: immich\n"));
    assert!(stdout.contains("Source: registered\n"));
    assert!(stdout.contains(&explicit.join("other.yaml").display().to_string()));
    assert!(!stdout.contains(&active.display().to_string()));
}

#[test]
fn malformed_session_is_lazy_for_explicit_list_host_use_and_clear() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let explicit = project(temp.path(), "explicit", &["compose.yaml"]);
    let store = RegistryStore::new(
        paths.registry_file().to_path_buf(),
        paths.registry_lock().to_path_buf(),
    );
    store
        .add(
            StackProfile::for_registration("new".to_owned(), explicit.clone(), Vec::new()).unwrap(),
        )
        .unwrap();
    let malformed = SessionEnvironment::with_value("not-json");
    let executor = RecordingExecutor::default();

    run(
        &["dstack", "-s", "new", "context", "show"],
        &paths,
        temp.path(),
        &malformed,
        &executor,
    )
    .unwrap();
    run(
        &["dstack", "list"],
        &paths,
        temp.path(),
        &malformed,
        &executor,
    )
    .unwrap();
    executor.push_capture(Vec::new(), 0);
    run(
        &["dstack", "container", "list"],
        &paths,
        temp.path(),
        &malformed,
        &executor,
    )
    .unwrap();
    let (activation, _) = run(
        &["dstack", "shell", "__activate", "bash", "new"],
        &paths,
        temp.path(),
        &malformed,
        &executor,
    )
    .unwrap();
    assert!(
        String::from_utf8(activation)
            .unwrap()
            .starts_with("export DSTACK_SESSION='{")
    );
    let (clear, _) = run(
        &["dstack", "shell", "__clear", "bash"],
        &paths,
        temp.path(),
        &malformed,
        &executor,
    )
    .unwrap();
    assert_eq!(clear, b"unset DSTACK_SESSION\n");

    let error = run(
        &["dstack", "context", "show"],
        &paths,
        temp.path(),
        &malformed,
        &executor,
    )
    .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("active terminal session is invalid")
    );
}

#[test]
fn stale_session_never_falls_back_to_valid_cwd() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = project(temp.path(), "cwd", &["compose.yaml"]);
    let active = project(temp.path(), "active", &["compose.yaml"]);
    let payload = environment("active", &active, &["compose.yaml"]);
    fs::remove_dir_all(&active).unwrap();
    let error = run(
        &["dstack", "context", "show"],
        &paths,
        &cwd,
        &payload,
        &RecordingExecutor::default(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("session project"));
    assert!(error.to_string().contains("dstack unset"));
    assert!(!error.to_string().contains(&cwd.display().to_string()));

    let active = project(temp.path(), "active-file", &["compose.yaml"]);
    let payload = environment("active", &active, &["compose.yaml"]);
    fs::remove_file(active.join("compose.yaml")).unwrap();
    let error = run(
        &["dstack", "context", "show"],
        &paths,
        &cwd,
        &payload,
        &RecordingExecutor::default(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("session Compose file"));
    assert!(error.to_string().contains("dstack use NAME"));
}

#[test]
fn registry_removal_does_not_invalidate_an_existing_snapshot() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let active = project(temp.path(), "active", &["compose.yaml"]);
    let store = RegistryStore::new(
        paths.registry_file().to_path_buf(),
        paths.registry_lock().to_path_buf(),
    );
    store
        .add(
            StackProfile::for_registration("saved".to_owned(), active.clone(), Vec::new()).unwrap(),
        )
        .unwrap();
    let session = environment("saved", &active, &["compose.yaml"]);
    store.remove("saved").unwrap();

    let (stdout, _) = run(
        &["dstack", "context", "show"],
        &paths,
        temp.path(),
        &session,
        &RecordingExecutor::default(),
    )
    .unwrap();
    let stdout = String::from_utf8(stdout).unwrap();
    assert!(stdout.contains("Stack: saved\n"));
    assert!(stdout.contains("Source: session\n"));
}

fn bash_payload(output: Vec<u8>) -> SessionState {
    let output = String::from_utf8(output).unwrap();
    let payload = output
        .strip_prefix("export DSTACK_SESSION='")
        .and_then(|value| value.strip_suffix("'\n"))
        .expect("simple test paths should produce one Bash single-quoted payload");
    SessionState::parse(payload).unwrap()
}

#[test]
fn activation_resolves_registered_and_discovered_names_with_fresh_file_precedence() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let registered = project(
        temp.path(),
        "registered",
        &[
            "compose.yaml",
            "compose.prod.yaml",
            "compose.one.yaml",
            "compose.two.yaml",
        ],
    );
    let store = RegistryStore::new(
        paths.registry_file().to_path_buf(),
        paths.registry_lock().to_path_buf(),
    );
    store
        .add(
            StackProfile::for_registration(
                "profile".to_owned(),
                registered.clone(),
                vec![
                    PathBuf::from("compose.yaml"),
                    PathBuf::from("compose.prod.yaml"),
                ],
            )
            .unwrap(),
        )
        .unwrap();
    let malformed = SessionEnvironment::with_value("old broken session");
    let executor = RecordingExecutor::default();

    let (default_output, _) = run(
        &["dstack", "shell", "__activate", "bash", "profile"],
        &paths,
        temp.path(),
        &malformed,
        &executor,
    )
    .unwrap();
    assert_eq!(
        bash_payload(default_output).files,
        [
            registered.join("compose.yaml"),
            registered.join("compose.prod.yaml")
        ]
    );

    let (explicit_output, _) = run(
        &[
            "dstack",
            "shell",
            "__activate",
            "bash",
            "profile",
            "--file",
            "compose.two.yaml",
            "--file",
            "compose.one.yaml",
        ],
        &paths,
        temp.path(),
        &malformed,
        &executor,
    )
    .unwrap();
    assert_eq!(
        bash_payload(explicit_output).files,
        [
            registered.join("compose.two.yaml"),
            registered.join("compose.one.yaml")
        ]
    );

    let discovered = project(
        &paths.home_dir().join("projects"),
        "发现 stack",
        &["compose.yaml"],
    );
    let (discovered_output, _) = run(
        &["dstack", "shell", "__activate", "bash", "发现 stack"],
        &paths,
        temp.path(),
        &malformed,
        &executor,
    )
    .unwrap();
    let state = bash_payload(discovered_output);
    assert_eq!(state.project, discovered);
    assert_eq!(state.files, [discovered.join("compose.yaml")]);
    assert_eq!(state.origin, SessionOrigin::Discovered);
}

#[test]
fn activation_rejects_unknown_and_ambiguous_discovered_names() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    project(
        &paths.home_dir().join("projects"),
        "duplicate",
        &["compose.yaml"],
    );
    project(
        &paths.home_dir().join("src"),
        "duplicate",
        &["compose.yaml"],
    );
    let executor = RecordingExecutor::default();
    let absent = SessionEnvironment::absent();

    for (name, expected) in [("missing", "was not found"), ("duplicate", "ambiguous")] {
        let error = run(
            &["dstack", "shell", "__activate", "bash", name],
            &paths,
            temp.path(),
            &absent,
            &executor,
        )
        .unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }
}
