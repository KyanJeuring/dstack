use super::{CapturedProcess, CapturedProcessSpec, ProcessError, ProcessSpec, ProcessStatus};

/// Executes one already-constructed process request.
pub trait ProcessExecutor {
    fn execute(&self, spec: &ProcessSpec) -> Result<ProcessStatus, ProcessError>;
}

/// Captures bounded process metadata without changing the streaming executor contract.
pub trait ProcessOutputExecutor {
    fn capture(&self, spec: &CapturedProcessSpec) -> Result<CapturedProcess, ProcessError>;
}
