use std::{fmt, io, path::PathBuf};

use super::ShellKind;

#[derive(Debug)]
pub enum ShellError {
    UnknownDetection {
        action: &'static str,
    },
    UnsupportedDetection {
        action: &'static str,
        process: String,
    },
    MissingHome(ShellKind),
    PowerShellProfileUnavailable(String),
    InvalidProfilePath(PathBuf),
    InvalidProfileEncoding(PathBuf),
    CorruptManagedBlock {
        path: PathBuf,
        reason: String,
    },
    DanglingProfileSymlink(PathBuf),
    ProfileNotFile(PathBuf),
    ReadOnlyProfile(PathBuf),
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for ShellError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownDetection { action } => write!(
                formatter,
                "could not reliably detect the current shell; no profile was changed\n\nSpecify it explicitly:\n\n  dstack shell {action} --shell bash\n  dstack shell {action} --shell zsh\n  dstack shell {action} --shell fish\n  dstack shell {action} --shell powershell"
            ),
            Self::UnsupportedDetection { action, process } => write!(
                formatter,
                "the detected shell process '{process}' is not supported; no profile was changed\n\nDStack shell integration currently supports:\n  bash\n  zsh\n  fish\n  powershell\n\nChoose an explicit target when appropriate:\n  dstack shell {action} --shell bash\n  dstack shell {action} --shell zsh\n  dstack shell {action} --shell fish\n  dstack shell {action} --shell powershell"
            ),
            Self::MissingHome(shell) => write!(
                formatter,
                "could not determine the home directory needed for the default {shell} profile; specify --profile PATH"
            ),
            Self::PowerShellProfileUnavailable(reason) => write!(
                formatter,
                "could not determine the PowerShell current-user/current-host profile ({reason}); specify --profile PATH"
            ),
            Self::InvalidProfilePath(path) => write!(
                formatter,
                "shell profile path '{}' is invalid; specify a file path",
                path.display()
            ),
            Self::InvalidProfileEncoding(path) => write!(
                formatter,
                "shell profile '{}' is not valid UTF-8; it was not changed",
                path.display()
            ),
            Self::CorruptManagedBlock { path, reason } => write!(
                formatter,
                "shell profile '{}' has invalid DStack managed markers ({reason}); it was not changed",
                path.display()
            ),
            Self::DanglingProfileSymlink(path) => write!(
                formatter,
                "shell profile symlink '{}' is dangling or cannot be resolved; it was not changed",
                path.display()
            ),
            Self::ProfileNotFile(path) => write!(
                formatter,
                "shell profile '{}' is not a regular file; it was not changed",
                path.display()
            ),
            Self::ReadOnlyProfile(path) => write!(
                formatter,
                "shell profile '{}' is read-only; it was not changed",
                path.display()
            ),
            Self::Io {
                action,
                path,
                source,
            } => write!(
                formatter,
                "could not {action} '{}': {source}",
                path.display()
            ),
        }
    }
}

impl std::error::Error for ShellError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
