use std::{ffi::OsString, path::Path};

use crate::{
    docker::DockerInvocation, error::AppError, outcome::AppOutcome, process::ProcessExecutor,
};

pub fn run(
    names: Vec<OsString>,
    current_dir: &Path,
    executor: &dyn ProcessExecutor,
) -> Result<AppOutcome, AppError> {
    let args = [
        vec![OsString::from("network"), OsString::from("inspect")],
        names,
    ]
    .concat();
    let status = executor.execute(&DockerInvocation::new(current_dir).process(args))?;
    Ok(AppOutcome::Child(status))
}
