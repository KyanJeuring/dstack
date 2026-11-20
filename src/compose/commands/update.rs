use std::ffi::OsString;

use crate::{
    context::ResolvedContext, error::AppError, outcome::AppOutcome, process::ProcessExecutor,
};

use super::execute;

pub fn run(
    context: &ResolvedContext,
    services: Vec<OsString>,
    remove_orphans: bool,
    executor: &dyn ProcessExecutor,
) -> Result<AppOutcome, AppError> {
    let status = execute::step(
        context,
        execute::services("pull", services.clone()),
        executor,
    )?;
    if !status.success() {
        return Ok(AppOutcome::Child(status));
    }

    let mut up = vec![OsString::from("up"), OsString::from("-d")];
    if remove_orphans {
        up.push(OsString::from("--remove-orphans"));
    }
    up.extend(services);
    execute::once(context, up, executor)
}
