use std::process::ExitCode;

use dstack_compose::Cli;

fn main() -> ExitCode {
    match dstack_compose::run(Cli::parse()) {
        Ok(outcome) => outcome.exit_code(),
        Err(error) => {
            eprintln!("dstack: {error}");
            error.exit_code()
        }
    }
}
