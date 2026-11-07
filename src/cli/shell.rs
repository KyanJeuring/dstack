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
    /// Emit the adapter source for one explicitly selected shell.
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
}

impl ShellCommand {
    pub fn shell(&self) -> ShellKind {
        match self {
            Self::Init { shell } | Self::Clear { shell } => *shell,
            Self::Activate(arguments) => arguments.shell,
        }
    }
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
