pub mod app;
pub mod cli;
pub mod compose;
pub mod config;
pub mod container;
pub mod context;
pub mod discovery;
pub mod docker;
pub mod error;
pub mod image;
pub mod migration;
pub mod network;
pub mod outcome;
pub mod output;
pub mod process;
pub mod registry;
pub mod resolution;
pub mod session;
pub mod shell;
pub mod stack;
pub mod volume;

pub use cli::Cli;
pub use error::AppError;
pub use outcome::AppOutcome;

/// Runs the application boundary after successful CLI parsing.
pub fn run(cli: Cli) -> Result<AppOutcome, AppError> {
    app::run(cli)
}
