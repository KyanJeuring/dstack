use std::ffi::OsString;

use clap::{ArgAction, Args, Subcommand};

use super::host::nonempty_filter;

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
    /// Show Docker network addresses for one container or all containers.
    Ip(ContainerIpArgs),
    /// Stream Docker's native live container resource statistics.
    Stats(ContainerStatsArgs),
    /// Inspect explicitly named Docker containers or all containers.
    Inspect(ContainerInspectArgs),
    /// Remove only the explicitly named Docker containers.
    Remove(ContainerRemoveArgs),
    /// Remove stopped Docker containers selected by Docker.
    Prune(ContainerPruneArgs),
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
    #[arg(required_unless_present = "all", conflicts_with = "all")]
    pub container: Option<OsString>,

    /// Include all containers and render one row per network membership.
    #[arg(
        long,
        required_unless_present = "container",
        conflicts_with = "container"
    )]
    pub all: bool,
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
    #[arg(
        value_name = "CONTAINER",
        num_args = 1..,
        required_unless_present = "all",
        conflicts_with = "all"
    )]
    pub containers: Vec<OsString>,

    /// Inspect all Docker containers.
    #[arg(
        long,
        required_unless_present = "containers",
        conflicts_with = "containers"
    )]
    pub all: bool,
}

#[derive(Debug, Args)]
pub struct ContainerRemoveArgs {
    /// Container names or IDs.
    #[arg(value_name = "CONTAINER", required = true, num_args = 1..)]
    pub containers: Vec<OsString>,
}

#[derive(Debug, Args)]
pub struct ContainerPruneArgs {
    /// Forward a Docker prune filter; repeat to preserve order.
    #[arg(
        long = "filter",
        value_name = "FILTER",
        action = ArgAction::Append,
        value_parser = nonempty_filter
    )]
    pub filters: Vec<String>,

    /// Skip Docker's native confirmation prompt.
    #[arg(long = "force")]
    pub force: bool,
}
