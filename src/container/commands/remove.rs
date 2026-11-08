use std::{ffi::OsString, path::Path};

use crate::{
    docker::DockerInvocation, error::AppError, outcome::AppOutcome, process::ProcessExecutor,
};

pub fn run(
    containers: Vec<OsString>,
    current_dir: &Path,
    executor: &dyn ProcessExecutor,
) -> Result<AppOutcome, AppError> {
    let args = [
        vec![OsString::from("container"), OsString::from("rm")],
        containers,
    ]
    .concat();
    let status = executor.execute(&DockerInvocation::new(current_dir).process(args))?;
    Ok(AppOutcome::Child(status))
}
