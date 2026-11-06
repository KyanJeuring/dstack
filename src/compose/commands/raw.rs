use std::ffi::OsString;

use crate::{
    compose::ComposeInvocation, context::ResolvedContext, error::AppError, outcome::AppOutcome,
    process::ProcessExecutor,
};

pub fn run(
    context: &ResolvedContext,
    args: Vec<OsString>,
    executor: &dyn ProcessExecutor,
) -> Result<AppOutcome, AppError> {
    let spec = ComposeInvocation::new(context).process(args);
    let status = executor.execute(&spec)?;
    Ok(AppOutcome::Child(status))
}
