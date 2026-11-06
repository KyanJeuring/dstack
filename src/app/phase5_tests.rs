use std::{ffi::OsString, fs, path::PathBuf};

use tempfile::TempDir;

use super::run_with_executor;
use crate::{
    AppOutcome, Cli,
    config::AppPaths,
    process::{ProcessStatus, StdioPolicy, testing::RecordingExecutor},
    registry::{RegistryStore, StackProfile},
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
    let project = parent.join(name);
    fs::create_dir_all(&project).unwrap();
    for file in files {
        fs::write(project.join(file), "services: {}\n").unwrap();
    }
    fs::canonicalize(project).unwrap()
}

fn execute(
    argv: &[&str],
    paths: &AppPaths,
    cwd: &std::path::Path,
    executor: &RecordingExecutor,
) -> AppOutcome {
    run_with_executor(
        Cli::try_parse_from(argv).unwrap(),
        paths,
        cwd,
        executor,
        &mut Vec::new(),
        &mut Vec::new(),
    )
    .unwrap()
}

fn compose_args(project: &std::path::Path, files: &[&str], operation: &[&str]) -> Vec<OsString> {
    let mut args = vec![OsString::from("compose")];
    for file in files {
        args.push(OsString::from("--file"));
        args.push(project.join(file).into_os_string());
    }
    args.extend(operation.iter().map(OsString::from));
    args
}

#[test]
fn services_reuses_exact_cwd_registered_discovered_and_explicit_file_resolution() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = project(temp.path(), "cwd project", &["compose.yaml"]);
    let registered = project(
        temp.path(),
        "registered project",
        &["compose.yaml", "compose.prod.yaml", "override file.yaml"],
    );
    let discovered = project(
        &paths.home_dir().join("projects"),
        "discovered",
        &["compose.yaml"],
    );
    let store = RegistryStore::new(
        paths.registry_file().to_path_buf(),
        paths.registry_lock().to_path_buf(),
    );
    store
        .add(
            StackProfile::for_registration(
                "named".to_owned(),
                registered.clone(),
                ["compose.yaml", "compose.prod.yaml"]
                    .map(PathBuf::from)
                    .to_vec(),
            )
            .unwrap(),
        )
        .unwrap();
    let executor = RecordingExecutor::default();

    for argv in [
        vec!["dstack", "services"],
        vec!["dstack", "-s", "named", "services"],
        vec!["dstack", "services", "-s", "named"],
        vec![
            "dstack",
            "-s",
            "named",
            "--file",
            "override file.yaml",
            "services",
        ],
        vec!["dstack", "services", "-s", "discovered"],
    ] {
        assert_eq!(
            execute(&argv, &paths, &cwd, &executor),
            AppOutcome::Child(ProcessStatus::from_code(0))
        );
    }

    let requests = executor.requests();
    assert_eq!(requests.len(), 5);
    assert_eq!(
        requests[0].args,
        compose_args(&cwd, &["compose.yaml"], &["config", "--services"])
    );
    assert_eq!(requests[0].current_dir, cwd);
    for request in &requests[1..3] {
        assert_eq!(
            request.args,
            compose_args(
                &registered,
                &["compose.yaml", "compose.prod.yaml"],
                &["config", "--services"],
            )
        );
        assert_eq!(request.current_dir, registered);
    }
    assert_eq!(
        requests[3].args,
        compose_args(
            &registered,
            &["override file.yaml"],
            &["config", "--services"],
        )
    );
    assert_eq!(requests[3].current_dir, registered);
    assert_eq!(
        requests[4].args,
        compose_args(&discovered, &["compose.yaml"], &["config", "--services"],)
    );
    assert_eq!(requests[4].current_dir, discovered);
    assert!(
        requests
            .iter()
            .all(|request| request.stdio == StdioPolicy::Inherit)
    );
}

#[test]
fn logs_exec_and_run_use_explicit_stack_over_cwd_and_preserve_native_argv() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = project(temp.path(), "unrelated cwd", &["compose.yaml"]);
    let registered = project(temp.path(), "registered project λ", &["compose file.yaml"]);
    let store = RegistryStore::new(
        paths.registry_file().to_path_buf(),
        paths.registry_lock().to_path_buf(),
    );
    store
        .add(
            StackProfile::for_registration(
                "named".to_owned(),
                registered.clone(),
                vec![PathBuf::from("compose file.yaml")],
            )
            .unwrap(),
        )
        .unwrap();
    let executor = RecordingExecutor::default();

    execute(
        &[
            "dstack", "logs", "服务", "worker", "服务", "--tail", "25", "-f", "-s", "named",
        ],
        &paths,
        &cwd,
        &executor,
    );
    execute(
        &[
            "dstack",
            "exec",
            "-s",
            "named",
            "服务",
            "--",
            "sh",
            "-c",
            "echo a && echo b",
        ],
        &paths,
        &cwd,
        &executor,
    );
    execute(
        &[
            "dstack",
            "-s",
            "named",
            "run",
            "worker",
            "--",
            "命令",
            "argument with spaces",
            "--file",
            "-s",
        ],
        &paths,
        &cwd,
        &executor,
    );

    let requests = executor.requests();
    assert_eq!(requests.len(), 3);
    let expected = [
        vec!["logs", "--follow", "--tail", "25", "服务", "worker", "服务"],
        vec!["exec", "服务", "sh", "-c", "echo a && echo b"],
        vec![
            "run",
            "--rm",
            "worker",
            "命令",
            "argument with spaces",
            "--file",
            "-s",
        ],
    ];
    for (request, operation) in requests.iter().zip(expected) {
        assert_eq!(
            request.args,
            compose_args(&registered, &["compose file.yaml"], &operation)
        );
        assert_eq!(request.current_dir, registered);
        assert_eq!(request.stdio, StdioPolicy::Inherit);
    }
}

#[test]
fn phase5_handlers_add_no_dstack_status_output_around_children() {
    let temp = TempDir::new().unwrap();
    let paths = paths(&temp);
    let cwd = project(temp.path(), "project", &["compose.yaml"]);
    let executor = RecordingExecutor::default();

    for argv in [
        vec!["dstack", "services"],
        vec!["dstack", "logs"],
        vec!["dstack", "exec", "app", "--", "true"],
        vec!["dstack", "run", "worker"],
    ] {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        run_with_executor(
            Cli::try_parse_from(argv).unwrap(),
            &paths,
            &cwd,
            &executor,
            &mut stdout,
            &mut stderr,
        )
        .unwrap();
        assert!(stdout.is_empty());
        assert!(stderr.is_empty());
    }
}
