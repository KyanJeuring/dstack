use clap::{ArgAction, Args, Subcommand};

use super::host::nonempty_filter;

#[derive(Debug, Args)]
pub struct SystemArgs {
    #[command(subcommand)]
    pub command: SystemCommand,
}

#[derive(Debug, Subcommand)]
pub enum SystemCommand {
    /// Show Docker daemon disk usage.
    Df(SystemDfArgs),
    /// Remove stopped containers, unused networks, dangling images, and unused build cache.
    #[command(
        long_about = "Remove stopped containers, unused networks, dangling images, and unused build cache using Docker's native system prune. Docker owns the confirmation prompt unless --force is supplied. --all broadens image cleanup to all unused images. --volumes also includes unused anonymous volumes, not all unused named volumes; use 'dstack volume prune --all' for that separate scope."
    )]
    Prune(SystemPruneArgs),
}

#[derive(Debug, Args)]
pub struct SystemDfArgs {
    /// Show detailed Docker daemon disk usage.
    #[arg(long)]
    pub verbose: bool,
}

#[derive(Debug, Args)]
pub struct SystemPruneArgs {
    /// Remove all unused images, not just dangling images.
    #[arg(long = "all")]
    pub all: bool,

    /// Also prune unused anonymous volumes, not unused named volumes.
    #[arg(long = "volumes")]
    pub volumes: bool,

    /// Forward a Docker system-prune filter; repeat to preserve order.
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
