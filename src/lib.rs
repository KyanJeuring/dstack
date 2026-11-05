pub mod app;
pub mod cli;
pub mod compose;
pub mod config;
pub mod discovery;
pub mod error;
pub mod migration;
pub mod registry;
pub mod resolution;

pub use cli::Cli;
pub use error::AppError;

/// Runs the application boundary after successful CLI parsing.
pub fn run(cli: Cli) -> Result<(), AppError> {
    app::run(cli)
}
