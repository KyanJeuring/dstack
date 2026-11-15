use std::{fmt, io, path::PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendFailureKind {
    Network,
    Checksum,
    Archive,
    Replacement,
    Release,
}

impl fmt::Display for BackendFailureKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Network => "release network request",
            Self::Checksum => "release checksum verification",
            Self::Archive => "release archive extraction",
            Self::Replacement => "executable replacement",
            Self::Release => "release metadata",
        })
    }
}

#[derive(Debug)]
pub enum UpdateError {
    UnsupportedPlatform {
        os: String,
        arch: String,
    },
    InvalidRunningVersion(String),
    CurrentExecutable(io::Error),
    OwnershipInspection {
        path: PathBuf,
        source: io::Error,
    },
    SystemPackageManaged {
        executable: PathBuf,
    },
    CargoManaged {
        executable: PathBuf,
    },
    InstallPathNotWritable {
        path: PathBuf,
    },
    Output(io::Error),
    Backend {
        kind: BackendFailureKind,
        message: String,
    },
}

impl UpdateError {
    pub(crate) fn backend(kind: BackendFailureKind, error: impl fmt::Display) -> Self {
        Self::Backend {
            kind,
            message: error.to_string(),
        }
    }
}

impl fmt::Display for UpdateError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform { os, arch } => write!(
                formatter,
                "self-update is not supported on platform {os}/{arch}; use a supported standalone release or the installation method that owns this binary"
            ),
            Self::InvalidRunningVersion(version) => write!(
                formatter,
                "running DStack version '{version}' is not a stable compatible v2 version"
            ),
            Self::CurrentExecutable(error) => {
                write!(formatter, "could not determine the running DStack executable: {error}")
            }
            Self::OwnershipInspection { path, source } => write!(
                formatter,
                "could not inspect DStack installation ownership marker '{}': {source}",
                path.display()
            ),
            Self::SystemPackageManaged { .. } => formatter.write_str(
                "this DStack installation is managed by a system package. Update it using the same package manager that installed it",
            ),
            Self::CargoManaged { .. } => formatter.write_str(
                "this DStack installation is managed by Cargo. Update it with 'cargo install dstack-compose --locked --force'",
            ),
            Self::InstallPathNotWritable { path } => write!(
                formatter,
                "DStack cannot replace '{}' because its installation location is not writable. Update it using its installation method or move it to a user-writable location; DStack will not elevate privileges",
                path.display()
            ),
            Self::Output(error) => write!(formatter, "could not write self-update output: {error}"),
            Self::Backend { kind, message } => {
                write!(formatter, "{kind} failed: {message}")
            }
        }
    }
}

impl std::error::Error for UpdateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CurrentExecutable(error) => Some(error),
            Self::OwnershipInspection { source, .. } => Some(source),
            Self::Output(error) => Some(error),
            Self::UnsupportedPlatform { .. }
            | Self::InvalidRunningVersion(_)
            | Self::SystemPackageManaged { .. }
            | Self::CargoManaged { .. }
            | Self::InstallPathNotWritable { .. }
            | Self::Backend { .. } => None,
        }
    }
}
