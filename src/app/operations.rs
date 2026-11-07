use std::path::{Path, PathBuf};

use crate::{
    cli::{
        Command, ComposeArgs, DownArgs, ExecArgs, LogsArgs, RebuildArgs, RecreateArgs, RunArgs,
        ServicesArgs, ServicesCommandArgs, TargetOptions, UpArgs,
    },
    compose::commands::{
        down, exec, logs, pull, raw, rebuild, recreate, restart, run as compose_run, services,
        start, stop, up, update,
    },
    config::AppPaths,
    context::{ResolvedContext, resolve_context},
    error::AppError,
    outcome::AppOutcome,
    process::ProcessExecutor,
    session::SessionEnvironment,
};

use super::{bootstrap::RuntimeState, target::merge_stack_options};

pub(crate) enum Request {
    Raw(ComposeArgs),
    Up(UpArgs),
    Start(ServicesArgs),
    Stop(ServicesArgs),
    Restart(ServicesArgs),
    Down(DownArgs),
    Recreate(RecreateArgs),
    Pull(ServicesArgs),
    Update(ServicesArgs),
    Rebuild(RebuildArgs),
    Services(ServicesCommandArgs),
    Logs(LogsArgs),
    Exec(ExecArgs),
    Run(RunArgs),
}

impl Request {
    pub(crate) fn from_command(command: Command) -> Result<Self, Command> {
        match command {
            Command::Compose(arguments) => Ok(Self::Raw(arguments)),
            Command::Up(arguments) => Ok(Self::Up(arguments)),
            Command::Start(arguments) => Ok(Self::Start(arguments)),
            Command::Stop(arguments) => Ok(Self::Stop(arguments)),
            Command::Restart(arguments) => Ok(Self::Restart(arguments)),
            Command::Down(arguments) => Ok(Self::Down(arguments)),
            Command::Recreate(arguments) => Ok(Self::Recreate(arguments)),
            Command::Pull(arguments) => Ok(Self::Pull(arguments)),
            Command::Update(arguments) => Ok(Self::Update(arguments)),
            Command::Rebuild(arguments) => Ok(Self::Rebuild(arguments)),
            Command::Services(arguments) => Ok(Self::Services(arguments)),
            Command::Logs(arguments) => Ok(Self::Logs(arguments)),
            Command::Exec(arguments) => Ok(Self::Exec(arguments)),
            Command::Run(arguments) => Ok(Self::Run(arguments)),
            command => Err(command),
        }
    }
}

pub(crate) struct Environment<'a> {
    pub(crate) paths: &'a AppPaths,
    pub(crate) current_dir: &'a Path,
    pub(crate) runtime: &'a RuntimeState,
    pub(crate) executor: &'a dyn ProcessExecutor,
    pub(crate) session: &'a SessionEnvironment,
}

pub(crate) fn run(
    request: Request,
    root_stack: Option<String>,
    root_files: Vec<PathBuf>,
    environment: Environment<'_>,
) -> Result<AppOutcome, AppError> {
    match request {
        Request::Raw(arguments) => {
            let context = resolve(
                &environment,
                root_stack,
                root_files,
                TargetOptions::default(),
            )?;
            raw::run(&context, arguments.args, environment.executor)
        }
        Request::Up(arguments) => {
            let context = resolve(&environment, root_stack, root_files, arguments.target)?;
            up::run(
                &context,
                arguments.services,
                arguments.remove_orphans,
                environment.executor,
            )
        }
        Request::Start(arguments) => {
            run_services(arguments, root_stack, root_files, &environment, start::run)
        }
        Request::Stop(arguments) => {
            run_services(arguments, root_stack, root_files, &environment, stop::run)
        }
        Request::Restart(arguments) => run_services(
            arguments,
            root_stack,
            root_files,
            &environment,
            restart::run,
        ),
        Request::Down(arguments) => {
            let context = resolve(&environment, root_stack, root_files, arguments.target)?;
            down::run(&context, arguments.volumes, environment.executor)
        }
        Request::Recreate(arguments) => {
            let context = resolve(&environment, root_stack, root_files, arguments.target)?;
            recreate::run(&context, arguments.volumes, environment.executor)
        }
        Request::Pull(arguments) => {
            run_services(arguments, root_stack, root_files, &environment, pull::run)
        }
        Request::Update(arguments) => {
            run_services(arguments, root_stack, root_files, &environment, update::run)
        }
        Request::Rebuild(arguments) => {
            let context = resolve(&environment, root_stack, root_files, arguments.target)?;
            rebuild::run(
                &context,
                arguments.services,
                arguments.no_cache,
                environment.executor,
            )
        }
        Request::Services(arguments) => {
            let context = resolve(&environment, root_stack, root_files, arguments.target)?;
            services::run(&context, environment.executor)
        }
        Request::Logs(arguments) => {
            let context = resolve(&environment, root_stack, root_files, arguments.target)?;
            logs::run(
                &context,
                arguments.services,
                arguments.follow,
                arguments.tail,
                environment.executor,
            )
        }
        Request::Exec(arguments) => {
            let context = resolve(&environment, root_stack, root_files, arguments.target)?;
            exec::run(
                &context,
                arguments.service,
                arguments.command,
                environment.executor,
            )
        }
        Request::Run(arguments) => {
            let context = resolve(&environment, root_stack, root_files, arguments.target)?;
            compose_run::run(
                &context,
                arguments.service,
                arguments.command,
                environment.executor,
            )
        }
    }
}

type ServiceHandler = fn(
    &ResolvedContext,
    Vec<std::ffi::OsString>,
    &dyn ProcessExecutor,
) -> Result<AppOutcome, AppError>;

fn run_services(
    arguments: ServicesArgs,
    root_stack: Option<String>,
    root_files: Vec<PathBuf>,
    environment: &Environment<'_>,
    handler: ServiceHandler,
) -> Result<AppOutcome, AppError> {
    let context = resolve(environment, root_stack, root_files, arguments.target)?;
    handler(&context, arguments.services, environment.executor)
}

fn resolve(
    environment: &Environment<'_>,
    root_stack: Option<String>,
    root_files: Vec<PathBuf>,
    target: TargetOptions,
) -> Result<ResolvedContext, AppError> {
    let stack = merge_stack_options(root_stack, target.stack)?;
    let files = [root_files, target.files].concat();
    resolve_context(
        &environment.runtime.config,
        &environment.runtime.registry,
        environment.paths.home_dir(),
        stack.as_deref(),
        &files,
        environment.current_dir,
        environment.session,
    )
}
