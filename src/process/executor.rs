use super::{ProcessError, ProcessSpec, ProcessStatus};

/// Executes one already-constructed process request.
pub trait ProcessExecutor {
    fn execute(&self, spec: &ProcessSpec) -> Result<ProcessStatus, ProcessError>;
}
