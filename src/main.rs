use std::process::ExitCode;

use clap::Parser;
use dstack::Cli;

fn main() -> ExitCode {
    match dstack::run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("dstack: {error}");
            error.exit_code()
        }
    }
}
