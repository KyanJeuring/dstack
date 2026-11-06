use std::path::PathBuf;

use clap::{Args, Subcommand};

#[derive(Debug, Args)]
pub struct RegistryArgs {
    #[command(subcommand)]
    pub command: RegistryCommand,
}

#[derive(Debug, Subcommand)]
pub enum RegistryCommand {
    /// List explicitly registered stack profiles.
    List,
    /// Show one registered stack profile.
    Show { name: String },
    /// Add a named stack profile.
    Add {
        name: String,
        path: PathBuf,
        #[arg(long = "file", value_name = "FILE", action = clap::ArgAction::Append)]
        files: Vec<PathBuf>,
    },
    /// Remove a persistent profile without touching project files.
    Remove { name: String },
}
