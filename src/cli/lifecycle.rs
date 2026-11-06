use std::{ffi::OsString, path::PathBuf};

use clap::Args;

#[derive(Debug, Args, Default)]
pub struct TargetOptions {
    /// Target a registered or discovered stack by exact name.
    #[arg(short = 's', long = "stack", value_name = "NAME")]
    pub stack: Option<String>,

    /// Select a Compose file explicitly; repeat to preserve order.
    #[arg(long = "file", value_name = "FILE", action = clap::ArgAction::Append)]
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ServicesArgs {
    #[command(flatten)]
    pub target: TargetOptions,

    /// Limit the operation to these Compose services.
    #[arg(value_name = "SERVICE")]
    pub services: Vec<OsString>,
}

#[derive(Debug, Args)]
pub struct UpArgs {
    #[command(flatten)]
    pub target: TargetOptions,

    /// Remove containers for services no longer defined by the Compose files.
    #[arg(long)]
    pub remove_orphans: bool,

    /// Limit the operation to these Compose services.
    #[arg(value_name = "SERVICE")]
    pub services: Vec<OsString>,
}

#[derive(Debug, Args)]
pub struct DownArgs {
    #[command(flatten)]
    pub target: TargetOptions,

    /// Remove named and anonymous volumes declared by the Compose project.
    #[arg(long)]
    pub volumes: bool,
}

#[derive(Debug, Args)]
pub struct RecreateArgs {
    #[command(flatten)]
    pub target: TargetOptions,

    /// Remove volumes during the down step before recreating the stack.
    #[arg(long)]
    pub volumes: bool,
}

#[derive(Debug, Args)]
pub struct RebuildArgs {
    #[command(flatten)]
    pub target: TargetOptions,

    /// Build without using cached layers.
    #[arg(long)]
    pub no_cache: bool,

    /// Limit the operation to these Compose services.
    #[arg(value_name = "SERVICE")]
    pub services: Vec<OsString>,
}
