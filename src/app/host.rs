use std::path::Path;

use crate::{
    cli::{Command, ContainerCommand, ImageCommand, NetworkCommand, VolumeCommand},
    container::commands as container_commands,
    error::AppError,
    image::commands as image_commands,
    network::commands as network_commands,
    outcome::AppOutcome,
    output::Output,
    process::{ProcessExecutor, ProcessOutputExecutor},
    volume::commands as volume_commands,
};

pub(super) fn is_host_scoped(command: &Command) -> bool {
    matches!(
        command,
        Command::Container(_) | Command::Image(_) | Command::Volume(_) | Command::Network(_)
    )
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
            ContainerCommand::List(arguments) => container_commands::list::run(
                arguments.all,
                arguments.filters,
                current_dir,
                capture_executor,
                stdout,
            ),
            ContainerCommand::Ports(arguments) => container_commands::ports::run(
                arguments.container,
                arguments.private_port,
                current_dir,
                stream_executor,
                capture_executor,
                stdout,
            ),
            ContainerCommand::Ip(arguments) => container_commands::ip::run(
                arguments.container,
                current_dir,
                capture_executor,
                stdout,
            ),
            ContainerCommand::Stats(arguments) => container_commands::stats::run(
                arguments.all,
                arguments.no_stream,
                arguments.containers,
                current_dir,
                stream_executor,
            ),
            ContainerCommand::Inspect(arguments) => {
                container_commands::inspect::run(arguments.containers, current_dir, stream_executor)
            }
            ContainerCommand::Remove(arguments) => {
                container_commands::remove::run(arguments.containers, current_dir, stream_executor)
            }
        },
        Command::Image(arguments) => match arguments.command {
            ImageCommand::List(arguments) => image_commands::list::run(
                arguments.all,
                arguments.filters,
                current_dir,
                capture_executor,
                stdout,
            ),
            ImageCommand::Inspect(arguments) => {
                image_commands::inspect::run(arguments.images, current_dir, stream_executor)
            }
            ImageCommand::Remove(arguments) => {
                image_commands::remove::run(arguments.images, current_dir, stream_executor)
            }
        },
        Command::Volume(arguments) => match arguments.command {
            VolumeCommand::List(arguments) => {
                volume_commands::list::run(arguments.filters, current_dir, capture_executor, stdout)
            }
            VolumeCommand::Inspect(arguments) => {
                volume_commands::inspect::run(arguments.names, current_dir, stream_executor)
            }
            VolumeCommand::Remove(arguments) => {
                volume_commands::remove::run(arguments.names, current_dir, stream_executor)
            }
        },
        Command::Network(arguments) => match arguments.command {
            NetworkCommand::List(arguments) => network_commands::list::run(
                arguments.filters,
                current_dir,
                capture_executor,
                stdout,
            ),
            NetworkCommand::Inspect(arguments) => {
                network_commands::inspect::run(arguments.names, current_dir, stream_executor)
            }
            NetworkCommand::Remove(arguments) => {
                network_commands::remove::run(arguments.names, current_dir, stream_executor)
            }
        },
        _ => unreachable!("non-host command was routed as host-scoped"),
    }
}
