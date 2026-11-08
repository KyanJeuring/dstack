use std::ffi::OsString;

use crate::{
    compose::state::{
        ComposeStateInspector, DesiredState, DesiredStateAssessment, write_already_message,
    },
    context::ResolvedContext,
    error::AppError,
    outcome::AppOutcome,
    output::Output,
    process::{ProcessExecutor, ProcessOutputExecutor},
};

use super::execute;

pub fn run(
    context: &ResolvedContext,
    services: Vec<OsString>,
    stream_executor: &dyn ProcessExecutor,
    capture_executor: &dyn ProcessOutputExecutor,
    stderr: &mut Output<'_>,
) -> Result<AppOutcome, AppError> {
    if ComposeStateInspector::new(capture_executor).assess(
        context,
        &services,
        DesiredState::Stopped,
    ) == DesiredStateAssessment::AlreadySatisfied
    {
        write_already_message(context, &services, DesiredState::Stopped, stderr)?;
        return Ok(AppOutcome::Success);
    }
    execute::once(
        context,
        execute::services("stop", services),
        stream_executor,
    )
}
