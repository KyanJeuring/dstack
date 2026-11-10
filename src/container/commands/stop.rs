use std::{ffi::OsString, io::Write, path::Path};

use crate::{
    docker::{DockerInvocation, TARGET_BATCH_SIZE, parse_identifiers},
    error::AppError,
    outcome::AppOutcome,
    output::{Output, TextStyle},
    process::{ProcessExecutor, ProcessOutputExecutor},
};

const STOP_ALL_COMMAND: &str = "dstack container stop --all";

pub fn run(
    all: bool,
    containers: Vec<OsString>,
    current_dir: &Path,
    stream_executor: &dyn ProcessExecutor,
    capture_executor: &dyn ProcessOutputExecutor,
    stderr: &mut Output<'_>,
) -> Result<AppOutcome, AppError> {
    if !all {
        let args = [
            vec![OsString::from("container"), OsString::from("stop")],
            containers,
        ]
        .concat();
        let status = stream_executor.execute(&DockerInvocation::new(current_dir).process(args))?;
        return Ok(AppOutcome::Child(status));
    }

    let invocation = DockerInvocation::new(current_dir);
    let enumeration = capture_executor.capture(&invocation.capture([
        "container",
        "ls",
        "--quiet",
        "--no-trunc",
    ]))?;
    if !enumeration.status.success() {
        return Ok(AppOutcome::Child(enumeration.status));
    }

    let identifiers = parse_identifiers(STOP_ALL_COMMAND, &enumeration.stdout)?;
    if identifiers.is_empty() {
        stderr.write_styled(TextStyle::Info, "[INFO]")?;
        writeln!(stderr, " No running containers to stop.")?;
        return Ok(AppOutcome::Success);
    }

    let mut outcome = AppOutcome::Success;
    for batch in identifiers.chunks(TARGET_BATCH_SIZE) {
        let args = [OsString::from("container"), OsString::from("stop")]
            .into_iter()
            .chain(batch.iter().map(OsString::from));
        let status = stream_executor.execute(&invocation.process(args))?;
        outcome = AppOutcome::Child(status);
        if !status.success() {
            break;
        }
    }
    Ok(outcome)
}
