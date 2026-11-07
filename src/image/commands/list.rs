use std::{ffi::OsString, path::Path};

use crate::{
    docker::DockerInvocation,
    error::AppError,
    outcome::AppOutcome,
    output::{Output, Table},
    process::ProcessOutputExecutor,
};

use super::super::parse;

pub(crate) const FORMAT: &str = "{{.Repository}}\t{{.Tag}}\t{{.ID}}\t{{.Size}}";

pub fn run(
    all: bool,
    filters: Vec<String>,
    current_dir: &Path,
    executor: &dyn ProcessOutputExecutor,
    output: &mut Output<'_>,
) -> Result<AppOutcome, AppError> {
    let mut args = vec![OsString::from("image"), OsString::from("ls")];
    if all {
        args.push(OsString::from("--all"));
    }
    for filter in filters {
        args.push(OsString::from("--filter"));
        args.push(OsString::from(filter));
    }
    args.push(OsString::from("--format"));
    args.push(OsString::from(FORMAT));

    let captured = executor.capture(&DockerInvocation::new(current_dir).capture(args))?;
    if !captured.status.success() {
        return Ok(AppOutcome::Child(captured.status));
    }
    let mut table = Table::new(["REPOSITORY", "TAG", "IMAGE ID", "SIZE"]);
    for row in parse::images(&captured.stdout)? {
        table.push([row.repository, row.tag, row.id, row.size]);
    }
    table.write(output)?;
    Ok(AppOutcome::Success)
}
