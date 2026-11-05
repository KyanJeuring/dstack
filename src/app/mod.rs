use std::{
    env,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
};

use crate::{
    Cli,
    cli::{Command, ConfigCommand, ContextCommand, MigrationCommand, RegistryCommand},
    compose::resolve_compose_files,
    config::{AppPaths, ConfigStore, DstackConfig},
    discovery::discover_stacks,
    error::AppError,
    migration::{MigrationReport, migrate_registry},
    output::{ColorPolicy, Output, Table},
    registry::{Registry, RegistryStore, StackProfile},
    resolution::{effective_stacks, resolve_stack},
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

struct RuntimeState {
    config: DstackConfig,
    registry: Registry,
    registry_store: RegistryStore,
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
    let command = cli.command.ok_or(AppError::NoCommand)?;
    validate_global_options(&command, cli.stack.as_deref(), &cli.files)?;
    let root_stack = cli.stack;
    let root_files = cli.files;
    let runtime = bootstrap(paths, stderr)?;

    match command {
        Command::List => list_effective(&runtime, paths, stdout),
        Command::Config(arguments) => match arguments.command {
            ConfigCommand::Path => {
                writeln!(stdout, "{}", paths.config_file().display())?;
                Ok(())
            }
            ConfigCommand::Show => show_config(paths, &runtime.config, stdout),
        },
        Command::Context(arguments) => match arguments.command {
            ContextCommand::Show(options) => {
                let stack = merge_stack_options(root_stack, options.stack)?;
                let files = [root_files, options.files].concat();
                show_context(
                    &runtime,
                    paths,
                    stack.as_deref(),
                    &files,
                    current_dir,
                    stdout,
                )
            }
        },
        Command::Registry(arguments) => match arguments.command {
            RegistryCommand::List => list_registry(&runtime.registry, stdout),
            RegistryCommand::Show { name } => show_profile(&runtime.registry, &name, stdout),
            RegistryCommand::Add { name, path, files } => {
                let profile =
                    StackProfile::for_registration(name, path, [root_files, files].concat())?;
                let registered_name = profile.name.clone();
                runtime.registry_store.add(profile)?;
                writeln!(stdout, "Registered stack profile '{registered_name}'.")?;
                Ok(())
            }
            RegistryCommand::Remove { name } => {
                runtime.registry_store.remove(&name)?;
                writeln!(stdout, "Removed stack profile '{name}'.")?;
                Ok(())
            }
        },
        Command::Migrate(arguments) => match arguments.command {
            MigrationCommand::Registry { dry_run } => {
                let report =
                    migrate_registry(&runtime.registry_store, paths.legacy_registry(), dry_run)?;
                write_migration_report(&report, stdout, stderr)?;
                if report.has_issues() {
                    Err(AppError::MigrationIncomplete {
                        skipped: report.skipped.len(),
                        conflicts: report.conflicts.len(),
                    })
                } else {
                    Ok(())
                }
            }
        },
    }
}

fn merge_stack_options(
    root: Option<String>,
    command: Option<String>,
) -> Result<Option<String>, AppError> {
    match (root, command) {
        (Some(_), Some(_)) => Err(AppError::InvalidOptions(
            "--stack/-s may be supplied only once".to_owned(),
        )),
        (Some(value), None) | (None, Some(value)) => Ok(Some(value)),
        (None, None) => Ok(None),
    }
}

fn validate_global_options(
    command: &Command,
    stack: Option<&str>,
    files: &[PathBuf],
) -> Result<(), AppError> {
    let supports_stack = matches!(command, Command::Context(_));
    let supports_files = matches!(
        command,
        Command::Context(_)
            | Command::Registry(crate::cli::RegistryArgs {
                command: RegistryCommand::Add { .. }
            })
    );

    if stack.is_some() && !supports_stack {
        return Err(AppError::InvalidOptions(
            "--stack/-s is only supported by stack-context operations in Phase 3".to_owned(),
        ));
    }
    if !files.is_empty() && !supports_files {
        return Err(AppError::InvalidOptions(
            "--file is only supported by 'context show' and 'registry add' in Phase 3".to_owned(),
        ));
    }
    Ok(())
}

fn bootstrap(paths: &AppPaths, stderr: &mut Output<'_>) -> Result<RuntimeState, AppError> {
    let registry_store = RegistryStore::new(
        paths.registry_file().to_path_buf(),
        paths.registry_lock().to_path_buf(),
    );
    let config_store = ConfigStore::new(paths.config_file().to_path_buf());

    let migrated_from = {
        let _lock = registry_store.acquire_lock()?;
        let migrated = registry_store.initialize_unlocked(paths.phase2_registry())?;
        config_store.ensure_exists()?;
        migrated
    };

    if let Some(source) = migrated_from {
        writeln!(
            stderr,
            "dstack: copied pre-release Phase 2 registry '{}' to '{}'; the source was left untouched",
            source.display(),
            paths.registry_file().display()
        )?;
    }

    Ok(RuntimeState {
        config: config_store.load()?,
        registry: registry_store.load()?,
        registry_store,
    })
}

fn show_config(
    paths: &AppPaths,
    config: &DstackConfig,
    output: &mut Output<'_>,
) -> Result<(), AppError> {
    output.write_label_value("Config:", &paths.config_file().display().to_string())?;
    output.write_label_value("Version:", &config.version.to_string())?;
    output.write_label_line("Discovery paths:")?;
    for path in &config.discovery.paths {
        writeln!(output, "  - {}", path.display())?;
    }
    output.write_label_line("Compose filenames (priority order):")?;
    for file in &config.discovery.compose_files {
        writeln!(output, "  - {}", file.display())?;
    }
    Ok(())
}

fn list_effective(
    runtime: &RuntimeState,
    paths: &AppPaths,
    output: &mut Output<'_>,
) -> Result<(), AppError> {
    let discovered = discover_stacks(&runtime.config, paths.home_dir())?;
    let mut table = Table::new(["NAME", "SOURCE", "PATH"]);
    for stack in effective_stacks(&runtime.registry, &discovered) {
        let source = if stack.ambiguous {
            "discovered (ambiguous)".to_owned()
        } else {
            stack.source.to_string()
        };
        table.push([stack.name, source, stack.project.display().to_string()]);
    }
    table.write(output)?;
    Ok(())
}

fn show_context(
    runtime: &RuntimeState,
    paths: &AppPaths,
    explicit_stack: Option<&str>,
    explicit_files: &[PathBuf],
    current_dir: &Path,
    output: &mut Output<'_>,
) -> Result<(), AppError> {
    let discovered = discover_stacks(&runtime.config, paths.home_dir())?;
    let stack = resolve_stack(
        explicit_stack,
        current_dir,
        &runtime.registry,
        &discovered,
        &runtime.config,
    )?;
    let compose = resolve_compose_files(&stack, explicit_files, &runtime.config)?;

    output.write_label_value("Stack:", &stack.name)?;
    output.write_label_value("Source:", &stack.source.to_string())?;
    output.write_label_value("Project:", &stack.project.display().to_string())?;
    output.write_label_line("Compose files:")?;
    for file in compose.files {
        writeln!(output, "  - {}", file.display())?;
    }
    Ok(())
}

fn list_registry(registry: &Registry, output: &mut Output<'_>) -> Result<(), AppError> {
    let mut table = Table::new(["NAME", "PATH", "FILES"]);
    for profile in &registry.stacks {
        table.push([
            profile.name.clone(),
            profile.path.display().to_string(),
            display_files(&profile.files),
        ]);
    }
    table.write(output)?;
    Ok(())
}

fn show_profile(registry: &Registry, name: &str, output: &mut Output<'_>) -> Result<(), AppError> {
    let profile = registry
        .get(name)
        .ok_or_else(|| crate::registry::RegistryError::UnknownProfile(name.to_owned()))?;
    output.write_label_value("Name:", &profile.name)?;
    output.write_label_value("Path:", &profile.path.display().to_string())?;
    if profile.files.is_empty() {
        output.write_label_value("Files:", "-")?;
    } else {
        output.write_label_line("Files:")?;
        for file in &profile.files {
            writeln!(output, "  - {}", file.display())?;
        }
    }
    Ok(())
}

fn display_files(files: &[PathBuf]) -> String {
    if files.is_empty() {
        "-".to_owned()
    } else {
        files
            .iter()
            .map(|file| file.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn write_migration_report(
    report: &MigrationReport,
    stdout: &mut Output<'_>,
    stderr: &mut Output<'_>,
) -> Result<(), AppError> {
    stdout.write_label_value("Legacy source:", &report.source.display().to_string())?;
    stdout.write_label_value("V2 destination:", &report.destination.display().to_string())?;
    stdout.write_label_value("Mode:", if report.dry_run { "dry-run" } else { "write" })?;
    stdout.write_label_value("Discovered:", &report.discovered.to_string())?;
    stdout.write_label_value(
        "Accepted:",
        &(report.added + report.already_present).to_string(),
    )?;
    stdout.write_label_value("Added:", &report.added.to_string())?;
    stdout.write_label_value("Already present:", &report.already_present.to_string())?;
    stdout.write_label_value("Skipped:", &report.skipped.len().to_string())?;
    stdout.write_label_value("Conflicts:", &report.conflicts.len().to_string())?;
    stdout.write_label_value("Written:", if report.wrote { "yes" } else { "no" })?;

    for issue in &report.skipped {
        writeln!(
            stderr,
            "dstack: legacy line {} skipped{}: {}",
            issue.line,
            issue
                .name
                .as_deref()
                .map(|name| format!(" ('{name}')"))
                .unwrap_or_default(),
            issue.reason
        )?;
    }
    for issue in &report.conflicts {
        writeln!(
            stderr,
            "dstack: legacy line {} conflict{}: {}",
            issue.line,
            issue
                .name
                .as_deref()
                .map(|name| format!(" ('{name}')"))
                .unwrap_or_default(),
            issue.reason
        )?;
    }
    Ok(())
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
        .expect_err("bare invocation should fail");
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
