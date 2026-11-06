use std::{fmt, io, path::PathBuf};

use super::CONFIG_DIR_ENV;

#[derive(Debug)]
pub enum ConfigError {
    EmptyOverride,
    HomeDirectoryUnavailable,
    UnsupportedSchemaVersion {
        found: u32,
        supported: u32,
    },
    MalformedConfig {
        path: PathBuf,
        source: toml::de::Error,
    },
    Serialize(toml::ser::Error),
    InvalidValue(String),
    MissingParent(PathBuf),
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyOverride => write!(
                formatter,
                "{CONFIG_DIR_ENV} is set but does not name a directory"
            ),
            Self::HomeDirectoryUnavailable => {
                formatter.write_str("could not determine the current user's home directory")
            }
            Self::UnsupportedSchemaVersion { found, supported } => write!(
                formatter,
                "configuration schema version {found} is not supported (this DStack supports version {supported})"
            ),
            Self::MalformedConfig { path, source } => write!(
                formatter,
                "could not parse configuration '{}': {source}",
                path.display()
            ),
            Self::Serialize(source) => write!(
                formatter,
                "could not serialize default configuration: {source}"
            ),
            Self::InvalidValue(reason) => {
                write!(formatter, "invalid DStack configuration: {reason}")
            }
            Self::MissingParent(path) => write!(
                formatter,
                "configuration path '{}' has no usable parent directory",
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

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::MalformedConfig { source, .. } => Some(source),
            Self::Serialize(source) => Some(source),
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
