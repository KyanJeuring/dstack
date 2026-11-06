use std::path::{Path, PathBuf};

use crate::{
    Cli,
    cli::{Command, ConfigCommand, ContextCommand, MigrationCommand, RegistryCommand},
    config::{AppPaths, commands as config_commands},
    context::{commands::show as show_context, resolve_context},
    error::AppError,
    migration::command as migrate_registry,
    outcome::AppOutcome,
    output::Output,
    process::ProcessExecutor,
    registry::commands as registry_commands,
    stack::commands::list as list_stacks,
};

use super::{
    bootstrap::RuntimeState,
    operations::{self, Environment, Request},
    target::merge_stack_options,
};

pub(crate) fn run(
    cli: Cli,
    paths: &AppPaths,
    current_dir: &Path,
    runtime: &RuntimeState,
    executor: &dyn ProcessExecutor,
    stdout: &mut Output<'_>,
    stderr: &mut Output<'_>,
) -> Result<AppOutcome, AppError> {
    let command = cli.command.ok_or(AppError::NoCommand)?;
    validate_global_options(&command, cli.stack.as_deref(), &cli.files)?;
    let root_stack = cli.stack;
    let root_files = cli.files;
    let command = match Request::from_command(command) {
        Ok(request) => {
            return run_operation(
                request,
                root_stack,
                root_files,
                paths,
                current_dir,
                runtime,
                executor,
            );
        }
        Err(command) => command,
    };

    match command {
        Command::List => list_stacks::run(&runtime.config, &runtime.registry, paths, stdout)
            .map(|()| AppOutcome::Success),
        Command::Config(arguments) => (match arguments.command {
            ConfigCommand::Path => config_commands::path::run(paths, stdout),
            ConfigCommand::Show => config_commands::show::run(paths, &runtime.config, stdout),
        })
        .map(|()| AppOutcome::Success),
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
                show_context::run(context, stdout).map(|()| AppOutcome::Success)
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
        }
        .map(|()| AppOutcome::Success),
        Command::Migrate(arguments) => match arguments.command {
            MigrationCommand::Registry { dry_run } => migrate_registry::run(
                &runtime.registry_store,
                paths.legacy_registry(),
                dry_run,
                stdout,
                stderr,
            )
            .map(|()| AppOutcome::Success),
        },
        Command::Compose(_)
        | Command::Up(_)
        | Command::Start(_)
        | Command::Stop(_)
        | Command::Restart(_)
        | Command::Down(_)
        | Command::Recreate(_)
        | Command::Pull(_)
        | Command::Update(_)
        | Command::Rebuild(_) => unreachable!("operational command was routed above"),
    }
}

fn run_operation(
    request: Request,
    root_stack: Option<String>,
    root_files: Vec<PathBuf>,
    paths: &AppPaths,
    current_dir: &Path,
    runtime: &RuntimeState,
    executor: &dyn ProcessExecutor,
) -> Result<AppOutcome, AppError> {
    operations::run(
        request,
        root_stack,
        root_files,
        Environment {
            paths,
            current_dir,
            runtime,
            executor,
        },
    )
}

fn validate_global_options(
    command: &Command,
    stack: Option<&str>,
    files: &[PathBuf],
) -> Result<(), AppError> {
    let supports_stack = matches!(
        command,
        Command::Context(_)
            | Command::Compose(_)
            | Command::Up(_)
            | Command::Start(_)
            | Command::Stop(_)
            | Command::Restart(_)
            | Command::Down(_)
            | Command::Recreate(_)
            | Command::Pull(_)
            | Command::Update(_)
            | Command::Rebuild(_)
    );
    let supports_files = matches!(
        command,
        Command::Context(_)
            | Command::Compose(_)
            | Command::Up(_)
            | Command::Start(_)
            | Command::Stop(_)
            | Command::Restart(_)
            | Command::Down(_)
            | Command::Recreate(_)
            | Command::Pull(_)
            | Command::Update(_)
            | Command::Rebuild(_)
            | Command::Registry(crate::cli::RegistryArgs {
                command: RegistryCommand::Add { .. }
            })
    );
    if stack.is_some() && !supports_stack {
        return Err(AppError::InvalidOptions(
            "--stack/-s is only supported by stack-context operations".to_owned(),
        ));
    }
    if !files.is_empty() && !supports_files {
        return Err(AppError::InvalidOptions(
            "--file is only supported by stack-context operations and 'registry add'".to_owned(),
        ));
    }
    Ok(())
}
