pub mod app;
pub mod cli;
pub mod error;

pub use cli::Cli;
pub use error::AppError;

/// Runs the Phase 1 application boundary after successful CLI parsing.
pub fn run(cli: Cli) -> Result<(), AppError> {
    app::run(cli)
}
