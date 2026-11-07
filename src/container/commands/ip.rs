use std::{ffi::OsString, path::Path};

use crate::{
    docker::DockerInvocation,
    error::AppError,
    outcome::AppOutcome,
    output::{Output, Table},
    process::ProcessOutputExecutor,
};

use super::super::parse;

pub(crate) const FORMAT: &str = "{{range $network, $config := .NetworkSettings.Networks}}{{$network}}\t{{$config.IPAddress}}\n{{end}}";

pub fn run(
    container: OsString,
    current_dir: &Path,
    executor: &dyn ProcessOutputExecutor,
    output: &mut Output<'_>,
) -> Result<AppOutcome, AppError> {
    let args = [
        OsString::from("container"),
        OsString::from("inspect"),
        OsString::from("--format"),
        OsString::from(FORMAT),
        container,
    ];
    let captured = executor.capture(&DockerInvocation::new(current_dir).capture(args))?;
    if !captured.status.success() {
        return Ok(AppOutcome::Child(captured.status));
    }
    let mut table = Table::new(["NETWORK", "IP"]);
    for row in parse::network_addresses(&captured.stdout)? {
        table.push([row.network, row.ip]);
    }
    table.write(output)?;
    Ok(AppOutcome::Success)
}
