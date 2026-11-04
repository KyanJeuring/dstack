use std::process::ExitCode;

use clap::Parser;
use dstack_compose::Cli;

fn main() -> ExitCode {
    match dstack_compose::run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("dstack: {error}");
            error.exit_code()
        }
    }
}
