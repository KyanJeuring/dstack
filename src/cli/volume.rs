use std::ffi::OsString;

use clap::{ArgAction, Args, Subcommand};

use super::host::nonempty_filter;

#[derive(Debug, Args)]
pub struct VolumeArgs {
    #[command(subcommand)]
    pub command: VolumeCommand,
}

#[derive(Debug, Subcommand)]
pub enum VolumeCommand {
    /// List Docker volumes using a DStack-rendered table.
    List(VolumeListArgs),
    /// Stream Docker's native volume inspection JSON.
    Inspect(VolumeNamesArgs),
    /// Remove only the explicitly named Docker volumes.
    Remove(VolumeNamesArgs),
}

#[derive(Debug, Args)]
pub struct VolumeListArgs {
    /// Apply a native Docker volume-list filter; repeat to preserve order.
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
pub struct VolumeNamesArgs {
    /// Volume names.
    #[arg(value_name = "NAME", required = true, num_args = 1..)]
    pub names: Vec<OsString>,
}
