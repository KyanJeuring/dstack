use std::path::Path;

use crate::{
    cli::{Command, ContainerCommand, ImageCommand, NetworkCommand, SystemCommand, VolumeCommand},
    container::commands as container_commands,
    error::AppError,
    image::commands as image_commands,
    network::commands as network_commands,
    outcome::AppOutcome,
    output::Output,
    process::{ProcessExecutor, ProcessOutputExecutor},
    system::commands as system_commands,
    volume::commands as volume_commands,
};

pub(super) fn is_host_scoped(command: &Command) -> bool {
    matches!(
        command,
        Command::Container(_)
            | Command::Image(_)
            | Command::Volume(_)
            | Command::Network(_)
            | Command::System(_)
    )
}

pub(super) fn run(
    command: Command,
    current_dir: &Path,
    stream_executor: &dyn ProcessExecutor,
    capture_executor: &dyn ProcessOutputExecutor,
    stdout: &mut Output<'_>,
    stderr: &mut Output<'_>,
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
                arguments.all,
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
            ContainerCommand::Inspect(arguments) => container_commands::inspect::run(
                arguments.all,
                arguments.containers,
                current_dir,
                stream_executor,
                capture_executor,
                stdout,
            ),
            ContainerCommand::Remove(arguments) => {
                container_commands::remove::run(arguments.containers, current_dir, stream_executor)
            }
            ContainerCommand::Stop(arguments) => container_commands::stop::run(
                arguments.all,
                arguments.containers,
                current_dir,
                stream_executor,
                capture_executor,
                stderr,
            ),
            ContainerCommand::Prune(arguments) => container_commands::prune::run(
                arguments.filters,
                arguments.force,
                current_dir,
                stream_executor,
            ),
        },
        Command::Image(arguments) => match arguments.command {
            ImageCommand::List(arguments) => image_commands::list::run(
                arguments.all,
                arguments.filters,
                current_dir,
                capture_executor,
                stdout,
            ),
            ImageCommand::Inspect(arguments) => image_commands::inspect::run(
                arguments.all,
                arguments.images,
                current_dir,
                stream_executor,
                capture_executor,
                stdout,
            ),
            ImageCommand::Remove(arguments) => {
                image_commands::remove::run(arguments.images, current_dir, stream_executor)
            }
            ImageCommand::Prune(arguments) => image_commands::prune::run(
                arguments.all,
                arguments.filters,
                arguments.force,
                current_dir,
                stream_executor,
            ),
        },
        Command::Volume(arguments) => match arguments.command {
            VolumeCommand::List(arguments) => {
                volume_commands::list::run(arguments.filters, current_dir, capture_executor, stdout)
            }
            VolumeCommand::Inspect(arguments) => volume_commands::inspect::run(
                arguments.all,
                arguments.names,
                current_dir,
                stream_executor,
                capture_executor,
                stdout,
            ),
            VolumeCommand::Remove(arguments) => {
                volume_commands::remove::run(arguments.names, current_dir, stream_executor)
            }
            VolumeCommand::Prune(arguments) => volume_commands::prune::run(
                arguments.all,
                arguments.filters,
                arguments.force,
                current_dir,
                stream_executor,
            ),
        },
        Command::Network(arguments) => match arguments.command {
            NetworkCommand::List(arguments) => network_commands::list::run(
                arguments.filters,
                current_dir,
                capture_executor,
                stdout,
            ),
            NetworkCommand::Inspect(arguments) => network_commands::inspect::run(
                arguments.all,
                arguments.names,
                current_dir,
                stream_executor,
                capture_executor,
                stdout,
            ),
            NetworkCommand::Remove(arguments) => {
                network_commands::remove::run(arguments.names, current_dir, stream_executor)
            }
            NetworkCommand::Prune(arguments) => network_commands::prune::run(
                arguments.filters,
                arguments.force,
                current_dir,
                stream_executor,
            ),
        },
        Command::System(arguments) => match arguments.command {
            SystemCommand::Df(arguments) => {
                system_commands::df::run(arguments.verbose, current_dir, stream_executor)
            }
            SystemCommand::Prune(arguments) => system_commands::prune::run(
                arguments.all,
                arguments.volumes,
                arguments.filters,
                arguments.force,
                current_dir,
                stream_executor,
            ),
        },
        _ => unreachable!("non-host command was routed as host-scoped"),
    }
}
