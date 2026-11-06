use std::{collections::VecDeque, ffi::OsString, io, sync::Mutex};

use super::{ProcessError, ProcessExecutor, ProcessSpec, ProcessStatus};

#[derive(Debug)]
enum Response {
    Status(ProcessStatus),
    SpawnError(io::ErrorKind, String),
}

#[derive(Debug, Default)]
pub(crate) struct RecordingExecutor {
    requests: Mutex<Vec<ProcessSpec>>,
    responses: Mutex<VecDeque<Response>>,
}

impl RecordingExecutor {
    pub(crate) fn push_status(&self, code: i32) {
        self.responses
            .lock()
            .unwrap()
            .push_back(Response::Status(ProcessStatus::from_code(code)));
    }

    pub(crate) fn push_spawn_error(&self, kind: io::ErrorKind, message: impl Into<String>) {
        self.responses
            .lock()
            .unwrap()
            .push_back(Response::SpawnError(kind, message.into()));
    }

    pub(crate) fn requests(&self) -> Vec<ProcessSpec> {
        self.requests.lock().unwrap().clone()
    }
}

impl ProcessExecutor for RecordingExecutor {
    fn execute(&self, spec: &ProcessSpec) -> Result<ProcessStatus, ProcessError> {
        self.requests.lock().unwrap().push(spec.clone());
        match self.responses.lock().unwrap().pop_front() {
            Some(Response::Status(status)) => Ok(status),
            Some(Response::SpawnError(kind, message)) => Err(ProcessError::Spawn {
                program: OsString::from(&spec.program),
                source: io::Error::new(kind, message),
            }),
            None => Ok(ProcessStatus::from_code(0)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{io, path::PathBuf};

    use super::RecordingExecutor;
    use crate::process::{ProcessError, ProcessExecutor, ProcessSpec};

    #[test]
    fn records_requests_and_returns_configured_statuses() {
        let executor = RecordingExecutor::default();
        executor.push_status(17);
        let spec = ProcessSpec::new("docker", PathBuf::from("project"));

        assert_eq!(executor.execute(&spec).unwrap().code(), Some(17));
        assert_eq!(executor.requests(), [spec]);
    }

    #[test]
    fn simulates_spawn_errors_without_starting_a_process() {
        let executor = RecordingExecutor::default();
        executor.push_spawn_error(io::ErrorKind::NotFound, "missing docker");
        let error = executor
            .execute(&ProcessSpec::new("docker", PathBuf::from("project")))
            .unwrap_err();

        assert!(matches!(error, ProcessError::Spawn { .. }));
        assert!(error.to_string().contains("missing docker"));
    }
}
