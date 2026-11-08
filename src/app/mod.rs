mod bootstrap;
mod dispatch;
mod early;
mod host;
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
    process::{NativeProcessExecutor, ProcessExecutor, ProcessOutputExecutor},
    session::SessionEnvironment,
};

/// Runs the v2 application after CLI parsing.
pub fn run(cli: Cli) -> Result<AppOutcome, AppError> {
    let stdout = io::stdout();
    let stderr = io::stderr();
    let stdout_color = ColorPolicy::from_environment(stdout.is_terminal());
    let stderr_color = ColorPolicy::from_environment(stderr.is_terminal());
    let mut stdout = anstream::AutoStream::new(stdout, stdout_color.stream_choice()).lock();
    let mut stderr = anstream::AutoStream::new(stderr, stderr_color.stream_choice()).lock();
    let mut stdout = Output::new(&mut stdout, stdout_color);
    let mut stderr = Output::new(&mut stderr, stderr_color);
    if let Some(outcome) = early::run(&cli, &mut stdout, &mut stderr)? {
        return Ok(outcome);
    }
    let paths = AppPaths::discover()?;
    let current_dir =
        env::current_dir().map_err(|source| crate::resolution::ResolutionError::Io {
            action: "read current directory",
            path: PathBuf::from("."),
            source,
        })?;
    let session = SessionEnvironment::from_process();
    run_with_outputs(
        cli,
        &paths,
        &current_dir,
        &NativeProcessExecutor,
        &session,
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
    if let Some(outcome) = early::run(&cli, &mut stdout, &mut stderr)? {
        return Ok(outcome);
    }
    run_with_outputs(
        cli,
        paths,
        current_dir,
        &NativeProcessExecutor,
        &SessionEnvironment::absent(),
        &mut stdout,
        &mut stderr,
    )
}

#[cfg(test)]
pub(crate) fn run_with_executor<E>(
    cli: Cli,
    paths: &AppPaths,
    current_dir: &Path,
    executor: &E,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<AppOutcome, AppError>
where
    E: ProcessExecutor + ProcessOutputExecutor,
{
    let mut stdout = Output::plain(stdout);
    let mut stderr = Output::plain(stderr);
    if let Some(outcome) = early::run(&cli, &mut stdout, &mut stderr)? {
        return Ok(outcome);
    }
    run_with_outputs(
        cli,
        paths,
        current_dir,
        executor,
        &SessionEnvironment::absent(),
        &mut stdout,
        &mut stderr,
    )
}

#[cfg(test)]
pub(crate) fn run_with_session_executor<E>(
    cli: Cli,
    paths: &AppPaths,
    current_dir: &Path,
    executor: &E,
    session: &SessionEnvironment,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<AppOutcome, AppError>
where
    E: ProcessExecutor + ProcessOutputExecutor,
{
    let mut stdout = Output::plain(stdout);
    let mut stderr = Output::plain(stderr);
    if let Some(outcome) = early::run(&cli, &mut stdout, &mut stderr)? {
        return Ok(outcome);
    }
    run_with_outputs(
        cli,
        paths,
        current_dir,
        executor,
        session,
        &mut stdout,
        &mut stderr,
    )
}

fn run_with_outputs<E>(
    cli: Cli,
    paths: &AppPaths,
    current_dir: &Path,
    executor: &E,
    session: &SessionEnvironment,
    stdout: &mut Output<'_>,
    stderr: &mut Output<'_>,
) -> Result<AppOutcome, AppError>
where
    E: ProcessExecutor + ProcessOutputExecutor,
{
    if cli.command.is_none() {
        return Err(AppError::NoCommand);
    }
    dispatch::validate_global_options(
        cli.command.as_ref().expect("command checked above"),
        cli.stack.as_deref(),
        &cli.files,
    )?;
    if host::is_host_scoped(cli.command.as_ref().expect("command checked above")) {
        return host::run(
            cli.command.expect("command checked above"),
            current_dir,
            executor,
            executor,
            stdout,
        );
    }
    let runtime = bootstrap::run(paths, stderr)?;
    dispatch::run(
        cli,
        dispatch::Environment {
            paths,
            current_dir,
            runtime: &runtime,
            executor,
            capture_executor: executor,
            session,
        },
        stdout,
        stderr,
    )
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod phase5_tests;

#[cfg(test)]
mod phase6_tests;

#[cfg(test)]
mod phase7_tests;

#[cfg(test)]
mod phase8_tests;
