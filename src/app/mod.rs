mod bootstrap;
mod dispatch;
mod operations;
mod target;

use std::{
    env,
    io::{self, IsTerminal},
    path::{Path, PathBuf},
};

#[cfg(test)]
use std::io::Write;

use crate::{
    Cli,
    config::AppPaths,
    error::AppError,
    outcome::AppOutcome,
    output::{ColorPolicy, Output},
    process::{NativeProcessExecutor, ProcessExecutor},
};

/// Runs the v2 application after CLI parsing.
pub fn run(cli: Cli) -> Result<AppOutcome, AppError> {
    let paths = AppPaths::discover()?;
    let current_dir =
        env::current_dir().map_err(|source| crate::resolution::ResolutionError::Io {
            action: "read current directory",
            path: PathBuf::from("."),
            source,
        })?;
    let stdout = io::stdout();
    let stderr = io::stderr();
    let stdout_color = ColorPolicy::from_environment(stdout.is_terminal());
    let stderr_color = ColorPolicy::from_environment(stderr.is_terminal());
    let mut stdout = anstream::AutoStream::new(stdout, stdout_color.stream_choice()).lock();
    let mut stderr = anstream::AutoStream::new(stderr, stderr_color.stream_choice()).lock();
    let mut stdout = Output::new(&mut stdout, stdout_color);
    let mut stderr = Output::new(&mut stderr, stderr_color);
    run_with_outputs(
        cli,
        &paths,
        &current_dir,
        &NativeProcessExecutor,
        &mut stdout,
        &mut stderr,
    )
}

#[cfg(test)]
pub(crate) fn run_with_environment(
    cli: Cli,
    paths: &AppPaths,
    current_dir: &Path,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<AppOutcome, AppError> {
    let mut stdout = Output::plain(stdout);
    let mut stderr = Output::plain(stderr);
    run_with_outputs(
        cli,
        paths,
        current_dir,
        &NativeProcessExecutor,
        &mut stdout,
        &mut stderr,
    )
}

#[cfg(test)]
pub(crate) fn run_with_executor(
    cli: Cli,
    paths: &AppPaths,
    current_dir: &Path,
    executor: &dyn ProcessExecutor,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<AppOutcome, AppError> {
    let mut stdout = Output::plain(stdout);
    let mut stderr = Output::plain(stderr);
    run_with_outputs(cli, paths, current_dir, executor, &mut stdout, &mut stderr)
}

fn run_with_outputs(
    cli: Cli,
    paths: &AppPaths,
    current_dir: &Path,
    executor: &dyn ProcessExecutor,
    stdout: &mut Output<'_>,
    stderr: &mut Output<'_>,
) -> Result<AppOutcome, AppError> {
    if cli.command.is_none() {
        return Err(AppError::NoCommand);
    }
    let runtime = bootstrap::run(paths, stderr)?;
    dispatch::run(cli, paths, current_dir, &runtime, executor, stdout, stderr)
}

#[cfg(test)]
mod tests;
