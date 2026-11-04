pub mod app;
pub mod cli;
pub mod config;
pub mod error;
pub mod migration;
pub mod registry;

pub use cli::Cli;
pub use error::AppError;

/// Runs the application boundary after successful CLI parsing.
pub fn run(cli: Cli) -> Result<(), AppError> {
    app::run(cli)
}
