mod inspector;
mod model;
mod parse;

use std::{ffi::OsString, io::Write};

pub(crate) use inspector::ComposeStateInspector;
pub(crate) use model::{DesiredState, DesiredStateAssessment};

use crate::{
    context::ResolvedContext,
    error::AppError,
    output::{Output, TextStyle},
};

pub(crate) fn write_already_message(
    context: &ResolvedContext,
    services: &[OsString],
    desired: DesiredState,
    stderr: &mut Output<'_>,
) -> Result<(), AppError> {
    stderr.write_styled(TextStyle::Info, "[INFO]")?;
    let state = desired.description();
    match services {
        [] => writeln!(
            stderr,
            " Stack '{}' is already {state}.",
            context.stack.name
        )?,
        [service] => writeln!(
            stderr,
            " Service '{}' is already {state}.",
            service.to_string_lossy()
        )?,
        services => writeln!(
            stderr,
            " Requested services are already {state}: {}",
            services
                .iter()
                .map(|service| service.to_string_lossy())
                .collect::<Vec<_>>()
                .join(", ")
        )?,
    }
    Ok(())
}
