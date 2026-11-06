mod compose;
mod config;
mod context;
mod inspect;
mod lifecycle;
mod migration;
mod registry;
mod service_command;

use std::{ffi::OsString, path::PathBuf};

use clap::{Parser, Subcommand};

pub use compose::ComposeArgs;
pub use config::{ConfigArgs, ConfigCommand};
pub use context::{ContextArgs, ContextCommand, ContextOptions};
pub use inspect::{LogsArgs, ServicesCommandArgs};
pub use lifecycle::{DownArgs, RebuildArgs, RecreateArgs, ServicesArgs, TargetOptions, UpArgs};
pub use migration::{MigrationArgs, MigrationCommand};
pub use registry::{RegistryArgs, RegistryCommand};
pub use service_command::{ExecArgs, RunArgs};

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
}

impl Cli {
    pub fn parse() -> Self {
        <Self as Parser>::parse_from(compose::normalize_invocation(std::env::args_os()))
    }

    pub fn try_parse_from<I, T>(arguments: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        <Self as Parser>::try_parse_from(compose::normalize_invocation(arguments))
    }
}

#[cfg(test)]
mod tests;
