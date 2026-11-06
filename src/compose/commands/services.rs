use crate::{
    context::ResolvedContext, error::AppError, outcome::AppOutcome, process::ProcessExecutor,
};

use super::execute;

pub fn run(
    context: &ResolvedContext,
    executor: &dyn ProcessExecutor,
) -> Result<AppOutcome, AppError> {
    execute::once(context, ["config", "--services"], executor)
}
