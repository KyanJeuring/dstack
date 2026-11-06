use std::ffi::OsString;

use clap::Args;

use super::TargetOptions;

#[derive(Debug, Args)]
#[command(
    override_usage = "dstack exec [OPTIONS] SERVICE\n       dstack exec [OPTIONS] SERVICE -- COMMAND...",
    after_help = "Examples:\n  dstack exec php\n  dstack exec php -- bash\n  dstack exec app -- sh -c 'echo hello && echo world'"
)]
pub struct ExecArgs {
    #[command(flatten)]
    pub target: TargetOptions,

    /// Compose service whose running container should receive the command.
    #[arg(value_name = "SERVICE")]
    pub service: OsString,

    /// Exact command argv. Separate it from DStack arguments with `--`.
    #[arg(value_name = "COMMAND", num_args = 0.., last = true, allow_hyphen_values = true)]
    pub command: Vec<OsString>,
}

#[derive(Debug, Args)]
#[command(
    override_usage = "dstack run [OPTIONS] SERVICE\n       dstack run [OPTIONS] SERVICE -- COMMAND...",
    after_help = "Examples:\n  dstack run worker\n  dstack run app -- bash\n  dstack run app -- sh -c 'echo hello'"
)]
pub struct RunArgs {
    #[command(flatten)]
    pub target: TargetOptions,

    /// Compose service to run in a one-off container.
    #[arg(value_name = "SERVICE")]
    pub service: OsString,

    /// Exact command argv. Separate it from DStack arguments with `--`.
    #[arg(value_name = "COMMAND", num_args = 0.., last = true, allow_hyphen_values = true)]
    pub command: Vec<OsString>,
}
