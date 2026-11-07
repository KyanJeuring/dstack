use std::{collections::VecDeque, ffi::OsString, io, sync::Mutex};

use super::{
    CapturedProcess, CapturedProcessSpec, ProcessError, ProcessExecutor, ProcessOutputExecutor,
    ProcessSpec, ProcessStatus,
};

#[derive(Debug)]
enum Response {
    Status(ProcessStatus),
    SpawnError(io::ErrorKind, String),
}

#[derive(Debug, Default)]
pub(crate) struct RecordingExecutor {
    requests: Mutex<Vec<ProcessSpec>>,
    responses: Mutex<VecDeque<Response>>,
    capture_requests: Mutex<Vec<CapturedProcessSpec>>,
    capture_responses: Mutex<VecDeque<CaptureResponse>>,
}

#[derive(Debug)]
enum CaptureResponse {
    Output(CapturedProcess),
    SpawnError(io::ErrorKind, String),
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

    pub(crate) fn push_capture(&self, stdout: impl Into<Vec<u8>>, code: i32) {
        self.capture_responses
            .lock()
            .unwrap()
            .push_back(CaptureResponse::Output(CapturedProcess {
                status: ProcessStatus::from_code(code),
                stdout: stdout.into(),
            }));
    }

    pub(crate) fn push_capture_spawn_error(&self, kind: io::ErrorKind, message: impl Into<String>) {
        self.capture_responses
            .lock()
            .unwrap()
            .push_back(CaptureResponse::SpawnError(kind, message.into()));
    }

    pub(crate) fn capture_requests(&self) -> Vec<CapturedProcessSpec> {
        self.capture_requests.lock().unwrap().clone()
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

impl ProcessOutputExecutor for RecordingExecutor {
    fn capture(&self, spec: &CapturedProcessSpec) -> Result<CapturedProcess, ProcessError> {
        self.capture_requests.lock().unwrap().push(spec.clone());
        match self.capture_responses.lock().unwrap().pop_front() {
            Some(CaptureResponse::Output(output)) => Ok(output),
            Some(CaptureResponse::SpawnError(kind, message)) => Err(ProcessError::Spawn {
                program: spec.process.program.clone(),
                source: io::Error::new(kind, message),
            }),
            None => Ok(CapturedProcess {
                status: ProcessStatus::from_code(0),
                stdout: Vec::new(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{io, path::PathBuf};

    use super::RecordingExecutor;
    use crate::process::{
        CapturePolicy, CapturedProcessSpec, ProcessError, ProcessExecutor, ProcessOutputExecutor,
        ProcessSpec,
    };

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

    #[test]
    fn capture_records_exact_spec_and_returns_stdout_and_status() {
        let executor = RecordingExecutor::default();
        executor.push_capture(b"one\ntwo\n".to_vec(), 17);
        let spec = CapturedProcessSpec::stdout(
            ProcessSpec::new("docker", PathBuf::from("host cwd")).with_args(["container", "ls"]),
        );

        let output = executor.capture(&spec).unwrap();

        assert_eq!(output.status.code(), Some(17));
        assert_eq!(output.stdout, b"one\ntwo\n");
        assert_eq!(executor.capture_requests(), [spec]);
        assert_eq!(
            executor.capture_requests()[0].policy,
            CapturePolicy::StdoutWithInheritedStderr
        );
        assert!(executor.requests().is_empty());
    }

    #[test]
    fn capture_spawn_error_is_distinct_and_does_not_affect_execute_queue() {
        let executor = RecordingExecutor::default();
        executor.push_capture_spawn_error(io::ErrorKind::PermissionDenied, "capture denied");
        executor.push_status(23);
        let capture =
            CapturedProcessSpec::stdout(ProcessSpec::new("docker", PathBuf::from("host cwd")));

        let error = executor.capture(&capture).unwrap_err();
        assert!(matches!(error, ProcessError::Spawn { .. }));
        assert!(error.to_string().contains("capture denied"));

        let stream = ProcessSpec::new("docker", PathBuf::from("project"));
        assert_eq!(executor.execute(&stream).unwrap().code(), Some(23));
        assert_eq!(executor.requests(), [stream]);
    }
}
