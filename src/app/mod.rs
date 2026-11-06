mod bootstrap;
mod dispatch;
mod operations;
mod target;

use std::{
    env,
    io::{self, IsTerminal},
    path::{Path, PathBuf},
};

#[cfg(test)]
use std::io::Write;

use crate::{
    Cli,
    config::AppPaths,
    error::AppError,
    outcome::AppOutcome,
    output::{ColorPolicy, Output},
    process::{NativeProcessExecutor, ProcessExecutor},
};

/// Runs the v2 application after CLI parsing.
pub fn run(cli: Cli) -> Result<AppOutcome, AppError> {
    let paths = AppPaths::discover()?;
    let current_dir =
        env::current_dir().map_err(|source| crate::resolution::ResolutionError::Io {
            action: "read current directory",
            path: PathBuf::from("."),
            source,
        })?;
    let stdout = io::stdout();
    let stderr = io::stderr();
    let stdout_color = ColorPolicy::from_environment(stdout.is_terminal());
    let stderr_color = ColorPolicy::from_environment(stderr.is_terminal());
    let mut stdout = anstream::AutoStream::new(stdout, stdout_color.stream_choice()).lock();
    let mut stderr = anstream::AutoStream::new(stderr, stderr_color.stream_choice()).lock();
    let mut stdout = Output::new(&mut stdout, stdout_color);
    let mut stderr = Output::new(&mut stderr, stderr_color);
    run_with_outputs(
        cli,
        &paths,
        &current_dir,
        &NativeProcessExecutor,
        &mut stdout,
        &mut stderr,
    )
}

#[cfg(test)]
pub(crate) fn run_with_environment(
    cli: Cli,
    paths: &AppPaths,
    current_dir: &Path,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<AppOutcome, AppError> {
    let mut stdout = Output::plain(stdout);
    let mut stderr = Output::plain(stderr);
    run_with_outputs(
        cli,
        paths,
        current_dir,
        &NativeProcessExecutor,
        &mut stdout,
        &mut stderr,
    )
}

#[cfg(test)]
pub(crate) fn run_with_executor(
    cli: Cli,
    paths: &AppPaths,
    current_dir: &Path,
    executor: &dyn ProcessExecutor,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<AppOutcome, AppError> {
    let mut stdout = Output::plain(stdout);
    let mut stderr = Output::plain(stderr);
    run_with_outputs(cli, paths, current_dir, executor, &mut stdout, &mut stderr)
}

fn run_with_outputs(
    cli: Cli,
    paths: &AppPaths,
    current_dir: &Path,
    executor: &dyn ProcessExecutor,
    stdout: &mut Output<'_>,
    stderr: &mut Output<'_>,
) -> Result<AppOutcome, AppError> {
    if cli.command.is_none() {
        return Err(AppError::NoCommand);
    }
    let runtime = bootstrap::run(paths, stderr)?;
    dispatch::run(cli, paths, current_dir, &runtime, executor, stdout, stderr)
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, fs, io, path::PathBuf};

    use tempfile::TempDir;

    use super::{run_with_environment, run_with_executor};
    use crate::{
        AppError, AppOutcome, Cli,
        config::AppPaths,
        process::testing::RecordingExecutor,
        registry::{Registry, RegistryStore, StackProfile},
    };

    fn paths(temp: &TempDir) -> AppPaths {
        AppPaths::with_home(
            temp.path().join("state"),
            temp.path().join("home"),
            temp.path().join("legacy/registry"),
            None,
        )
    }

    fn local_project(temp: &TempDir) -> PathBuf {
        let project = temp.path().join("project with spaces").join("café");
        fs::create_dir_all(&project).unwrap();
        fs::write(project.join("compose.yaml"), "services: {}\n").unwrap();
        project
    }

    #[test]
    fn no_command_returns_usage_error_without_bootstrap() {
        let temp = TempDir::new().unwrap();
        let paths = paths(&temp);
        let error = run_with_environment(
            Cli {
                stack: None,
                files: Vec::new(),
                command: None,
            },
            &paths,
            temp.path(),
            &mut Vec::new(),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(matches!(error, AppError::NoCommand));
        assert!(!paths.application_dir().exists());
    }

    #[test]
    fn first_registry_list_bootstraps_config_and_empty_registry() {
        let temp = TempDir::new().unwrap();
        let paths = paths(&temp);
        let cli = Cli::try_parse_from(["dstack", "registry", "list"]).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        run_with_environment(cli, &paths, temp.path(), &mut stdout, &mut stderr).unwrap();
        assert_eq!(stdout, b"NAME  PATH  FILES\n");
        assert!(stderr.is_empty());
        assert!(paths.config_file().is_file());
        assert!(paths.registry_file().is_file());
    }

    #[test]
    fn phase2_registry_is_copied_only_when_canonical_is_absent() {
        let temp = TempDir::new().unwrap();
        let old = temp.path().join("phase2/registry.toml");
        fs::create_dir_all(old.parent().unwrap()).unwrap();
        let project = temp.path().join("project");
        fs::create_dir_all(&project).unwrap();
        let mut old_registry = Registry::default();
        old_registry
            .insert(
                StackProfile::for_registration("copied".to_owned(), project, Vec::new()).unwrap(),
            )
            .unwrap();
        let old_contents = old_registry.serialize().unwrap();
        fs::write(&old, &old_contents).unwrap();
        let paths = AppPaths::with_home(
            temp.path().join("canonical"),
            temp.path().join("home"),
            temp.path().join("legacy/registry"),
            Some(old.clone()),
        );
        let mut stderr = Vec::new();
        run_with_environment(
            Cli::try_parse_from(["dstack", "registry", "list"]).unwrap(),
            &paths,
            temp.path(),
            &mut Vec::new(),
            &mut stderr,
        )
        .unwrap();
        assert!(paths.registry_file().is_file());
        assert_eq!(fs::read_to_string(&old).unwrap(), old_contents);
        assert!(
            fs::read_to_string(paths.registry_file())
                .unwrap()
                .contains("copied")
        );
        assert!(
            String::from_utf8(stderr)
                .unwrap()
                .contains("left untouched")
        );
    }

    #[test]
    fn canonical_registry_wins_without_merging_phase2_state() {
        let temp = TempDir::new().unwrap();
        let paths = AppPaths::with_home(
            temp.path().join("canonical"),
            temp.path().join("home"),
            PathBuf::from("unused"),
            Some(temp.path().join("phase2/registry.toml")),
        );
        fs::create_dir_all(paths.application_dir()).unwrap();
        fs::write(paths.registry_file(), "version = 1\nstacks = []\n").unwrap();
        fs::create_dir_all(paths.phase2_registry().unwrap().parent().unwrap()).unwrap();
        fs::write(paths.phase2_registry().unwrap(), "not = [valid").unwrap();
        let canonical = fs::read(paths.registry_file()).unwrap();
        run_with_environment(
            Cli::try_parse_from(["dstack", "registry", "list"]).unwrap(),
            &paths,
            temp.path(),
            &mut Vec::new(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(fs::read(paths.registry_file()).unwrap(), canonical);
        assert_eq!(
            fs::read(paths.phase2_registry().unwrap()).unwrap(),
            b"not = [valid"
        );
    }

    #[test]
    fn raw_compose_executes_exact_native_argv_without_typed_defaults() {
        let temp = TempDir::new().unwrap();
        let paths = paths(&temp);
        let project = local_project(&temp);
        let executor = RecordingExecutor::default();
        let cases = [
            vec!["ps"],
            vec!["up"],
            vec!["up", "-d"],
            vec!["down"],
            vec!["down", "-v"],
            vec!["logs", "-f"],
            vec!["config"],
            vec![
                "build",
                "--label",
                "value with spaces",
                "--label",
                "Unicode-项目",
                "--no-cache",
            ],
        ];

        for raw in &cases {
            let cli =
                Cli::try_parse_from(["dstack", "compose"].into_iter().chain(raw.iter().copied()))
                    .unwrap();
            let outcome = run_with_executor(
                cli,
                &paths,
                &project,
                &executor,
                &mut Vec::new(),
                &mut Vec::new(),
            )
            .unwrap();
            assert_eq!(
                outcome,
                AppOutcome::Child(crate::process::ProcessStatus::from_code(0))
            );
        }

        let requests = executor.requests();
        assert_eq!(requests.len(), cases.len());
        for (request, raw) in requests.iter().zip(cases) {
            let expected = [
                vec![
                    OsString::from("compose"),
                    OsString::from("--file"),
                    project.join("compose.yaml").into_os_string(),
                ],
                raw.into_iter().map(OsString::from).collect(),
            ]
            .concat();
            assert_eq!(request.program, OsString::from("docker"));
            assert_eq!(request.args, expected);
            assert_eq!(request.current_dir, project);
        }

        let raw_up = &requests[1].args;
        assert!(!raw_up.contains(&OsString::from("-d")));
        assert!(!raw_up.contains(&OsString::from("--build")));
    }

    #[test]
    fn hard_boundary_preserves_compose_file_and_stack_shaped_raw_arguments() {
        let temp = TempDir::new().unwrap();
        let paths = paths(&temp);
        let project = local_project(&temp);
        let executor = RecordingExecutor::default();
        let cli = Cli::try_parse_from([
            "dstack",
            "compose",
            "--",
            "down",
            "--file",
            "special file.yaml",
            "-s",
            "compose-value",
        ])
        .unwrap();

        run_with_executor(
            cli,
            &paths,
            &project,
            &executor,
            &mut Vec::new(),
            &mut Vec::new(),
        )
        .unwrap();

        assert_eq!(
            executor.requests()[0].args,
            [
                OsString::from("compose"),
                OsString::from("--file"),
                project.join("compose.yaml").into_os_string(),
                OsString::from("down"),
                OsString::from("--file"),
                OsString::from("special file.yaml"),
                OsString::from("-s"),
                OsString::from("compose-value"),
            ]
        );
    }

    #[test]
    fn raw_compose_returns_child_status_and_maps_spawn_errors_separately() {
        let temp = TempDir::new().unwrap();
        let paths = paths(&temp);
        let project = local_project(&temp);
        let executor = RecordingExecutor::default();
        executor.push_status(17);

        let outcome = run_with_executor(
            Cli::try_parse_from(["dstack", "compose", "ps"]).unwrap(),
            &paths,
            &project,
            &executor,
            &mut Vec::new(),
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(
            outcome,
            AppOutcome::Child(crate::process::ProcessStatus::from_code(17))
        );

        let executor = RecordingExecutor::default();
        executor.push_spawn_error(io::ErrorKind::NotFound, "docker is not installed");
        let error = run_with_executor(
            Cli::try_parse_from(["dstack", "compose", "ps"]).unwrap(),
            &paths,
            &project,
            &executor,
            &mut Vec::new(),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(matches!(error, AppError::Process(_)));
        assert!(
            error
                .to_string()
                .contains("could not start process 'docker'")
        );
        assert!(error.to_string().contains("docker is not installed"));
    }

    #[test]
    fn operations_reuse_registered_discovered_and_explicit_file_resolution() {
        let temp = TempDir::new().unwrap();
        let paths = paths(&temp);
        let registered = temp.path().join("registered project");
        fs::create_dir_all(&registered).unwrap();
        for file in ["compose.yaml", "compose.prod.yaml", "override file.yaml"] {
            fs::write(registered.join(file), "services: {}\n").unwrap();
        }
        let store = RegistryStore::new(
            paths.registry_file().to_path_buf(),
            paths.registry_lock().to_path_buf(),
        );
        store
            .add(
                StackProfile::for_registration(
                    "registered".to_owned(),
                    registered.clone(),
                    ["compose.yaml", "compose.prod.yaml"]
                        .map(PathBuf::from)
                        .to_vec(),
                )
                .unwrap(),
            )
            .unwrap();

        let discovered = paths.home_dir().join("projects").join("discovered");
        fs::create_dir_all(&discovered).unwrap();
        fs::write(discovered.join("compose.yaml"), "services: {}\n").unwrap();
        let unrelated_cwd = local_project(&temp);
        let executor = RecordingExecutor::default();

        for argv in [
            vec!["dstack", "-s", "registered", "up"],
            vec![
                "dstack",
                "-s",
                "registered",
                "--file",
                "override file.yaml",
                "stop",
                "php",
            ],
            vec!["dstack", "start", "-s", "discovered"],
        ] {
            run_with_executor(
                Cli::try_parse_from(argv).unwrap(),
                &paths,
                &unrelated_cwd,
                &executor,
                &mut Vec::new(),
                &mut Vec::new(),
            )
            .unwrap();
        }

        let requests = executor.requests();
        assert_eq!(
            requests[0].current_dir,
            fs::canonicalize(&registered).unwrap()
        );
        assert_eq!(
            requests[0].args,
            [
                OsString::from("compose"),
                OsString::from("--file"),
                registered.join("compose.yaml").into_os_string(),
                OsString::from("--file"),
                registered.join("compose.prod.yaml").into_os_string(),
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
                registered.join("override file.yaml").into_os_string(),
                OsString::from("stop"),
                OsString::from("php"),
            ]
        );
        assert_eq!(requests[2].current_dir, discovered);
        assert_eq!(
            requests[2].args,
            [
                OsString::from("compose"),
                OsString::from("--file"),
                requests[2]
                    .current_dir
                    .join("compose.yaml")
                    .into_os_string(),
                OsString::from("start"),
            ]
        );
    }
}
