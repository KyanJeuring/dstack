mod compose;
mod config;
mod container;
mod context;
mod host;
mod image;
mod inspect;
mod lifecycle;
mod migration;
mod network;
mod registry;
mod service_command;
mod session;
mod shell;
mod volume;

use std::{ffi::OsString, path::PathBuf};

use clap::{Parser, Subcommand};

pub use compose::ComposeArgs;
pub use config::{ConfigArgs, ConfigCommand};
pub use container::{
    ContainerArgs, ContainerCommand, ContainerInspectArgs, ContainerIpArgs, ContainerListArgs,
    ContainerPortsArgs, ContainerRemoveArgs, ContainerStatsArgs,
};
pub use context::{ContextArgs, ContextCommand, ContextOptions};
pub use image::{ImageArgs, ImageCommand, ImageInspectArgs, ImageListArgs, ImageRemoveArgs};
pub use inspect::{LogsArgs, ServicesCommandArgs};
pub use lifecycle::{DownArgs, RebuildArgs, RecreateArgs, ServicesArgs, TargetOptions, UpArgs};
pub use migration::{MigrationArgs, MigrationCommand};
pub use network::{
    NetworkArgs, NetworkCommand, NetworkInspectArgs, NetworkListArgs, NetworkRemoveArgs,
};
pub use registry::{RegistryArgs, RegistryCommand};
pub use service_command::{ExecArgs, RunArgs};
pub use session::UseArgs;
pub use shell::{ShellActivateArgs, ShellArgs, ShellCommand, ShellManageArgs, ShellStatusArgs};
pub use volume::{VolumeArgs, VolumeCommand, VolumeInspectArgs, VolumeListArgs, VolumeNamesArgs};

#[derive(Debug, Parser)]
#[command(
  name = "dstack",
  version,
  about = "Docker Compose stack management from anywhere",
  long_about = None
)]
pub struct Cli {
    /// Target a registered or discovered stack by exact name.
    #[arg(short = 's', long = "stack", value_name = "NAME")]
    pub stack: Option<String>,

    /// Select a Compose file explicitly; repeat to preserve a multi-file order.
    #[arg(long = "file", value_name = "FILE", action = clap::ArgAction::Append)]
    pub files: Vec<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show all effective registered and discovered stacks.
    List,
    /// Run raw Docker Compose arguments against the resolved stack.
    Compose(ComposeArgs),
    /// Inspect or explicitly remove host-scoped Docker containers.
    Container(ContainerArgs),
    /// Inspect or explicitly remove host-scoped Docker images.
    Image(ImageArgs),
    /// Inspect or explicitly remove host-scoped Docker volumes.
    Volume(VolumeArgs),
    /// Inspect or explicitly remove host-scoped Docker networks.
    Network(NetworkArgs),
    /// Create and start services in detached, build-aware mode.
    Up(UpArgs),
    /// Start existing service containers.
    Start(ServicesArgs),
    /// Stop service containers without removing them.
    Stop(ServicesArgs),
    /// Restart service containers without recreating them.
    Restart(ServicesArgs),
    /// Stop and remove the stack.
    Down(DownArgs),
    /// Tear down and recreate the whole stack.
    Recreate(RecreateArgs),
    /// Pull service images.
    Pull(ServicesArgs),
    /// Pull images and start updated services.
    Update(ServicesArgs),
    /// Build images and start rebuilt services.
    Rebuild(RebuildArgs),
    /// List services defined by the resolved Compose model.
    Services(ServicesCommandArgs),
    /// Show service logs without following by default.
    Logs(LogsArgs),
    /// Execute a command or open a supported interactive shell when no command is given.
    Exec(ExecArgs),
    /// Run a removable one-off container, using its default command when none is given.
    Run(RunArgs),
    /// Inspect DStack's user configuration.
    Config(ConfigArgs),
    /// Inspect the resolved stack and Compose-file context.
    Context(ContextArgs),
    /// Manage persistent named stack profiles.
    Registry(RegistryArgs),
    /// Explicitly migrate legacy DStack data.
    Migrate(MigrationArgs),
    /// Select a stack for this terminal session (requires shell integration).
    Use(UseArgs),
    /// Clear the active stack from this terminal (requires shell integration).
    Unset,
    /// Install, inspect, or remove terminal shell integration.
    Shell(ShellArgs),
}

impl Cli {
    pub fn parse() -> Self {
        <Self as Parser>::parse_from(normalize_invocation(std::env::args_os()))
    }

    pub fn try_parse_from<I, T>(arguments: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        <Self as Parser>::try_parse_from(normalize_invocation(arguments))
    }
}

fn normalize_invocation<I, T>(arguments: I) -> Vec<OsString>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    normalize_host_options(compose::normalize_invocation(arguments))
}

fn normalize_host_options(arguments: Vec<OsString>) -> Vec<OsString> {
    let Some(command_index) = find_host_command(&arguments) else {
        return arguments;
    };

    let mut root = arguments[..command_index].to_vec();
    let mut command = vec![arguments[command_index].clone()];
    let mut index = command_index + 1;
    while index < arguments.len() {
        let argument = &arguments[index];
        if compose::is_owned_pair(argument) {
            root.push(argument.clone());
            index += 1;
            if let Some(value) = arguments.get(index) {
                root.push(value.clone());
            }
        } else if compose::is_owned_inline(argument) {
            root.push(argument.clone());
        } else {
            command.push(argument.clone());
        }
        index += 1;
    }
    root.extend(command);
    root
}

fn find_host_command(arguments: &[OsString]) -> Option<usize> {
    let mut index = 1;
    while index < arguments.len() {
        let argument = &arguments[index];
        if matches!(
            argument.to_str(),
            Some("container" | "image" | "volume" | "network")
        ) {
            return Some(index);
        }
        if compose::is_owned_pair(argument) {
            index += 2;
        } else if compose::is_owned_inline(argument) {
            index += 1;
        } else {
            return None;
        }
    }
    None
}

#[cfg(test)]
mod tests;
