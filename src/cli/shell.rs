use std::path::PathBuf;

use clap::{Args, Subcommand};

use crate::shell::ShellKind;

#[derive(Debug, Args)]
pub struct ShellArgs {
    #[command(subcommand)]
    pub command: ShellCommand,
}

#[derive(Debug, Subcommand)]
pub enum ShellCommand {
    /// Install DStack terminal integration and static completion in a shell profile.
    Install(ShellManageArgs),

    /// Inspect profile installation and current-adapter state.
    Status(ShellStatusArgs),

    /// Remove only DStack's managed integration block.
    Uninstall(ShellManageArgs),

    #[command(name = "__init", hide = true)]
    Init {
        #[arg(value_enum, value_name = "SHELL")]
        shell: ShellKind,
    },

    #[command(name = "__activate", hide = true)]
    Activate(ShellActivateArgs),

    #[command(name = "__clear", hide = true)]
    Clear {
        #[arg(value_enum, value_name = "SHELL")]
        shell: ShellKind,
    },

    #[command(name = "__completions", hide = true)]
    Completions {
        #[arg(value_enum, value_name = "SHELL")]
        shell: ShellKind,
    },
}

#[derive(Debug, Args)]
pub struct ShellManageArgs {
    /// Override automatic shell detection.
    #[arg(long, value_enum, value_name = "SHELL")]
    pub shell: Option<ShellKind>,

    /// Override the shell profile path.
    #[arg(long, value_name = "PATH")]
    pub profile: Option<PathBuf>,

    /// Report the planned change without writing the profile.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct ShellStatusArgs {
    /// Override automatic shell detection.
    #[arg(long, value_enum, value_name = "SHELL")]
    pub shell: Option<ShellKind>,

    /// Override the shell profile path.
    #[arg(long, value_name = "PATH")]
    pub profile: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ShellActivateArgs {
    #[arg(value_enum, value_name = "SHELL")]
    pub shell: ShellKind,

    #[arg(value_name = "NAME")]
    pub name: String,

    #[arg(long = "file", value_name = "FILE", action = clap::ArgAction::Append)]
    pub files: Vec<PathBuf>,
}
