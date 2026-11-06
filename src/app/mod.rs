mod bootstrap;
mod dispatch;

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
    output::{ColorPolicy, Output},
};

/// Runs the v2 application after CLI parsing.
pub fn run(cli: Cli) -> Result<(), AppError> {
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
    run_with_outputs(cli, &paths, &current_dir, &mut stdout, &mut stderr)
}

#[cfg(test)]
pub(crate) fn run_with_environment(
    cli: Cli,
    paths: &AppPaths,
    current_dir: &Path,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), AppError> {
    let mut stdout = Output::plain(stdout);
    let mut stderr = Output::plain(stderr);
    run_with_outputs(cli, paths, current_dir, &mut stdout, &mut stderr)
}

fn run_with_outputs(
    cli: Cli,
    paths: &AppPaths,
    current_dir: &Path,
    stdout: &mut Output<'_>,
    stderr: &mut Output<'_>,
) -> Result<(), AppError> {
    if cli.command.is_none() {
        return Err(AppError::NoCommand);
    }
    let runtime = bootstrap::run(paths, stderr)?;
    dispatch::run(cli, paths, current_dir, &runtime, stdout, stderr)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use clap::Parser;
    use tempfile::TempDir;

    use super::run_with_environment;
    use crate::{
        AppError, Cli,
        config::AppPaths,
        registry::{Registry, StackProfile},
    };

    fn paths(temp: &TempDir) -> AppPaths {
        AppPaths::with_home(
            temp.path().join("state"),
            temp.path().join("home"),
            temp.path().join("legacy/registry"),
            None,
        )
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
}
