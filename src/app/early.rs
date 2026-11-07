use std::io::Write;

use crate::{
    Cli,
    cli::{Command, ShellCommand},
    error::AppError,
    outcome::AppOutcome,
    output::{Output, TextStyle},
    shell::{
        ManagementEnvironment, ShellMutation, init_script, integration_guidance, render_mutation,
        run_install, run_status, run_uninstall,
    },
};

pub(super) fn run(
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
                    writeln!(stdout, "{}", init_script(*shell))?;
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
        _ => Ok(None),
    }
}
