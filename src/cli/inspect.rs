use std::ffi::OsString;

use clap::Args;

use super::TargetOptions;

#[derive(Debug, Args)]
#[command(after_help = "Example:\n  dstack services")]
pub struct ServicesCommandArgs {
    #[command(flatten)]
    pub target: TargetOptions,
}

#[derive(Debug, Args)]
#[command(
    after_help = "Examples:\n  dstack logs\n  dstack logs php nginx\n  dstack logs --follow php\n  dstack logs --tail 50 php"
)]
pub struct LogsArgs {
    #[command(flatten)]
    pub target: TargetOptions,

    /// Continue streaming new log output until interrupted.
    #[arg(short = 'f', long)]
    pub follow: bool,

    /// Show this many lines from the end of each service log.
    #[arg(long, value_name = "N", default_value_t = 100)]
    pub tail: u64,

    /// Limit output to these Compose services, in the given order.
    #[arg(value_name = "SERVICE")]
    pub services: Vec<OsString>,
}
