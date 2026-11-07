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
