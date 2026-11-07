use std::{fmt, str::Utf8Error};

#[derive(Debug)]
pub enum DockerOutputError {
    InvalidUtf8 {
        command: &'static str,
        source: Utf8Error,
    },
    UnexpectedFieldCount {
        command: &'static str,
        line: usize,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for DockerOutputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidUtf8 { command, .. } => write!(
                formatter,
                "could not interpret Docker output for '{command}': captured output was not valid UTF-8"
            ),
            Self::UnexpectedFieldCount {
                command,
                line,
                expected,
                actual,
            } => write!(
                formatter,
                "could not interpret Docker output for '{command}': machine row {line} had {actual} fields; expected {expected}"
            ),
        }
    }
}

impl std::error::Error for DockerOutputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidUtf8 { source, .. } => Some(source),
            Self::UnexpectedFieldCount { .. } => None,
        }
    }
}
