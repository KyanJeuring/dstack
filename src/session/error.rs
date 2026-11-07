use std::{fmt, path::PathBuf};

#[derive(Debug)]
pub enum SessionError {
    NonUtf8Environment,
    Malformed(serde_json::Error),
    Serialize(serde_json::Error),
    UnsupportedSchemaVersion {
        found: u32,
        supported: u32,
    },
    InvalidState(String),
    NonUtf8Path {
        label: &'static str,
        path: PathBuf,
    },
    StaleProject(PathBuf),
    StaleComposeFile(PathBuf),
    ResolveProject {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl SessionError {
    fn recovery(formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("; run 'dstack unset' or select it again with 'dstack use NAME'")
    }
}

impl fmt::Display for SessionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonUtf8Environment => {
                formatter.write_str("active terminal session is not valid UTF-8")?;
                Self::recovery(formatter)
            }
            Self::Malformed(source) => {
                write!(formatter, "active terminal session is invalid: {source}")?;
                Self::recovery(formatter)
            }
            Self::Serialize(source) => {
                write!(formatter, "could not serialize terminal session: {source}")
            }
            Self::UnsupportedSchemaVersion { found, supported } => {
                write!(
                    formatter,
                    "active terminal session uses unsupported schema version {found}; this DStack supports version {supported}"
                )?;
                Self::recovery(formatter)
            }
            Self::InvalidState(reason) => {
                write!(formatter, "active terminal session is invalid: {reason}")?;
                Self::recovery(formatter)
            }
            Self::NonUtf8Path { label, path } => write!(
                formatter,
                "cannot activate terminal session because {label} '{}' is not valid UTF-8",
                path.display()
            ),
            Self::StaleProject(path) => {
                write!(
                    formatter,
                    "active terminal session project '{}' is missing or is not a directory",
                    path.display()
                )?;
                Self::recovery(formatter)
            }
            Self::StaleComposeFile(path) => {
                write!(
                    formatter,
                    "active terminal session Compose file '{}' is missing or is not a regular file",
                    path.display()
                )?;
                Self::recovery(formatter)
            }
            Self::ResolveProject { path, source } => {
                write!(
                    formatter,
                    "could not validate active terminal session project '{}': {source}",
                    path.display()
                )?;
                Self::recovery(formatter)
            }
        }
    }
}

impl std::error::Error for SessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Malformed(source) | Self::Serialize(source) => Some(source),
            Self::ResolveProject { source, .. } => Some(source),
            _ => None,
        }
    }
}
