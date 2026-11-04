use std::io::{self, Write};

use crate::{
    Cli,
    cli::{Command, MigrationCommand, RegistryCommand},
    config::AppPaths,
    error::AppError,
    migration::{MigrationReport, migrate_registry},
    registry::{RegistryStore, StackProfile},
};

/// Runs the v2 application after CLI parsing.
pub fn run(cli: Cli) -> Result<(), AppError> {
    let paths = AppPaths::discover()?;
    let stdout = io::stdout();
    let stderr = io::stderr();
    run_with_paths(cli, &paths, &mut stdout.lock(), &mut stderr.lock())
}

pub(crate) fn run_with_paths(
    cli: Cli,
    paths: &AppPaths,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), AppError> {
    let command = cli.command.ok_or(AppError::NoCommand)?;
    let store = RegistryStore::new(
        paths.registry_file().to_path_buf(),
        paths.registry_lock().to_path_buf(),
    );

    match command {
        Command::Registry(arguments) => match arguments.command {
            RegistryCommand::List => list_registry(&store, stdout),
            RegistryCommand::Show { name } => show_profile(&store, &name, stdout),
            RegistryCommand::Add { name, path, files } => {
                let profile = StackProfile::for_registration(name, path, files)?;
                let registered_name = profile.name.clone();
                store.add(profile)?;
                writeln!(stdout, "Registered stack profile '{registered_name}'.")?;
                Ok(())
            }
            RegistryCommand::Remove { name } => {
                store.remove(&name)?;
                writeln!(stdout, "Removed stack profile '{name}'.")?;
                Ok(())
            }
        },
        Command::Migrate(arguments) => match arguments.command {
            MigrationCommand::Registry { dry_run } => {
                let report = migrate_registry(&store, paths.legacy_registry(), dry_run)?;
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

fn list_registry(store: &RegistryStore, output: &mut dyn Write) -> Result<(), AppError> {
    let registry = store.load()?;
    writeln!(output, "NAME\tPATH\tFILES")?;
    for profile in registry.stacks {
        writeln!(
            output,
            "{}\t{}\t{}",
            profile.name,
            profile.path.display(),
            display_files(&profile.files)
        )?;
    }
    Ok(())
}

fn show_profile(store: &RegistryStore, name: &str, output: &mut dyn Write) -> Result<(), AppError> {
    let registry = store.load()?;
    let profile = registry
        .get(name)
        .ok_or_else(|| crate::registry::RegistryError::UnknownProfile(name.to_owned()))?;
    writeln!(output, "Name: {}", profile.name)?;
    writeln!(output, "Path: {}", profile.path.display())?;
    if profile.files.is_empty() {
        writeln!(output, "Files: -")?;
    } else {
        writeln!(output, "Files:")?;
        for file in &profile.files {
            writeln!(output, "  - {}", file.display())?;
        }
    }
    Ok(())
}

fn display_files(files: &[std::path::PathBuf]) -> String {
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
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<(), AppError> {
    writeln!(stdout, "Legacy source: {}", report.source.display())?;
    writeln!(stdout, "V2 destination: {}", report.destination.display())?;
    writeln!(
        stdout,
        "Mode: {}",
        if report.dry_run { "dry-run" } else { "write" }
    )?;
    writeln!(stdout, "Discovered: {}", report.discovered)?;
    writeln!(
        stdout,
        "Accepted: {}",
        report.added + report.already_present
    )?;
    writeln!(stdout, "Added: {}", report.added)?;
    writeln!(stdout, "Already present: {}", report.already_present)?;
    writeln!(stdout, "Skipped: {}", report.skipped.len())?;
    writeln!(stdout, "Conflicts: {}", report.conflicts.len())?;
    writeln!(
        stdout,
        "Written: {}",
        if report.wrote { "yes" } else { "no" }
    )?;

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
    use std::path::PathBuf;

    use clap::Parser;
    use tempfile::TempDir;

    use super::run_with_paths;
    use crate::{AppError, Cli, config::AppPaths};

    #[test]
    fn no_command_returns_usage_error() {
        let temp = TempDir::new().unwrap();
        let paths = AppPaths::new(temp.path().join("v2"), temp.path().join("v1/registry"));
        let error = run_with_paths(
            Cli { command: None },
            &paths,
            &mut Vec::new(),
            &mut Vec::new(),
        )
        .expect_err("bare invocation should fail");
        assert!(matches!(error, AppError::NoCommand));
    }

    #[test]
    fn list_uses_injected_paths_and_writes_stable_header() {
        let temp = TempDir::new().unwrap();
        let paths = AppPaths::new(temp.path().join("v2"), PathBuf::from("unused-legacy"));
        let cli = Cli::try_parse_from(["dstack", "registry", "list"]).unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        run_with_paths(cli, &paths, &mut stdout, &mut stderr).unwrap();

        assert_eq!(stdout, b"NAME\tPATH\tFILES\n");
        assert!(stderr.is_empty());
        assert!(!paths.registry_file().exists());
    }
}
