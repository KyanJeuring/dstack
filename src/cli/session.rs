use std::path::PathBuf;

use clap::Args;

#[derive(Debug, Args)]
pub struct UseArgs {
    /// Exact registered or unambiguous discovered stack name.
    #[arg(value_name = "NAME")]
    pub name: String,

    /// Select a Compose file explicitly; repeat to preserve order.
    #[arg(long = "file", value_name = "FILE", action = clap::ArgAction::Append)]
    pub files: Vec<PathBuf>,
}
