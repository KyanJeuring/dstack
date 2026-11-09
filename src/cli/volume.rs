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
    /// Inspect explicitly named Docker volumes or all volumes.
    Inspect(VolumeInspectArgs),
    /// Remove only the explicitly named Docker volumes.
    Remove(VolumeNamesArgs),
    /// Remove unused anonymous local Docker volumes by default.
    Prune(VolumePruneArgs),
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

#[derive(Debug, Args)]
pub struct VolumeInspectArgs {
    /// Volume names.
    #[arg(
        value_name = "NAME",
        num_args = 1..,
        required_unless_present = "all",
        conflicts_with = "all"
    )]
    pub names: Vec<OsString>,

    /// Inspect all Docker volumes.
    #[arg(long, required_unless_present = "names", conflicts_with = "names")]
    pub all: bool,
}

#[derive(Debug, Args)]
pub struct VolumePruneArgs {
    /// Remove all unused local volumes, including named volumes.
    #[arg(long = "all")]
    pub all: bool,

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
