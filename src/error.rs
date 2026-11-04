use std::{fmt, process::ExitCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppError {
    NoCommand,
}

impl AppError {
    pub fn exit_code(self) -> ExitCode {
        match self {
            Self::NoCommand => ExitCode::from(2),
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCommand => formatter.write_str(
                "no DStack v2 commands are available in Phase 1; use --help or --version",
            ),
        }
    }
}

impl std::error::Error for AppError {}
