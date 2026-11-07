use std::path::Path;

use crate::{
    cli::{Command, ContainerCommand},
    container::commands,
    error::AppError,
    outcome::AppOutcome,
    output::Output,
    process::{ProcessExecutor, ProcessOutputExecutor},
};

pub(super) fn is_host_scoped(command: &Command) -> bool {
    matches!(command, Command::Container(_))
}

pub(super) fn run(
    command: Command,
    current_dir: &Path,
    stream_executor: &dyn ProcessExecutor,
    capture_executor: &dyn ProcessOutputExecutor,
    stdout: &mut Output<'_>,
) -> Result<AppOutcome, AppError> {
    match command {
        Command::Container(arguments) => match arguments.command {
            ContainerCommand::List(arguments) => commands::list::run(
                arguments.all,
                arguments.filters,
                current_dir,
                capture_executor,
                stdout,
            ),
            ContainerCommand::Ports(arguments) => commands::ports::run(
                arguments.container,
                arguments.private_port,
                current_dir,
                stream_executor,
                capture_executor,
                stdout,
            ),
            ContainerCommand::Ip(arguments) => {
                commands::ip::run(arguments.container, current_dir, capture_executor, stdout)
            }
            ContainerCommand::Stats(arguments) => commands::stats::run(
                arguments.all,
                arguments.no_stream,
                arguments.containers,
                current_dir,
                stream_executor,
            ),
            ContainerCommand::Inspect(arguments) => {
                commands::inspect::run(arguments.containers, current_dir, stream_executor)
            }
        },
        _ => unreachable!("non-host command was routed as host-scoped"),
    }
}
