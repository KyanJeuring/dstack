use std::ffi::OsString;

use crate::{
    compose::shell, context::ResolvedContext, error::AppError, outcome::AppOutcome,
    process::ProcessExecutor,
};

use super::execute;

pub fn run(
    context: &ResolvedContext,
    service: OsString,
    command: Vec<OsString>,
    executor: &dyn ProcessExecutor,
) -> Result<AppOutcome, AppError> {
    let mut args = vec![OsString::from("exec"), service.clone()];
    if command.is_empty() {
        let selected = shell::select(context, &service, executor)?.ok_or_else(|| {
            AppError::InteractiveShellUnavailable {
                service,
                tried: shell::SUPPORTED_SHELLS.join(", "),
            }
        })?;
        args.push(selected);
    } else {
        args.extend(command);
    }
    execute::once(context, args, executor)
}
