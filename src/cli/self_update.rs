use clap::Args;

#[derive(Debug, Args)]
pub struct SelfUpdateArgs {
    /// Check for a compatible stable v2 release without downloading or replacing anything.
    #[arg(long)]
    pub check: bool,
}
