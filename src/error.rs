use std::{ffi::OsString, fmt, io, process::ExitCode};

use crate::{
    compose::ComposeError, config::ConfigError, discovery::DiscoveryError,
    docker::DockerOutputError, migration::MigrationError, process::ProcessError,
    registry::RegistryError, resolution::ResolutionError, session::SessionError, shell::ShellError,
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
    Session(SessionError),
    Shell(ShellError),
    Compose(ComposeError),
    Process(ProcessError),
    DockerOutput(DockerOutputError),
    InteractiveShellUnavailable {
        service: OsString,
        tried: String,
    },
    MigrationIncomplete {
        skipped: usize,
        conflicts: usize,
    },
    ShellIntegrationRequired {
        command: &'static str,
        guidance: String,
    },
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
            | Self::Session(_)
            | Self::Shell(_)
            | Self::Compose(_)
            | Self::Process(_)
            | Self::DockerOutput(_)
            | Self::InteractiveShellUnavailable { .. }
            | Self::MigrationIncomplete { .. }
            | Self::ShellIntegrationRequired { .. }
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
            Self::Session(error) => error.fmt(formatter),
            Self::Shell(error) => error.fmt(formatter),
            Self::Compose(error) => error.fmt(formatter),
            Self::Process(error) => error.fmt(formatter),
            Self::DockerOutput(error) => error.fmt(formatter),
            Self::InteractiveShellUnavailable { service, tried } => write!(
                formatter,
                "could not open an interactive shell for service '{}'; tried {tried}. Ensure the service is running and contains a supported shell.",
                service.to_string_lossy()
            ),
            Self::MigrationIncomplete { skipped, conflicts } => write!(
                formatter,
                "legacy registry migration completed with {skipped} skipped entr{} and {conflicts} conflict{}",
                if *skipped == 1 { "y" } else { "ies" },
                if *conflicts == 1 { "" } else { "s" }
            ),
            Self::ShellIntegrationRequired { command, guidance } => write!(
                formatter,
                "'{command}' requires the DStack shell adapter in the current terminal because a child process cannot modify its parent shell.\n\n{guidance}"
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
            Self::Session(error) => Some(error),
            Self::Shell(error) => Some(error),
            Self::Compose(error) => Some(error),
            Self::Process(error) => Some(error),
            Self::DockerOutput(error) => Some(error),
            Self::Output(error) => Some(error),
            Self::NoCommand
            | Self::InvalidOptions(_)
            | Self::InteractiveShellUnavailable { .. }
            | Self::MigrationIncomplete { .. } => None,
            Self::ShellIntegrationRequired { .. } => None,
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

impl From<SessionError> for AppError {
    fn from(error: SessionError) -> Self {
        Self::Session(error)
    }
}

impl From<ShellError> for AppError {
    fn from(error: ShellError) -> Self {
        Self::Shell(error)
    }
}

impl From<ComposeError> for AppError {
    fn from(error: ComposeError) -> Self {
        Self::Compose(error)
    }
}

impl From<ProcessError> for AppError {
    fn from(error: ProcessError) -> Self {
        Self::Process(error)
    }
}

impl From<DockerOutputError> for AppError {
    fn from(error: DockerOutputError) -> Self {
        Self::DockerOutput(error)
    }
}

impl From<io::Error> for AppError {
    fn from(error: io::Error) -> Self {
        Self::Output(error)
    }
}
