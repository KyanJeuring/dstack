use std::ffi::OsString;

use crate::{
    context::ResolvedContext, error::AppError, outcome::AppOutcome, process::ProcessExecutor,
};

use super::execute;

pub fn run(
    context: &ResolvedContext,
    services: Vec<OsString>,
    no_cache: bool,
    executor: &dyn ProcessExecutor,
) -> Result<AppOutcome, AppError> {
    let mut build = vec![OsString::from("build")];
    if no_cache {
        build.push(OsString::from("--no-cache"));
    }
    build.extend(services.iter().cloned());
    let status = execute::step(context, build, executor)?;
    if !status.success() {
        return Ok(AppOutcome::Child(status));
    }

    let mut up = vec![OsString::from("up"), OsString::from("-d")];
    up.extend(services);
    execute::once(context, up, executor)
}
