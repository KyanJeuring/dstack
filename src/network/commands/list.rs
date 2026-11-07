use std::{ffi::OsString, path::Path};

use crate::{
    docker::DockerInvocation,
    error::AppError,
    outcome::AppOutcome,
    output::{Output, Table},
    process::ProcessOutputExecutor,
};

use super::super::parse;

pub(crate) const FORMAT: &str = "{{.ID}}\t{{.Name}}\t{{.Driver}}\t{{.Scope}}";

pub fn run(
    filters: Vec<String>,
    current_dir: &Path,
    executor: &dyn ProcessOutputExecutor,
    output: &mut Output<'_>,
) -> Result<AppOutcome, AppError> {
    let mut args = vec![OsString::from("network"), OsString::from("ls")];
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
    let mut table = Table::new(["NETWORK ID", "NAME", "DRIVER", "SCOPE"]);
    for row in parse::networks(&captured.stdout)? {
        table.push([row.id, row.name, row.driver, row.scope]);
    }
    table.write(output)?;
    Ok(AppOutcome::Success)
}
