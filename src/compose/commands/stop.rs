use std::ffi::OsString;

use crate::{
    context::ResolvedContext, error::AppError, outcome::AppOutcome, process::ProcessExecutor,
};

use super::execute;

pub fn run(
    context: &ResolvedContext,
    services: Vec<OsString>,
    executor: &dyn ProcessExecutor,
) -> Result<AppOutcome, AppError> {
    execute::once(context, execute::services("stop", services), executor)
}
