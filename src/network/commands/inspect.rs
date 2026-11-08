use std::{ffi::OsString, path::Path};

use crate::{
    docker::{DockerInvocation, InspectResource, inspect_all, write_inspect_json},
    error::AppError,
    outcome::AppOutcome,
    output::Output,
    process::{ProcessExecutor, ProcessOutputExecutor},
};

pub fn run(
    all: bool,
    names: Vec<OsString>,
    current_dir: &Path,
    stream_executor: &dyn ProcessExecutor,
    capture_executor: &dyn ProcessOutputExecutor,
    output: &mut Output<'_>,
) -> Result<AppOutcome, AppError> {
    if all {
        return write_inspect_json(
            inspect_all(InspectResource::Network, current_dir, capture_executor)?,
            output,
        );
    }
    let args = [
        vec![OsString::from("network"), OsString::from("inspect")],
        names,
    ]
    .concat();
    let status = stream_executor.execute(&DockerInvocation::new(current_dir).process(args))?;
    Ok(AppOutcome::Child(status))
}
