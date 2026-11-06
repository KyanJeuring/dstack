use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct MigrationArgs {
    #[command(subcommand)]
    pub command: MigrationCommand,
}

#[derive(Debug, Subcommand)]
pub enum MigrationCommand {
    /// Import the legacy v1 registry without modifying its source file.
    Registry {
        /// Preview accepted, skipped, and conflicting entries without writing.
        #[arg(long)]
        dry_run: bool,
    },
}
