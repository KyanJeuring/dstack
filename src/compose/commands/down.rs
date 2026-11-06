use std::ffi::OsString;

use crate::{
    context::ResolvedContext, error::AppError, outcome::AppOutcome, process::ProcessExecutor,
};

use super::execute;

pub fn run(
    context: &ResolvedContext,
    volumes: bool,
    executor: &dyn ProcessExecutor,
) -> Result<AppOutcome, AppError> {
    let mut args = vec![OsString::from("down")];
    if volumes {
        args.push(OsString::from("--volumes"));
    }
    execute::once(context, args, executor)
}
