use std::path::{Path, PathBuf};

use crate::{
    Cli,
    cli::{Command, ConfigCommand, ContextCommand, MigrationCommand, RegistryCommand},
    config::{AppPaths, commands as config_commands},
    context::{commands::show as show_context, resolve_context},
    error::AppError,
    migration::command as migrate_registry,
    output::Output,
    registry::commands as registry_commands,
    stack::commands::list as list_stacks,
};

use super::bootstrap::RuntimeState;

pub(crate) fn run(
    cli: Cli,
    paths: &AppPaths,
    current_dir: &Path,
    runtime: &RuntimeState,
    stdout: &mut Output<'_>,
    stderr: &mut Output<'_>,
) -> Result<(), AppError> {
    let command = cli.command.ok_or(AppError::NoCommand)?;
    validate_global_options(&command, cli.stack.as_deref(), &cli.files)?;
    let root_stack = cli.stack;
    let root_files = cli.files;

    match command {
        Command::List => list_stacks::run(&runtime.config, &runtime.registry, paths, stdout),
        Command::Config(arguments) => match arguments.command {
            ConfigCommand::Path => config_commands::path::run(paths, stdout),
            ConfigCommand::Show => config_commands::show::run(paths, &runtime.config, stdout),
        },
        Command::Context(arguments) => match arguments.command {
            ContextCommand::Show(options) => {
                let stack = merge_stack_options(root_stack, options.stack)?;
                let files = [root_files, options.files].concat();
                let context = resolve_context(
                    &runtime.config,
                    &runtime.registry,
                    paths.home_dir(),
                    stack.as_deref(),
                    &files,
                    current_dir,
                )?;
                show_context::run(context, stdout)
            }
        },
        Command::Registry(arguments) => match arguments.command {
            RegistryCommand::List => registry_commands::list::run(&runtime.registry, stdout),
            RegistryCommand::Show { name } => {
                registry_commands::show::run(&runtime.registry, &name, stdout)
            }
            RegistryCommand::Add { name, path, files } => registry_commands::add::run(
                &runtime.registry_store,
                name,
                path,
                [root_files, files].concat(),
                stdout,
            ),
            RegistryCommand::Remove { name } => {
                registry_commands::remove::run(&runtime.registry_store, &name, stdout)
            }
        },
        Command::Migrate(arguments) => match arguments.command {
            MigrationCommand::Registry { dry_run } => migrate_registry::run(
                &runtime.registry_store,
                paths.legacy_registry(),
                dry_run,
                stdout,
                stderr,
            ),
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
