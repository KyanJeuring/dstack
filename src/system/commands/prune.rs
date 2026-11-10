use std::{ffi::OsString, path::Path};

use crate::{
    docker::DockerInvocation, error::AppError, outcome::AppOutcome, process::ProcessExecutor,
};

pub fn run(
    all: bool,
    volumes: bool,
    filters: Vec<String>,
    force: bool,
    current_dir: &Path,
    executor: &dyn ProcessExecutor,
) -> Result<AppOutcome, AppError> {
    let mut args = vec![OsString::from("system"), OsString::from("prune")];
    if all {
        args.push(OsString::from("--all"));
    }
    if volumes {
        args.push(OsString::from("--volumes"));
    }
    for filter in filters {
        args.push(OsString::from("--filter"));
        args.push(OsString::from(filter));
    }
    if force {
        args.push(OsString::from("--force"));
    }
    let status = executor.execute(&DockerInvocation::new(current_dir).process(args))?;
    Ok(AppOutcome::Child(status))
}
