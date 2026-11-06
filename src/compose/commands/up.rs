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
    let mut args = ["up", "-d", "--build"]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    if remove_orphans {
        args.push(OsString::from("--remove-orphans"));
    }
    args.extend(services);
    execute::once(context, args, executor)
}
