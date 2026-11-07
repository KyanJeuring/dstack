use std::{ffi::OsString, path::Path};

use crate::{
    docker::DockerInvocation, error::AppError, outcome::AppOutcome, process::ProcessExecutor,
};

pub fn run(
    all: bool,
    no_stream: bool,
    containers: Vec<OsString>,
    current_dir: &Path,
    executor: &dyn ProcessExecutor,
) -> Result<AppOutcome, AppError> {
    let mut args = vec![OsString::from("container"), OsString::from("stats")];
    if all {
        args.push(OsString::from("--all"));
    }
    if no_stream {
        args.push(OsString::from("--no-stream"));
    }
    args.extend(containers);
    let status = executor.execute(&DockerInvocation::new(current_dir).process(args))?;
    Ok(AppOutcome::Child(status))
}
