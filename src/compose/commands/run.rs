use std::ffi::OsString;

use crate::{
    context::ResolvedContext, error::AppError, outcome::AppOutcome, process::ProcessExecutor,
};

use super::execute;

pub fn run(
    context: &ResolvedContext,
    service: OsString,
    command: Vec<OsString>,
    executor: &dyn ProcessExecutor,
) -> Result<AppOutcome, AppError> {
    let mut args = vec![OsString::from("run"), OsString::from("--rm"), service];
    args.extend(command);
    execute::once(context, args, executor)
}
