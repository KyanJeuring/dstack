use std::{ffi::OsString, fmt, io};

#[derive(Debug)]
pub enum ProcessError {
    Spawn {
        program: OsString,
        source: io::Error,
    },
    Wait {
        program: OsString,
        source: io::Error,
    },
}

impl fmt::Display for ProcessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn { program, source } => write!(
                formatter,
                "could not start process '{}': {source}",
                program.to_string_lossy()
            ),
            Self::Wait { program, source } => write!(
                formatter,
                "could not wait for process '{}': {source}",
                program.to_string_lossy()
            ),
        }
    }
}

impl std::error::Error for ProcessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn { source, .. } | Self::Wait { source, .. } => Some(source),
        }
    }
}
