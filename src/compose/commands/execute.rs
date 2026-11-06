use std::ffi::OsString;

use crate::{
    compose::ComposeInvocation,
    context::ResolvedContext,
    error::AppError,
    outcome::AppOutcome,
    process::{ProcessExecutor, ProcessStatus},
};

pub(super) fn once(
    context: &ResolvedContext,
    args: impl IntoIterator<Item = impl Into<OsString>>,
    executor: &dyn ProcessExecutor,
) -> Result<AppOutcome, AppError> {
    step(context, args, executor).map(AppOutcome::Child)
}

pub(super) fn step(
    context: &ResolvedContext,
    args: impl IntoIterator<Item = impl Into<OsString>>,
    executor: &dyn ProcessExecutor,
) -> Result<ProcessStatus, AppError> {
    let spec = ComposeInvocation::new(context).process(args);
    executor.execute(&spec).map_err(AppError::from)
}

pub(super) fn services(operation: impl Into<OsString>, services: Vec<OsString>) -> Vec<OsString> {
    std::iter::once(operation.into()).chain(services).collect()
}
