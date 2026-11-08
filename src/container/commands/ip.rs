use std::{ffi::OsString, path::Path};

use crate::{
    docker::{DockerInvocation, InspectAllResult, InspectResource, inspect_all},
    error::AppError,
    outcome::AppOutcome,
    output::{Output, Table},
    process::ProcessOutputExecutor,
};

use super::super::parse;

pub(crate) const FORMAT: &str = "{{range $network, $config := .NetworkSettings.Networks}}{{$network}}\t{{$config.IPAddress}}\n{{end}}";

pub fn run(
    container: Option<OsString>,
    all: bool,
    current_dir: &Path,
    executor: &dyn ProcessOutputExecutor,
    output: &mut Output<'_>,
) -> Result<AppOutcome, AppError> {
    if all {
        return run_all(current_dir, executor, output);
    }
    let Some(container) = container else {
        return Err(AppError::InvalidOptions(
            "container ip requires one CONTAINER or --all".to_owned(),
        ));
    };
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

fn run_all(
    current_dir: &Path,
    executor: &dyn ProcessOutputExecutor,
    output: &mut Output<'_>,
) -> Result<AppOutcome, AppError> {
    let inspected = match inspect_all(InspectResource::Container, current_dir, executor)? {
        InspectAllResult::Complete(inspected) => inspected,
        InspectAllResult::Child(status) => return Ok(AppOutcome::Child(status)),
    };
    let mut table = Table::new(["CONTAINER", "NETWORK", "IP"]);
    for row in parse::all_network_addresses(inspected.objects, &inspected.identifiers)? {
        table.push([row.container, row.network, row.ip]);
    }
    table.write(output)?;
    Ok(AppOutcome::Success)
}
