use std::{fmt, io, path::PathBuf};

#[derive(Debug)]
pub enum RegistryError {
    InvalidName(String),
    InvalidProjectDirectory(PathBuf),
    InvalidComposeFile(String),
    DuplicateComposeFile(PathBuf),
    DuplicateName(String),
    UnknownProfile(String),
    UnsupportedSchemaVersion {
        found: u32,
        supported: u32,
    },
    MalformedRegistry {
        path: PathBuf,
        source: toml::de::Error,
    },
    Serialize(toml::ser::Error),
    InvalidStoredProfile {
        name: String,
        reason: String,
    },
    NonUtf8Path {
        label: &'static str,
        path: PathBuf,
    },
    InvalidPathText {
        label: &'static str,
        path: PathBuf,
    },
    LockBusy(PathBuf),
    MissingParent(PathBuf),
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName(reason) => write!(formatter, "invalid stack profile name: {reason}"),
            Self::InvalidProjectDirectory(path) => write!(
                formatter,
                "project path '{}' is not an existing directory",
                path.display()
            ),
            Self::InvalidComposeFile(reason) => {
                write!(formatter, "invalid Compose-file value: {reason}")
            }
            Self::DuplicateComposeFile(path) => write!(
                formatter,
                "Compose-file value '{}' was provided more than once",
                path.display()
            ),
            Self::DuplicateName(name) => {
                write!(formatter, "stack profile '{name}' is already registered")
            }
            Self::UnknownProfile(name) => {
                write!(formatter, "stack profile '{name}' is not registered")
            }
            Self::UnsupportedSchemaVersion { found, supported } => write!(
                formatter,
                "registry schema version {found} is not supported (this DStack supports version {supported})"
            ),
            Self::MalformedRegistry { path, source } => write!(
                formatter,
                "could not parse registry '{}': {source}",
                path.display()
            ),
            Self::Serialize(source) => write!(formatter, "could not serialize registry: {source}"),
            Self::InvalidStoredProfile { name, reason } => {
                write!(formatter, "invalid stored profile '{name}': {reason}")
            }
            Self::NonUtf8Path { label, path } => write!(
                formatter,
                "{label} '{}' cannot be represented safely in the TOML registry because it is not valid UTF-8",
                path.display()
            ),
            Self::InvalidPathText { label, path } => write!(
                formatter,
                "{label} '{}' contains unsupported control characters",
                path.display()
            ),
            Self::LockBusy(path) => write!(
                formatter,
                "registry is locked by another DStack process ('{}')",
                path.display()
            ),
            Self::MissingParent(path) => write!(
                formatter,
                "registry path '{}' has no usable parent directory",
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

impl std::error::Error for RegistryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MalformedRegistry { source, .. } => Some(source),
            Self::Serialize(source) => Some(source),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
