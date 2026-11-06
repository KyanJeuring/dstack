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
    let mut down = vec![OsString::from("down")];
    if volumes {
        down.push(OsString::from("--volumes"));
    }
    let status = execute::step(context, down, executor)?;
    if !status.success() {
        return Ok(AppOutcome::Child(status));
    }
    execute::once(context, ["up", "-d", "--build"], executor)
}
