use std::ffi::OsString;

use clap::{ArgAction, Args, Subcommand};

#[derive(Debug, Args)]
pub struct ContainerArgs {
    #[command(subcommand)]
    pub command: ContainerCommand,
}

#[derive(Debug, Subcommand)]
pub enum ContainerCommand {
    /// List running containers using a DStack-rendered table.
    List(ContainerListArgs),
    /// Show published ports for running containers or query one container directly.
    Ports(ContainerPortsArgs),
    /// Show a container's address on each attached Docker network.
    Ip(ContainerIpArgs),
    /// Stream Docker's native live container resource statistics.
    Stats(ContainerStatsArgs),
    /// Stream Docker's native container inspection JSON.
    Inspect(ContainerInspectArgs),
}

#[derive(Debug, Args)]
pub struct ContainerListArgs {
    /// Include stopped containers.
    #[arg(short = 'a', long = "all")]
    pub all: bool,

    /// Apply a native Docker container-list filter; repeat to preserve order.
    #[arg(
        short = 'f',
        long = "filter",
        value_name = "FILTER",
        action = ArgAction::Append,
        allow_hyphen_values = true,
        value_parser = nonempty_filter
    )]
    pub filters: Vec<String>,
}

#[derive(Debug, Args)]
pub struct ContainerPortsArgs {
    /// Container name or ID. Omit to list ports for all running containers.
    pub container: Option<OsString>,
    /// Optional private port, such as 8096/tcp.
    pub private_port: Option<OsString>,
}

#[derive(Debug, Args)]
pub struct ContainerIpArgs {
    /// Container name or ID.
    pub container: OsString,
}

#[derive(Debug, Args)]
pub struct ContainerStatsArgs {
    /// Include stopped containers.
    #[arg(short = 'a', long = "all")]
    pub all: bool,

    /// Print one statistics sample and exit.
    #[arg(long = "no-stream")]
    pub no_stream: bool,

    /// Limit statistics to these containers.
    #[arg(value_name = "CONTAINER")]
    pub containers: Vec<OsString>,
}

#[derive(Debug, Args)]
pub struct ContainerInspectArgs {
    /// Container names or IDs.
    #[arg(value_name = "CONTAINER", required = true, num_args = 1..)]
    pub containers: Vec<OsString>,
}

fn nonempty_filter(value: &str) -> Result<String, String> {
    if value.is_empty() {
        Err("filter must not be empty".to_owned())
    } else {
        Ok(value.to_owned())
    }
}
