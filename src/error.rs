use std::{fmt, io, process::ExitCode};

use crate::{
    compose::ComposeError, config::ConfigError, discovery::DiscoveryError,
    migration::MigrationError, registry::RegistryError, resolution::ResolutionError,
};

#[derive(Debug)]
pub enum AppError {
    NoCommand,
    InvalidOptions(String),
    Config(ConfigError),
    Registry(RegistryError),
    Migration(MigrationError),
    Discovery(DiscoveryError),
    Resolution(ResolutionError),
    Compose(ComposeError),
    MigrationIncomplete { skipped: usize, conflicts: usize },
    Output(io::Error),
}

impl AppError {
    pub fn exit_code(&self) -> ExitCode {
        match self {
            Self::NoCommand | Self::InvalidOptions(_) => ExitCode::from(2),
            Self::Config(_)
            | Self::Registry(_)
            | Self::Migration(_)
            | Self::Discovery(_)
            | Self::Resolution(_)
            | Self::Compose(_)
            | Self::MigrationIncomplete { .. }
            | Self::Output(_) => ExitCode::FAILURE,
        }
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoCommand => formatter
                .write_str("no command provided; use --help to see available DStack v2 commands"),
            Self::InvalidOptions(reason) => formatter.write_str(reason),
            Self::Config(error) => error.fmt(formatter),
            Self::Registry(error) => error.fmt(formatter),
            Self::Migration(error) => error.fmt(formatter),
            Self::Discovery(error) => error.fmt(formatter),
            Self::Resolution(error) => error.fmt(formatter),
            Self::Compose(error) => error.fmt(formatter),
            Self::MigrationIncomplete { skipped, conflicts } => write!(
                formatter,
                "legacy registry migration completed with {skipped} skipped entr{} and {conflicts} conflict{}",
                if *skipped == 1 { "y" } else { "ies" },
                if *conflicts == 1 { "" } else { "s" }
            ),
            Self::Output(error) => write!(formatter, "could not write command output: {error}"),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Config(error) => Some(error),
            Self::Registry(error) => Some(error),
            Self::Migration(error) => Some(error),
            Self::Discovery(error) => Some(error),
            Self::Resolution(error) => Some(error),
            Self::Compose(error) => Some(error),
            Self::Output(error) => Some(error),
            Self::NoCommand | Self::InvalidOptions(_) | Self::MigrationIncomplete { .. } => None,
        }
    }
}

impl From<ConfigError> for AppError {
    fn from(error: ConfigError) -> Self {
        Self::Config(error)
    }
}

impl From<RegistryError> for AppError {
    fn from(error: RegistryError) -> Self {
        Self::Registry(error)
    }
}

impl From<MigrationError> for AppError {
    fn from(error: MigrationError) -> Self {
        Self::Migration(error)
    }
}

impl From<DiscoveryError> for AppError {
    fn from(error: DiscoveryError) -> Self {
        Self::Discovery(error)
    }
}

impl From<ResolutionError> for AppError {
    fn from(error: ResolutionError) -> Self {
        Self::Resolution(error)
    }
}

impl From<ComposeError> for AppError {
    fn from(error: ComposeError) -> Self {
        Self::Compose(error)
    }
}

impl From<io::Error> for AppError {
    fn from(error: io::Error) -> Self {
        Self::Output(error)
    }
}
