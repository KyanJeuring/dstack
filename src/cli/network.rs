use std::ffi::OsString;

use clap::{ArgAction, Args, Subcommand};

use super::host::nonempty_filter;

#[derive(Debug, Args)]
pub struct NetworkArgs {
    #[command(subcommand)]
    pub command: NetworkCommand,
}

#[derive(Debug, Subcommand)]
pub enum NetworkCommand {
    /// List Docker networks using a DStack-rendered table.
    List(NetworkListArgs),
    /// Stream Docker's native network inspection JSON.
    Inspect(NetworkInspectArgs),
    /// Remove only the explicitly named Docker networks.
    Remove(NetworkRemoveArgs),
}

#[derive(Debug, Args)]
pub struct NetworkListArgs {
    /// Apply a native Docker network-list filter; repeat to preserve order.
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
pub struct NetworkInspectArgs {
    /// Network names or IDs.
    #[arg(value_name = "NAME", required = true, num_args = 1..)]
    pub names: Vec<OsString>,
}

#[derive(Debug, Args)]
pub struct NetworkRemoveArgs {
    /// Network names or IDs.
    #[arg(value_name = "NAME", required = true, num_args = 1..)]
    pub names: Vec<OsString>,
}
