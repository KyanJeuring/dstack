use std::process::{Command, Stdio};

use super::{ProcessError, ProcessExecutor, ProcessSpec, ProcessStatus, StdioPolicy};

#[derive(Debug, Default, Clone, Copy)]
pub struct NativeProcessExecutor;

impl ProcessExecutor for NativeProcessExecutor {
    fn execute(&self, spec: &ProcessSpec) -> Result<ProcessStatus, ProcessError> {
        let mut command = Command::new(&spec.program);
        command.args(&spec.args).current_dir(&spec.current_dir);
        match spec.stdio {
            StdioPolicy::Inherit => {
                command
                    .stdin(Stdio::inherit())
                    .stdout(Stdio::inherit())
                    .stderr(Stdio::inherit());
            }
            StdioPolicy::Null => {
                command
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null());
            }
        }

        let mut child = command.spawn().map_err(|source| ProcessError::Spawn {
            program: spec.program.clone(),
            source,
        })?;
        let status = child.wait().map_err(|source| ProcessError::Wait {
            program: spec.program.clone(),
            source,
        })?;
        Ok(status
            .code()
            .map_or_else(ProcessStatus::without_code, ProcessStatus::from_code))
    }
}

#[cfg(test)]
mod tests {
    use std::{error::Error, path::PathBuf};

    use super::NativeProcessExecutor;
    use crate::process::{ProcessError, ProcessExecutor, ProcessSpec};

    #[test]
    fn missing_executable_maps_to_spawn_error_with_os_context() {
        let spec = ProcessSpec::new(
            "dstack-phase4-executable-that-does-not-exist",
            PathBuf::from("."),
        );
        let error = NativeProcessExecutor.execute(&spec).unwrap_err();

        assert!(matches!(error, ProcessError::Spawn { .. }));
        assert!(error.source().is_some());
        assert!(error.to_string().contains("does-not-exist"));
    }
}
