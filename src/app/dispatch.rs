use std::path::{Path, PathBuf};

use crate::{
    Cli,
    cli::{
        Command, ConfigCommand, ContextCommand, MigrationCommand, RegistryCommand, ShellCommand,
    },
    config::{AppPaths, commands as config_commands},
    context::{commands::show as show_context, resolve_context},
    error::AppError,
    migration::command as migrate_registry,
    outcome::AppOutcome,
    output::Output,
    process::ProcessExecutor,
    registry::commands as registry_commands,
    session::{SessionEnvironment, commands as session_commands},
    stack::commands::list as list_stacks,
};

use super::{
    bootstrap::RuntimeState,
    operations::{self, Environment as OperationEnvironment, Request},
    target::merge_stack_options,
};

pub(crate) struct Environment<'a> {
    pub(crate) paths: &'a AppPaths,
    pub(crate) current_dir: &'a Path,
    pub(crate) runtime: &'a RuntimeState,
    pub(crate) executor: &'a dyn ProcessExecutor,
    pub(crate) session: &'a SessionEnvironment,
}

pub(crate) fn run(
    cli: Cli,
    environment: Environment<'_>,
    stdout: &mut Output<'_>,
    stderr: &mut Output<'_>,
) -> Result<AppOutcome, AppError> {
    let command = cli.command.ok_or(AppError::NoCommand)?;
    validate_global_options(&command, cli.stack.as_deref(), &cli.files)?;
    let root_stack = cli.stack;
    let root_files = cli.files;
    let command = match Request::from_command(command) {
        Ok(request) => {
            return run_operation(request, root_stack, root_files, &environment);
        }
        Err(command) => command,
    };

    match command {
        Command::List => list_stacks::run(
            &environment.runtime.config,
            &environment.runtime.registry,
            environment.paths,
            stdout,
        )
        .map(|()| AppOutcome::Success),
        Command::Config(arguments) => (match arguments.command {
            ConfigCommand::Path => config_commands::path::run(environment.paths, stdout),
            ConfigCommand::Show => {
                config_commands::show::run(environment.paths, &environment.runtime.config, stdout)
            }
        })
        .map(|()| AppOutcome::Success),
        Command::Context(arguments) => match arguments.command {
            ContextCommand::Show(options) => {
                let stack = merge_stack_options(root_stack, options.stack)?;
                let files = [root_files, options.files].concat();
                let context = resolve_context(
                    &environment.runtime.config,
                    &environment.runtime.registry,
                    environment.paths.home_dir(),
                    stack.as_deref(),
                    &files,
                    environment.current_dir,
                    environment.session,
                )?;
                show_context::run(context, stdout).map(|()| AppOutcome::Success)
            }
        },
        Command::Registry(arguments) => match arguments.command {
            RegistryCommand::List => {
                registry_commands::list::run(&environment.runtime.registry, stdout)
            }
            RegistryCommand::Show { name } => {
                registry_commands::show::run(&environment.runtime.registry, &name, stdout)
            }
            RegistryCommand::Add { name, path, files } => registry_commands::add::run(
                &environment.runtime.registry_store,
                name,
                path,
                [root_files, files].concat(),
                stdout,
            ),
            RegistryCommand::Remove { name } => {
                registry_commands::remove::run(&environment.runtime.registry_store, &name, stdout)
            }
        }
        .map(|()| AppOutcome::Success),
        Command::Migrate(arguments) => match arguments.command {
            MigrationCommand::Registry { dry_run } => migrate_registry::run(
                &environment.runtime.registry_store,
                environment.paths.legacy_registry(),
                dry_run,
                stdout,
                stderr,
            )
            .map(|()| AppOutcome::Success),
        },
        Command::Shell(arguments) => match arguments.command {
            ShellCommand::Activate(arguments) => session_commands::use_stack::run(
                session_commands::use_stack::Environment {
                    config: &environment.runtime.config,
                    registry: &environment.runtime.registry,
                    home_dir: environment.paths.home_dir(),
                },
                session_commands::use_stack::Request {
                    name: &arguments.name,
                    explicit_files: &arguments.files,
                    shell: arguments.shell,
                },
                stdout,
                stderr,
            )
            .map(|()| AppOutcome::Success),
            ShellCommand::Install(_)
            | ShellCommand::Status(_)
            | ShellCommand::Uninstall(_)
            | ShellCommand::Init { .. }
            | ShellCommand::Clear { .. } => {
                unreachable!("early shell command was routed after bootstrap")
            }
        },
        Command::Container(_)
        | Command::Image(_)
        | Command::Volume(_)
        | Command::Network(_)
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
        | Command::Services(_)
        | Command::Logs(_)
        | Command::Exec(_)
        | Command::Run(_) => unreachable!("operational command was routed above"),
        Command::Use(_) | Command::Unset => {
            unreachable!("public session command was routed after the early boundary")
        }
    }
}

fn run_operation(
    request: Request,
    root_stack: Option<String>,
    root_files: Vec<PathBuf>,
    environment: &Environment<'_>,
) -> Result<AppOutcome, AppError> {
    operations::run(
        request,
        root_stack,
        root_files,
        OperationEnvironment {
            paths: environment.paths,
            current_dir: environment.current_dir,
            runtime: environment.runtime,
            executor: environment.executor,
            session: environment.session,
        },
    )
}

pub(super) fn validate_global_options(
    command: &Command,
    stack: Option<&str>,
    files: &[PathBuf],
) -> Result<(), AppError> {
    if matches!(
        command,
        Command::Container(_) | Command::Image(_) | Command::Volume(_) | Command::Network(_)
    ) && (stack.is_some() || !files.is_empty())
    {
        return Err(AppError::InvalidOptions(
            "container/image/volume/network commands are host-scoped and do not accept --stack or --file"
                .to_owned(),
        ));
    }
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
            | Command::Services(_)
            | Command::Logs(_)
            | Command::Exec(_)
            | Command::Run(_)
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
            | Command::Services(_)
            | Command::Logs(_)
            | Command::Exec(_)
            | Command::Run(_)
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
