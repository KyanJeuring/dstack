use std::io::Write;

use crate::{
    Cli,
    cli::{Command, ShellCommand},
    error::AppError,
    outcome::AppOutcome,
    output::{Output, TextStyle},
    shell::{ShellMutation, init_script, render_mutation},
};

pub(super) fn run(
    cli: &Cli,
    stdout: &mut Output<'_>,
    stderr: &mut Output<'_>,
) -> Result<Option<AppOutcome>, AppError> {
    let command = cli.command.as_ref().ok_or(AppError::NoCommand)?;
    match command {
        Command::Use(_) => Err(AppError::ShellIntegrationRequired("use")),
        Command::Unset => Err(AppError::ShellIntegrationRequired("unset")),
        Command::Shell(arguments) => {
            if cli.stack.is_some() || !cli.files.is_empty() {
                return Err(AppError::InvalidOptions(
                    "shell integration commands do not accept root --stack or --file options"
                        .to_owned(),
                ));
            }
            match &arguments.command {
                ShellCommand::Init { .. } => {
                    writeln!(stdout, "{}", init_script(arguments.command.shell()))?;
                    Ok(Some(AppOutcome::Success))
                }
                ShellCommand::Clear { .. } => {
                    writeln!(
                        stdout,
                        "{}",
                        render_mutation(arguments.command.shell(), ShellMutation::UnsetSession,)
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
