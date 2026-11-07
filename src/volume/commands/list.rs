use std::{ffi::OsString, path::Path};

use crate::{
    docker::DockerInvocation,
    error::AppError,
    outcome::AppOutcome,
    output::{Output, Table},
    process::ProcessOutputExecutor,
};

use super::super::parse;

pub(crate) const FORMAT: &str = "{{.Driver}}\t{{.Name}}";

pub fn run(
    filters: Vec<String>,
    current_dir: &Path,
    executor: &dyn ProcessOutputExecutor,
    output: &mut Output<'_>,
) -> Result<AppOutcome, AppError> {
    let mut args = vec![OsString::from("volume"), OsString::from("ls")];
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
    let mut table = Table::new(["DRIVER", "NAME"]);
    for row in parse::volumes(&captured.stdout)? {
        table.push([row.driver, row.name]);
    }
    table.write(output)?;
    Ok(AppOutcome::Success)
}
