use crate::{Cli, error::AppError};

/// Runs functionality available after CLI parsing.
///
/// Phase 1 implements only Clap-owned help and version output. Reaching this
/// boundary therefore means no supported operation was requested.
pub fn run(_cli: Cli) -> Result<(), AppError> {
    Err(AppError::NoCommand)
}

#[cfg(test)]
mod tests {
    use super::run;
    use crate::{AppError, Cli};

    #[test]
    fn no_command_returns_the_phase_one_error() {
        assert_eq!(run(Cli {}), Err(AppError::NoCommand));
    }
}
