use std::ffi::{OsStr, OsString};

use crate::{
    context::ResolvedContext,
    process::{ProcessError, ProcessExecutor, StdioPolicy},
};

use super::ComposeInvocation;

/// Supported interactive shells, in probe preference order.
pub const SUPPORTED_SHELLS: [&str; 4] = ["bash", "sh", "zsh", "ash"];

pub fn select(
    context: &ResolvedContext,
    service: &OsStr,
    executor: &dyn ProcessExecutor,
) -> Result<Option<OsString>, ProcessError> {
    for candidate in SUPPORTED_SHELLS {
        let spec = ComposeInvocation::new(context)
            .process([
                OsString::from("exec"),
                OsString::from("-T"),
                service.to_os_string(),
                OsString::from(candidate),
                OsString::from("-c"),
                OsString::from("exit 0"),
            ])
            .with_stdio(StdioPolicy::Null);
        if executor.execute(&spec)?.success() {
            return Ok(Some(OsString::from(candidate)));
        }
    }
    Ok(None)
}
