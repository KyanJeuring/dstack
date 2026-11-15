use std::io::Write;

use crate::{
    Cli,
    cli::{Command, ShellCommand},
    error::AppError,
    outcome::AppOutcome,
    output::{Output, TextStyle},
    shell::{
        ManagementEnvironment, ShellMutation, completion_script, init_script, integration_guidance,
        render_mutation, run_install, run_status, run_uninstall,
    },
    update::{GitHubBackend, UpdateBackend, UpdateEnvironment, run as run_update},
};

pub(super) fn run(
    cli: &Cli,
    stdout: &mut Output<'_>,
    stderr: &mut Output<'_>,
) -> Result<Option<AppOutcome>, AppError> {
    if matches!(cli.command.as_ref(), Some(Command::SelfUpdate(_))) {
        let environment = UpdateEnvironment::from_process()?;
        return run_with_update_backend(cli, &GitHubBackend, &environment, stdout, stderr);
    }
    run_without_update(cli, stdout, stderr)
}

pub(crate) fn run_with_update_backend(
    cli: &Cli,
    backend: &dyn UpdateBackend,
    update_environment: &UpdateEnvironment,
    stdout: &mut Output<'_>,
    stderr: &mut Output<'_>,
) -> Result<Option<AppOutcome>, AppError> {
    let command = cli.command.as_ref().ok_or(AppError::NoCommand)?;
    if let Command::SelfUpdate(arguments) = command {
        if cli.stack.is_some() || !cli.files.is_empty() {
            return Err(AppError::InvalidOptions(
                "self-update does not accept root --stack or --file options".to_owned(),
            ));
        }
        run_update(arguments, backend, update_environment, stdout)?;
        return Ok(Some(AppOutcome::Success));
    }
    run_without_update(cli, stdout, stderr)
}

fn run_without_update(
    cli: &Cli,
    stdout: &mut Output<'_>,
    stderr: &mut Output<'_>,
) -> Result<Option<AppOutcome>, AppError> {
    let command = cli.command.as_ref().ok_or(AppError::NoCommand)?;
    match command {
        Command::Use(_) => Err(AppError::ShellIntegrationRequired {
            command: "use",
            guidance: integration_guidance(&ManagementEnvironment::from_process()),
        }),
        Command::Unset => Err(AppError::ShellIntegrationRequired {
            command: "unset",
            guidance: integration_guidance(&ManagementEnvironment::from_process()),
        }),
        Command::Shell(arguments) => {
            if cli.stack.is_some() || !cli.files.is_empty() {
                return Err(AppError::InvalidOptions(
                    "shell integration commands do not accept root --stack or --file options"
                        .to_owned(),
                ));
            }
            match &arguments.command {
                ShellCommand::Install(arguments) => {
                    run_install(arguments, &ManagementEnvironment::from_process(), stdout)?;
                    Ok(Some(AppOutcome::Success))
                }
                ShellCommand::Status(arguments) => {
                    run_status(arguments, &ManagementEnvironment::from_process(), stdout)?;
                    Ok(Some(AppOutcome::Success))
                }
                ShellCommand::Uninstall(arguments) => {
                    run_uninstall(arguments, &ManagementEnvironment::from_process(), stdout)?;
                    Ok(Some(AppOutcome::Success))
                }
                ShellCommand::Init { shell } => {
                    write!(stdout, "{}", init_script(*shell))?;
                    Ok(Some(AppOutcome::Success))
                }
                ShellCommand::Completions { shell } => {
                    write!(stdout, "{}", completion_script(*shell))?;
                    Ok(Some(AppOutcome::Success))
                }
                ShellCommand::Clear { shell } => {
                    writeln!(
                        stdout,
                        "{}",
                        render_mutation(*shell, ShellMutation::UnsetSession,)
                    )?;
                    stderr.write_styled(TextStyle::Success, "[OK]")?;
                    writeln!(stderr, " DStack terminal context cleared")?;
                    Ok(Some(AppOutcome::Success))
                }
                ShellCommand::Activate(_) => Ok(None),
            }
        }
        Command::SelfUpdate(_) => unreachable!("self-update was routed without its backend"),
        _ => Ok(None),
    }
}
