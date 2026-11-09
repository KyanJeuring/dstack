use std::ffi::OsString;

use clap::{ArgAction, Args, Subcommand};

use super::host::nonempty_filter;

#[derive(Debug, Args)]
pub struct ImageArgs {
    #[command(subcommand)]
    pub command: ImageCommand,
}

#[derive(Debug, Subcommand)]
pub enum ImageCommand {
    /// List Docker images using a DStack-rendered table.
    List(ImageListArgs),
    /// Inspect explicitly named Docker images or all images.
    Inspect(ImageInspectArgs),
    /// Remove only the explicitly named Docker images.
    Remove(ImageRemoveArgs),
    /// Remove dangling Docker images by default.
    Prune(ImagePruneArgs),
}

#[derive(Debug, Args)]
pub struct ImageListArgs {
    /// Include intermediate images.
    #[arg(short = 'a', long = "all")]
    pub all: bool,

    /// Apply a native Docker image-list filter; repeat to preserve order.
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
pub struct ImageInspectArgs {
    /// Image names or IDs.
    #[arg(
        value_name = "IMAGE",
        num_args = 1..,
        required_unless_present = "all",
        conflicts_with = "all"
    )]
    pub images: Vec<OsString>,

    /// Inspect all Docker images.
    #[arg(long, required_unless_present = "images", conflicts_with = "images")]
    pub all: bool,
}

#[derive(Debug, Args)]
pub struct ImageRemoveArgs {
    /// Image names or IDs.
    #[arg(value_name = "IMAGE", required = true, num_args = 1..)]
    pub images: Vec<OsString>,
}

#[derive(Debug, Args)]
pub struct ImagePruneArgs {
    /// Remove all unused images, not just dangling images.
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
