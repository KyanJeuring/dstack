use std::ffi::OsString;

use crate::{
    context::ResolvedContext, error::AppError, outcome::AppOutcome, process::ProcessExecutor,
};

use super::execute;

pub fn run(
    context: &ResolvedContext,
    services: Vec<OsString>,
    follow: bool,
    tail: u64,
    executor: &dyn ProcessExecutor,
) -> Result<AppOutcome, AppError> {
    let mut args = vec![OsString::from("logs")];
    if follow {
        args.push(OsString::from("--follow"));
    }
    args.push(OsString::from("--tail"));
    args.push(OsString::from(tail.to_string()));
    args.extend(services);
    execute::once(context, args, executor)
}
