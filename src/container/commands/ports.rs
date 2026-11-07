use std::{ffi::OsString, path::Path};

use crate::{
    docker::DockerInvocation,
    error::AppError,
    outcome::AppOutcome,
    output::{Output, Table},
    process::{ProcessExecutor, ProcessOutputExecutor},
};

use super::super::parse;

const FORMAT: &str = "{{.Names}}\t{{.Ports}}";

pub fn run(
    container: Option<OsString>,
    private_port: Option<OsString>,
    current_dir: &Path,
    stream_executor: &dyn ProcessExecutor,
    capture_executor: &dyn ProcessOutputExecutor,
    output: &mut Output<'_>,
) -> Result<AppOutcome, AppError> {
    if let Some(container) = container {
        let mut args = vec![
            OsString::from("container"),
            OsString::from("port"),
            container,
        ];
        if let Some(private_port) = private_port {
            args.push(private_port);
        }
        let status = stream_executor.execute(&DockerInvocation::new(current_dir).process(args))?;
        return Ok(AppOutcome::Child(status));
    }

    let args = ["container", "ls", "--format", FORMAT];
    let captured = capture_executor.capture(&DockerInvocation::new(current_dir).capture(args))?;
    if !captured.status.success() {
        return Ok(AppOutcome::Child(captured.status));
    }
    let mut table = Table::new(["NAME", "PORTS"]);
    for row in parse::ports(&captured.stdout)? {
        table.push([row.name, row.ports]);
    }
    table.write(output)?;
    Ok(AppOutcome::Success)
}
